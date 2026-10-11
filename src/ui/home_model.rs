// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The home page's view model.
//!
//! The reference has no counterpart to this file: it assembles `Distro`
//! structs inline inside `src/ui/data.rs`. It is pulled out here because the
//! one thing P2 must be able to prove without a window is that a dead adb
//! connection still renders something (spec section 5).
//!
//! Every degraded tier is expressed with a component the reference already
//! ships. Nothing in this file may justify a new one.

// The GUI bin is the only caller, and this file is also compiled into the
// CLI purely so `cargo test --release --bin wsadash` can run its tests
// without dragging in the Slint UI. Everything below therefore looks unused
// from the CLI's side. `command.rs` carries the same allowance for the same
// reason.
#![allow(dead_code)]

use std::collections::HashSet;

use crate::adb::decoder::package_is_running;
use crate::wsa::dashboard::op_guard;
use crate::wsa::models::{AppInfo, DegradationTier, SubsystemStatus, SubsystemState};

// -- i18n keys, kept under the reference's names -----------------------------
//
// Only the keys Rust has to decide between live here. `main.loading`,
// `main.subsystem_booting` and the rest of the always-on labels are used
// straight from `.slint` and belong to the i18n table (Task 5).

pub const NO_APPS: &str = "main.no_apps";
pub const STALE: &str = "main.stale_data";
pub const UNAUTHORIZED: &str = "main.subsystem_unauthorized";
pub const OFFLINE: &str = "main.subsystem_offline";
pub const STOPPED: &str = "main.subsystem_stopped";
pub const NOT_INSTALLED: &str = "main.subsystem_not_installed";
pub const START_SUBSYSTEM: &str = "main.start_subsystem";
pub const GUIDE_LINK: &str = "main.guide_link";

// -- how the reference's `Distro` struct is filled for us -------------------

/// Spec R2's cold start. `com.android.settings` exists in every WSA install,
/// so opening it starts the subsystem even when no user app is installed.
pub const START_WSA_URI: &str = "wsa://com.android.settings";

/// Where `main.guide_link` points when WSA is not installed at all (R6).
///
/// Verified reachable 2026-10-11: Microsoft's WSA-specific page. It redirects
/// to the "previous versions" archive because WSA left the Microsoft Store on
/// 2025-03-05, and it still documents the exact `adb connect
/// 127.0.0.1:58526` this app uses -- so it is the most useful destination
/// left, and unlike the current dev-tools hub it is actually about WSA.
pub const GUIDE_URL: &str = "https://learn.microsoft.com/en-us/windows/android/wsa/";

/// Q29's palette has eight tints. The *colours* are not here -- they live in
/// `Theme.badge-tint` in `theme.slint`, because which tint a package gets is
/// data and what that tint looks like on a light or a dark background is
/// presentation. Slint also has no way to turn a `#rrggbb` string back into a
/// colour at runtime, so a colour travelling this way could not be painted.
pub const PALETTE_LEN: usize = 8;

/// How the home page degrades.
///
/// Deliberately not one-to-one with [`SubsystemState`]: these name the
/// *rendering*, and several states render the same way.
///
/// Each variant is a component the reference already ships. Adding one would
/// be a defect at review (2026-10-11 ruling).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Degraded {
    /// Nothing degraded. The list renders exactly as the reference renders it.
    None,
    /// The reference's own loading overlay: "..." plus `main.loading`.
    LoadingOverlay,
    /// Empty rows pushed as placeholders, which the reference's `DistroCard`
    /// draws as grey blocks. `toast_key` carries the reason.
    Skeleton { toast_key: &'static str },
    /// `DistroNotFoundView`. An empty `action_url` is what makes the
    /// reference render no link at all (`if (root.guide_url != "")`).
    NotFound {
        headline_key: &'static str,
        action_key: &'static str,
        action_url: String,
    },
}

/// One card. Mirrors the reference's `Distro` fields, reinterpreted.
#[derive(Debug, Clone, PartialEq)]
pub struct AppRow {
    pub package: String,
    pub display_name: String,
    pub version: String,
    /// Q29: first character of `display_name`.
    pub initial: String,
    /// Q29: `1..=PALETTE_LEN`, stable per package; `0` means "no opinion", and
    /// the card falls back to the reference's fixed grey. Index rather than a
    /// colour, for the reason `PALETTE_LEN` gives.
    pub initial_color_index: i32,
    pub running: bool,
}

#[derive(Debug, Clone)]
pub struct HomeViewModel {
    pub tier: DegradationTier,
    pub degraded: Degraded,
    pub rows: Vec<AppRow>,
    /// A non-`Ready` tier that still has something to show is showing a
    /// snapshot from before the break.
    pub greyed: bool,
    pub stale_label_key: &'static str,
    /// One op at a time (spec 2.2), so one op disables every row's buttons.
    pub buttons_disabled: bool,
    /// Text for `TaskStatusToast`, or `""` for none.
    pub toast_key: &'static str,
}

impl HomeViewModel {
    /// What `DistroNotFoundView` prints; `""` when that view is not shown.
    pub fn headline_key(&self) -> &'static str {
        match self.degraded {
            Degraded::NotFound { headline_key, .. } => headline_key,
            _ => "",
        }
    }

    /// The link slot's label; `""` when that view is not shown.
    pub fn action_key(&self) -> &'static str {
        match self.degraded {
            Degraded::NotFound { action_key, .. } => action_key,
            _ => "",
        }
    }

    /// What the link slot opens; `""` when the reference renders no link.
    pub fn action_url(&self) -> &str {
        match &self.degraded {
            Degraded::NotFound { action_url, .. } => action_url.as_str(),
            _ => "",
        }
    }
}

/// One poll's worth of truth. `last_good` lives in `data.rs`, not here.
#[derive(Debug, Clone)]
pub struct HomeSnapshot {
    pub apps: Vec<AppInfo>,
    /// `ps -A`'s answer. An empty set means "nothing is running", not "could
    /// not read" -- see `WsaDashboard::running_apps`.
    pub running: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct HomeInputs<'a> {
    pub status: &'a SubsystemStatus,
    /// Only a `Ready` tier has one.
    pub fresh: Option<&'a HomeSnapshot>,
    /// Kept across failures so a degraded tier still has rows to grey.
    pub last_good: Option<&'a HomeSnapshot>,
    /// Task 12's single slot.
    pub active_op: Option<&'static str>,
    /// Q24's search box.
    pub query: &'a str,
}

/// Build the view model for one poll.
pub fn build(inputs: &HomeInputs) -> HomeViewModel {
    let tier = inputs.status.state.tier();
    let op_key = inputs.active_op.map(operation_key).unwrap_or("");

    // A degraded tier never draws from `fresh`: either the read failed or it
    // was skipped, so trusting it would show a page that looks live and is
    // not.
    let source = match tier {
        DegradationTier::Ready => inputs.fresh.or(inputs.last_good),
        _ => inputs.last_good,
    };
    let greyed = tier != DegradationTier::Ready && source.is_some();

    let (degraded, rows) = match tier {
        DegradationTier::Ready => {
            let rows = source
                .map(|snap| rows_of(snap, inputs.query))
                .unwrap_or_default();
            if rows.is_empty() {
                (no_apps(), rows)
            } else {
                (Degraded::None, rows)
            }
        }
        DegradationTier::Partial => {
            let rows = source
                .map(|snap| rows_of(snap, inputs.query))
                .unwrap_or_default();
            (waiting_on(inputs.status.state), rows)
        }
        DegradationTier::Down => {
            let rows = source
                .map(|snap| rows_of(snap, inputs.query))
                .unwrap_or_default();
            (offer_start(inputs.status.state), rows)
        }
    };

    // An operation outranks everything: the user needs to know what the app
    // is doing more than it needs to know the tier.
    let toast_key = if !op_key.is_empty() {
        op_key
    } else {
        match &degraded {
            Degraded::Skeleton { toast_key } => toast_key,
            _ => "",
        }
    };

    HomeViewModel {
        tier,
        degraded,
        rows,
        greyed,
        stale_label_key: if greyed { STALE } else { "" },
        buttons_disabled: !op_key.is_empty(),
        toast_key,
    }
}

fn no_apps() -> Degraded {
    Degraded::NotFound {
        headline_key: NO_APPS,
        action_key: "",
        action_url: String::new(),
    }
}

/// `Partial` tiers. Booting gets the reference's loading overlay; the other
/// two get skeleton rows plus a toast, because a stale entry still has an app
/// list behind it and "start" would be the wrong action.
fn waiting_on(state: SubsystemState) -> Degraded {
    match state {
        SubsystemState::Booting => Degraded::LoadingOverlay,
        SubsystemState::Unauthorized => Degraded::Skeleton {
            toast_key: UNAUTHORIZED,
        },
        _ => Degraded::Skeleton {
            toast_key: OFFLINE,
        },
    }
}

/// `Down` tiers. The link slot carries the one action that makes sense.
fn offer_start(state: SubsystemState) -> Degraded {
    match state {
        SubsystemState::NotInstalled => Degraded::NotFound {
            headline_key: NOT_INSTALLED,
            action_key: GUIDE_LINK,
            action_url: GUIDE_URL.to_string(),
        },
        _ => Degraded::NotFound {
            headline_key: STOPPED,
            action_key: START_SUBSYSTEM,
            action_url: START_WSA_URI.to_string(),
        },
    }
}

/// Task 12's op names, rendered as the reference's `operation.*` keys.
///
/// `Launching` and `Installing` have no counterpart in the reference's table
/// -- it never launches or installs a distro from the UI -- so both are new
/// keys under the reference's naming scheme (MAPPING 4.1).
fn operation_key(op: &str) -> &'static str {
    match op {
        op_guard::STARTING => "operation.starting",
        op_guard::LAUNCHING => "operation.launching",
        op_guard::STOPPING => "operation.stopping",
        op_guard::INSTALLING => "operation.installing",
        op_guard::UNINSTALLING => "operation.deleting",
        _ => "",
    }
}

fn rows_of(snap: &HomeSnapshot, query: &str) -> Vec<AppRow> {
    let mut rows: Vec<AppRow> = snap.apps.iter().map(row_of).collect();
    for row in &mut rows {
        row.running = package_is_running(&row.package, &snap.running);
    }
    filter_rows(rows, query)
}

fn row_of(info: &AppInfo) -> AppRow {
    let display_name = info.display_name();
    AppRow {
        package: info.package.clone(),
        initial: initial_of(&display_name),
        initial_color_index: initial_color_of(&info.package),
        version: info.version_name.clone(),
        display_name,
        running: false,
    }
}

/// First character of the display name. `chars`, not bytes: a CJK name has
/// no ASCII letter and the badge must still show something.
pub fn initial_of(display_name: &str) -> String {
    display_name
        .chars()
        .next()
        .map(String::from)
        .unwrap_or_default()
}

/// Which of the eight tints a package gets, so the same package is the same
/// colour on every run -- a colour that shuffled per refresh would look like a
/// bug, and there is no reason to make it look like one.
///
/// Returns `1..=PALETTE_LEN`. Zero is reserved for "no opinion", which is what
/// a placeholder row has: it stands for an app we cannot see, not for one with
/// a name worth tinting.
pub fn initial_color_of(package: &str) -> i32 {
    let sum: usize = package.bytes().map(|byte| byte as usize).sum();
    (sum % PALETTE_LEN) as i32 + 1
}

/// Q24: match either the display name or the package, ignoring case.
pub fn filter_rows(rows: Vec<AppRow>, query: &str) -> Vec<AppRow> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| {
            row.display_name.to_lowercase().contains(&needle)
                || row.package.to_lowercase().contains(&needle)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wsa::models::{AppInfo, DegradationTier, SubsystemState, SubsystemStatus};

    fn status(state: SubsystemState) -> SubsystemStatus {
        SubsystemStatus {
            state,
            adb_port: 58526,
            boot_completed: state == SubsystemState::Ready,
        }
    }

    fn app(package: &str) -> AppInfo {
        AppInfo {
            package: package.into(),
            version_name: "1.0".into(),
            version_code: 1,
            apk_size_bytes: 1024,
            is_system: false,
        }
    }

    fn snapshot(apps: Vec<AppInfo>, running: &[&str]) -> HomeSnapshot {
        HomeSnapshot {
            apps,
            running: running.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// The status has to outlive the call to `build`, so the helper owns it
    /// and hands back the finished model rather than a pair to hold together.
    fn model_of(
        state: SubsystemState,
        fresh: Option<&HomeSnapshot>,
        last_good: Option<&HomeSnapshot>,
    ) -> HomeViewModel {
        let status = status(state);
        build(&HomeInputs {
            status: &status,
            fresh,
            last_good,
            active_op: None,
            query: "",
        })
    }

    /// A Ready subsystem with apps must render the list and nothing else. This
    /// is the test that would catch any stray degraded element leaking into
    /// the normal case, which the review ruled a defect.
    #[test]
    fn ready_tier_renders_the_list_with_no_degraded_element() {
        let snap = snapshot(vec![app("com.foo.app")], &[]);
        let model = model_of(SubsystemState::Ready, Some(&snap), None);

        assert_eq!(model.tier, DegradationTier::Ready);
        assert_eq!(model.degraded, Degraded::None);
        assert!(!model.greyed);
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].package, "com.foo.app");
        assert_eq!(model.stale_label_key, "");
        assert_eq!(model.toast_key, "");
        assert!(!model.buttons_disabled);
    }

    /// Booting is the reference's own loading overlay: "..." plus
    /// `main.loading`. It is the third `if` branch of the reference's
    /// `MainView`, so no new component is involved -- and no toast on top,
    /// because the overlay already says the app is waiting.
    #[test]
    fn booting_tier_renders_the_reference_loading_overlay() {
        let model = model_of(SubsystemState::Booting, None, None);
        assert_eq!(model.degraded, Degraded::LoadingOverlay);
        assert_eq!(model.toast_key, "");
        assert!(model.rows.is_empty());
    }

    /// Offline has a stale-entry shape: the subsystem is running, so `Stopped`
    /// would be a lie and a start action would do nothing. Skeleton rows plus
    /// a status toast; reconnection is automatic, not a button.
    #[test]
    fn offline_tier_renders_skeleton_rows_and_a_status_toast() {
        let model = model_of(SubsystemState::Offline, None, None);
        assert_eq!(
            model.degraded,
            Degraded::Skeleton {
                toast_key: "main.subsystem_offline"
            }
        );
        assert!(model.rows.is_empty(), "skeleton rows are pushed as empty Distro entries");
        assert_eq!(model.toast_key, "main.subsystem_offline");
    }

    #[test]
    fn unauthorized_tier_renders_skeleton_rows_and_a_toast() {
        let model = model_of(SubsystemState::Unauthorized, None, None);
        assert_eq!(
            model.degraded,
            Degraded::Skeleton {
                toast_key: "main.subsystem_unauthorized"
            }
        );
        assert_eq!(model.toast_key, "main.subsystem_unauthorized");
    }

    /// The start action rides in `DistroNotFoundView`'s link slot, which the
    /// reference hands to `open_url`. `wsa://com.android.settings` is the URI
    /// spec R2 picked as the cold-start fallback.
    #[test]
    fn stopped_tier_renders_the_not_found_view_with_the_start_action_url() {
        let model = model_of(SubsystemState::Stopped, None, None);
        assert_eq!(
            model.degraded,
            Degraded::NotFound {
                headline_key: "main.subsystem_stopped",
                action_key: "main.start_subsystem",
                action_url: "wsa://com.android.settings".into(),
            }
        );
    }

    /// With a snapshot to fall back on, the list branch keeps rendering --
    /// the reference shows its list whenever `distros` is non-empty -- greyed
    /// and labelled so nobody mistakes stale rows for a live read.
    #[test]
    fn stopped_tier_with_a_snapshot_keeps_the_rows_greyed_and_labelled() {
        let snap = snapshot(vec![app("com.foo.app")], &[]);
        let model = model_of(SubsystemState::Stopped, None, Some(&snap));

        assert_eq!(model.rows.len(), 1);
        assert!(model.greyed);
        assert_eq!(model.stale_label_key, "main.stale_data");
    }

    /// **The acceptance anchor for this phase.** No snapshot, nothing to
    /// list: the page still says something rather than rendering a hole.
    #[test]
    fn stopped_tier_without_any_snapshot_renders_the_not_found_view_not_blank() {
        let model = model_of(SubsystemState::Stopped, None, None);

        assert!(model.rows.is_empty());
        assert_eq!(model.headline_key(), "main.subsystem_stopped");
        assert_eq!(model.action_key(), "main.start_subsystem");
        assert_eq!(model.action_url(), START_WSA_URI);
        assert!(!model.headline_key().is_empty(), "a blank tier renders an empty page");
    }

    #[test]
    fn not_installed_tier_renders_the_not_found_view() {
        let model = model_of(SubsystemState::NotInstalled, None, None);
        assert_eq!(model.headline_key(), "main.subsystem_not_installed");
        assert_eq!(model.action_url(), GUIDE_URL);
    }

    /// An empty `guide_url` is what makes the reference skip its link
    /// entirely (`if (root.guide_url != "")`), so "no apps" needs no change
    /// to that component either.
    #[test]
    fn ready_tier_with_zero_apps_renders_the_not_found_view_without_a_link() {
        let snap = snapshot(vec![], &[]);
        let model = model_of(SubsystemState::Ready, Some(&snap), None);

        assert_eq!(model.tier, DegradationTier::Ready);
        assert_eq!(model.headline_key(), "main.no_apps");
        assert_eq!(model.action_key(), "", "no dangling label");
        assert_eq!(model.action_url(), "", "no dangling link");
        assert!(!model.greyed, "an empty Ready list is not a stale one");
    }

    /// One op at a time is the whole concurrency story (spec 2.2), so one
    /// running op disables every row rather than just its own: the executor
    /// is a single global gate anyway, and greying only one card would
    /// imply the others were clickable.
    #[test]
    fn any_active_op_disables_every_row_button() {
        let snap = snapshot(vec![app("com.foo.app"), app("com.bar.app")], &[]);
        let status = status(SubsystemState::Ready);
        let model = build(&HomeInputs {
            status: &status,
            fresh: Some(&snap),
            last_good: None,
            active_op: Some("Starting"),
            query: "",
        });
        assert!(model.buttons_disabled);
        assert_eq!(model.toast_key, "operation.starting");
    }

    #[test]
    fn search_filters_by_package_and_display_name_case_insensitively() {
        let mut rows = vec![
            row_of(&app("com.topjohnwu.magisk")),
            row_of(&app("com.microsoft.launcher")),
        ];
        rows[0].running = true;

        let hit = filter_rows(rows.clone(), "MAGISK");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].package, "com.topjohnwu.magisk");

        let hit = filter_rows(rows.clone(), "launcher");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].package, "com.microsoft.launcher");

        assert!(filter_rows(rows.clone(), "nope").is_empty());
        assert_eq!(filter_rows(rows, "").len(), 2, "an empty query keeps everything");
    }

    #[test]
    fn initial_color_is_stable_for_the_same_package() {
        let first = initial_color_of("com.topjohnwu.magisk");
        assert_eq!(first, initial_color_of("com.topjohnwu.magisk"));
        assert!(
            (1..=PALETTE_LEN as i32).contains(&first),
            "expected 1..={PALETTE_LEN}, got {first}"
        );
    }

    /// A different package is not obliged to land on a different colour --
    /// eight tints for as many packages as a user installs means collisions
    /// are normal -- but the mapping must not be a constant, or Q29's
    /// per-package identity buys nothing.
    #[test]
    fn initial_color_is_not_one_colour_for_every_package() {
        let packages = [
            "com.topjohnwu.magisk",
            "com.microsoft.launcher",
            "com.nowcent.ham",
            "org.mozilla.firefox",
            "com.android.chrome",
            "com.whatsapp",
        ];
        let distinct: std::collections::HashSet<i32> = packages
            .iter()
            .map(|package| initial_color_of(package))
            .collect();
        assert!(
            distinct.len() > 1,
            "every package shares one tint, so Q29 buys nothing: {distinct:?}"
        );
    }

    /// A CJK display name has no ASCII letter to take, and the card must
    /// still show something rather than a blank badge.
    #[test]
    fn initial_of_chinese_display_name_takes_first_char() {
        assert_eq!(initial_of("微信"), "微");
        assert_eq!(initial_of("Magisk"), "M");
        assert_eq!(initial_of(""), "");
    }

    /// Every key the model chooses between has to resolve, or the card would
    /// render the raw key to the user.
    #[test]
    fn every_key_this_module_names_is_in_the_i18n_table() {
        for key in [
            NO_APPS,
            STALE,
            UNAUTHORIZED,
            OFFLINE,
            STOPPED,
            NOT_INSTALLED,
            START_SUBSYSTEM,
            GUIDE_LINK,
        ] {
            assert_ne!(
                crate::i18n::t(key),
                key,
                "{key} is named by the view model but has no translation"
            );
        }
    }

    #[test]
    fn row_of_builds_the_card_from_an_app_info() {
        let info = app("com.topjohnwu.magisk");
        let row = row_of(&info);
        assert_eq!(row.display_name, "Magisk");
        assert_eq!(row.version, "1.0");
        assert!(!row.running);
    }

    #[test]
    fn a_running_package_marks_its_row_running() {
        let snap = snapshot(vec![app("com.foo.app")], &["com.foo.app"]);
        let model = model_of(SubsystemState::Ready, Some(&snap), None);
        assert!(model.rows[0].running);
    }

    #[test]
    fn a_subprocess_alone_marks_its_row_running() {
        let snap = snapshot(vec![app("com.foo.app")], &["com.foo.app:remote"]);
        let model = model_of(SubsystemState::Ready, Some(&snap), None);
        assert!(model.rows[0].running);
    }

    /// `com.foo` must not read `com.foobar` as running. The colon is the
    /// whole distinction.
    #[test]
    fn a_bare_prefix_does_not_mark_a_row_running() {
        let snap = snapshot(vec![app("com.foo.app")], &["com.foo.appx"]);
        let model = model_of(SubsystemState::Ready, Some(&snap), None);
        assert!(!model.rows[0].running);
    }

    #[test]
    fn an_unreadable_process_list_never_marks_anything_running() {
        let snap = snapshot(vec![app("com.foo.app")], &[]);
        let model = model_of(SubsystemState::Ready, Some(&snap), None);
        assert!(!model.rows[0].running, "spec 5: an empty set is an answer, not a failure");
    }

    /// Searching must not resurrect a stale snapshot into the Ready tier,
    /// where the app list is already known to be complete.
    #[test]
    fn the_query_filters_stale_rows_too() {
        let snap = snapshot(vec![app("com.foo.app"), app("com.bar.app")], &[]);
        let status = status(SubsystemState::Stopped);
        let model = build(&HomeInputs {
            status: &status,
            fresh: None,
            last_good: Some(&snap),
            active_op: None,
            query: "foo",
        });
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].package, "com.foo.app");
    }
}
