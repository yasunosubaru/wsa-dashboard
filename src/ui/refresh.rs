// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! When a refresh is allowed to happen.
//!
//! The reference keeps this rule inside `src/ui/data.rs`, in a function that
//! also mutates a static. Pulled out and made pure because it is the only thing
//! standing between "the user pressed start" and "adb is asked again a second
//! later" -- and because a rule that is inseparable from a clock cannot be
//! tested without sleeping, and a rule nobody tests is a rule that erodes.
//!
//! Compiled into the CLI bin purely so `cargo test --release --bin wsadash`
//! can run these tests; the GUI is the only caller.

#![allow(dead_code)]

use std::time::Duration;

use super::home_model::{snapshot_changed, HomeSnapshot};

/// How long a routine refresh is held off for.
///
/// The reference's number. `wsl -l -v` is a snapshot command and adb's
/// `pm list packages` is no cheaper, so a tick that fires every four seconds
/// no matter what spends most of its life spawning a process nobody asked
/// about. The home page's own tick is slower than this; the gate exists for
/// the paths that are not the timer -- clicks, restores, reconnects.
pub const DEBOUNCE: Duration = Duration::from_secs(4);

/// The reason spelling that bypasses the debounce and the visibility check.
///
/// The reference's exact words, kept as a constant rather than repeated
/// literals, because the reference compares this by value at several call
/// sites and a second spelling would silently bypass nothing.
pub const MANUAL_TRIGGER: &str = "manual trigger";

/// The reason the timer uses.
///
/// Also the reference's spelling, for the same reason.
pub const PERIODIC_TRIGGER: &str = "periodic trigger";

/// Whether this poll may run.
///
/// Three rules, in the reference's order:
///
/// 1. **An operation in flight owns the adapter.** Asking mid-flight reports a
///    half-applied state and then, a second later, its opposite. This is why
///    the busy flag comes in rather than being read from a global: the caller
///    holds the one place that knows whether an operation is running, and a
///    second source of that truth here would be a second thing to keep in
///    step with it.
/// 2. **Routine refreshes are spaced out.** Anything under [`DEBOUNCE`] is
///    dropped unless the user asked for it by name.
/// 3. **A hidden window is not asked on its behalf.** Minimised to the tray is
///    not a reason to keep spending adb round trips; an explicit click is.
///
/// A manual trigger clears rules 2 and 3, but not rule 1: a user clicking
/// "refresh" while an uninstall is running gets the same answer as the timer
/// would have given, and is no better informed by being asked twice.
pub fn should_refresh(reason: &str, is_visible: bool, busy: bool, elapsed: Option<Duration>) -> bool {
    if busy {
        return false;
    }

    if reason == MANUAL_TRIGGER {
        return true;
    }

    if let Some(elapsed) = elapsed {
        if elapsed < DEBOUNCE {
            return false;
        }
    }

    if !is_visible {
        return false;
    }

    true
}

/// Whether a new list should replace the one the window already has.
///
/// `previous` is what the window was last given -- the snapshot its rows were
/// built from, and the query they were filtered with. `current` is this poll's.
///
/// Two inputs because either alone is insufficient: a new snapshot behind the
/// same query is a new list, and the same snapshot behind a changed query is a
/// new list too. Replacing a model that would be identical is what this
/// exists to prevent -- `set_distros` re-renders every card, and doing it four
/// times a minute for no reason is a flicker nobody asked for.
pub fn repush_needed(
    previous: Option<(&HomeSnapshot, &str)>,
    current: Option<&HomeSnapshot>,
    query: &str,
) -> bool {
    match (previous, current) {
        // Nothing pushed, nothing to push. The window already starts with an
        // empty model, so writing one again would be a no-op -- which is why
        // the query does not get a vote here: with no rows there is nothing for
        // a filter to change.
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some((prev, prev_query)), Some(now)) => {
            prev_query != query || snapshot_changed(prev, now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Not via `home_model`: that module imports `AppInfo` privately, and a
    // path through another module's private `use` binding does not resolve.
    use crate::wsa::models::AppInfo;

    #[test]
    fn manual_trigger_bypasses_the_four_second_gate() {
        assert!(should_refresh(
            MANUAL_TRIGGER,
            true,
            false,
            Some(Duration::from_millis(1))
        ));
    }

    #[test]
    fn elapsed_under_four_seconds_blocks_non_manual() {
        assert!(!should_refresh(
            PERIODIC_TRIGGER,
            true,
            false,
            Some(DEBOUNCE - Duration::from_millis(1))
        ));
    }

    #[test]
    fn four_seconds_and_over_is_long_enough() {
        assert!(should_refresh(
            PERIODIC_TRIGGER,
            true,
            false,
            Some(DEBOUNCE)
        ));
        assert!(should_refresh(PERIODIC_TRIGGER, true, false, None));
    }

    /// Minimised to the tray is not a reason to keep spending adb round trips.
    #[test]
    fn hidden_window_blocks_non_manual() {
        assert!(!should_refresh(PERIODIC_TRIGGER, false, false, Some(DEBOUNCE)));
    }

    /// A manual trigger still respects the window state -- no, it does not, and
    /// that is the point of the rule: an explicit click is the user asking.
    #[test]
    fn manual_trigger_is_asked_even_when_the_window_is_hidden() {
        assert!(should_refresh(MANUAL_TRIGGER, false, false, None));
    }

    /// An operation in flight owns the adapter, and that outranks both of the
    /// rules a manual trigger is allowed to clear.
    #[test]
    fn busy_blocks_everything() {
        assert!(!should_refresh(PERIODIC_TRIGGER, true, true, Some(DEBOUNCE)));
        assert!(!should_refresh(MANUAL_TRIGGER, true, true, None));
        assert!(!should_refresh(MANUAL_TRIGGER, false, true, None));
    }

    /// The bypass is decided by comparing the reason, so the spelling has to
    /// be exact. What must NOT happen is a differently-spelled reason slipping
    /// past the debounce; being allowed at all when nothing else holds it back
    /// is correct, and is asserted below so the two are not confused.
    #[test]
    fn the_manual_trigger_spelling_is_exact() {
        assert!(!should_refresh(
            "manual trigger please",
            true,
            false,
            Some(Duration::from_millis(1))
        ));
        assert!(!should_refresh(
            "Manual Trigger",
            true,
            false,
            Some(Duration::from_millis(1))
        ));

        // With no debounce left to bypass and a visible window, a routine
        // refresh is allowed -- the spelling only decides *which rules apply*.
        assert!(should_refresh("Manual Trigger", true, false, None));
    }

    // -- repush_needed ------------------------------------------------------
    //
    // The second gate. `should_refresh` decides whether to ask; this decides
    // whether what came back is worth re-rendering the grid for.

    fn snap(apps: &[&str], running: &[&str]) -> HomeSnapshot {
        HomeSnapshot {
            apps: apps
                .iter()
                .map(|package| AppInfo {
                    package: (*package).to_string(),
                    version_name: "1.0".into(),
                    version_code: 1,
                    apk_size_bytes: 1024,
                    is_system: false,
                })
                .collect(),
            running: running.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn first_push_of_an_empty_list_is_skipped() {
        assert!(
            !repush_needed(None, None, ""),
            "the window already starts with an empty model; writing one again \
             would be a no-op that costs a model rebuild"
        );
    }

    #[test]
    fn a_status_flip_is_pushed_even_though_the_packages_are_identical() {
        let before = snap(&["com.a"], &[]);
        let after = snap(&["com.a"], &["com.a"]);
        assert!(repush_needed(Some((&before, "")), Some(&after), ""));
    }

    #[test]
    fn the_same_snapshot_behind_a_new_query_is_pushed() {
        let source = snap(&["com.a", "com.b"], &[]);
        assert!(repush_needed(
            Some((&source, "com.a")),
            Some(&source),
            "com.b"
        ));
    }

    #[test]
    fn an_unchanged_snapshot_behind_the_same_query_is_not_pushed() {
        let source = snap(&["com.a"], &[]);
        assert!(repush_needed(None, Some(&source), ""));
        assert!(!repush_needed(
            Some((&source, "")),
            Some(&source),
            ""
        ));
        assert!(!repush_needed(
            Some((&source, "")),
            Some(&source),
            ""
        ));
    }

    #[test]
    fn rows_appear_when_a_snapshot_arrives_and_vanish_when_the_cache_does() {
        let source = snap(&["com.a"], &[]);

        // Nothing to draw from, then something: the list is appearing.
        assert!(repush_needed(None, Some(&source), ""));

        // Before there was a snapshot and now there is none: rows are being
        // withdrawn, and the window must be told or it keeps showing cards
        // nobody can vouch for.
        assert!(repush_needed(Some((&source, "")), None, ""));

        // ...and the poll after that, still nothing, changes nothing.
        assert!(!repush_needed(None, None, ""));
    }
}
