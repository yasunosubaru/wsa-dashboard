// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Which side-bar entries have a page behind them.
//!
//! This is a *decision*, not a handler, so it lives here rather than in
//! `handlers/common.rs` where it is used. `handlers` belongs to the GUI binary
//! and is reached through `mod ui;`, which the CLI's test run never compiles --
//! a fact this project already records twice (`data.rs`, `handlers/home.rs`).
//! Three tests written where the constant used to sit would have run zero times
//! and would have looked, in a diff, exactly like tests that do.
//!
//! # Why this list exists at all
//!
//! The side bar shows **Settings from P3 on**, because the reference gives that
//! entry no `show_*` switch -- it ships all nine pages, so there was nothing to
//! switch off -- while our settings page is P6. Without a guard, a click on it
//! would set `selected_tab` to an index with no view, the opacity switch would
//! take the home page off the screen (its `x` goes to 10000px, out of the way
//! by design), and the content area would be empty. Spec section 5 forbids that
//! on the home page and there is no reason to want it anywhere else.
//!
//! # What adds an index here
//!
//! The same act as flipping the matching `show_*` in `app.slint` -- one list, one
//! phase, kept in step:
//!
//! | phase | tabs | entries |
//! |---|---|---|
//! | P3 | 0 | home (the only one P3 could show a page for) |
//! | P4 | 0, 1, 2 | + install APK (1), + device (2) |
//! | P5a | 5 | storage -- the half of P5 that verifies with the subsystem stopped |
//! | P5b | 3 | network |
//! | P6 | 7 | settings |
//! | P6 | 7, 8 | settings, about |
//! | P7 | 4 | task scheduler |
//! | — | 6 | donate: spec section 3 maps it 1:1, but section 10 assigns it to no
//! phase, so it stays out until something does |
pub const TABS_WITH_A_VIEW: &[i32] = &[0, 1, 2, 3, 5, 7, 8];

/// The nine entries a `SidebarItem` can select, whatever has a page.
///
/// Named for what it guards rather than for the number nine: the guard's job is
/// to reject an index no `opacity` binding matches, and "every sidebar entry"
/// says what that means.
pub const SIDEBAR_ENTRY_COUNT: i32 = 9;

/// Is this a side-bar entry at all?
///
/// Separate from [`has_a_view`], because the two failures are different: an
/// index outside `0..9` is a bug in the caller, and an index inside it with no
/// page is the planned state of P3.
pub fn is_sidebar_entry(tab: i32) -> bool {
    (0..SIDEBAR_ENTRY_COUNT).contains(&tab)
}

/// May we switch to it?
///
/// Yes only when it is a real entry *and* its page exists. `false` is expected
/// today and not a warning: the side bar offers more than the program can show.
pub fn has_a_view(tab: i32) -> bool {
    is_sidebar_entry(tab) && TABS_WITH_A_VIEW.contains(&tab)
}

#[cfg(test)]
mod tests {
    use super::{TABS_WITH_A_VIEW, has_a_view, is_sidebar_entry};

    /// Six pages behind nine entries, and exactly which six.
    ///
    /// The list is asserted rather than inferred, and the loop over all nine is
    /// what ties it to `is_sidebar_entry` -- a value that reached `has_a_view`
    /// for an index no sidebar item can select would pass an `assert_eq` on the
    /// list and still be wrong. Three of the nine still open onto nothing, which
    /// is the planned state of P6's Task 5 and P7; see the table on
    /// `TABS_WITH_A_VIEW`.
    ///
    /// Renamed each time an index is added, because the old name *is* the
    /// assertion: a test called `..._and_storage_...` that also has a page for
    /// settings is a test whose name is now a partial statement, and a partial
    /// statement in a test name is read as the whole truth.
    #[test]
    fn every_built_page_has_one_and_the_two_unbuilt_ones_do_not() {
        assert_eq!(TABS_WITH_A_VIEW, &[0, 1, 2, 3, 5, 7, 8]);
        for tab in 0..=8 {
            assert_eq!(
                has_a_view(tab),
                TABS_WITH_A_VIEW.contains(&tab),
                "tab {tab} disagrees with the sidebar switch that shows it"
            );
        }
        for &tab in TABS_WITH_A_VIEW {
            assert!(is_sidebar_entry(tab), "tab {tab} is not one of the nine");
        }
    }

    /// Everything in the list is something the side bar can select. An
    /// out-of-range index would set `selected_tab` to a value no `opacity`
    /// binding matches, which is how you get a screen with every page switched
    /// off at once -- the one outcome worse than the blank page the guard
    /// exists to prevent.
    #[test]
    fn every_tab_with_a_view_is_a_real_sidebar_entry() {
        for &tab in TABS_WITH_A_VIEW {
            assert!(is_sidebar_entry(tab), "tab {tab} is not one of the nine");
        }
    }

    /// The settings entry got its page in P6. The test that used to say the
    /// opposite -- that it was offered and led nowhere -- now says it works,
    /// because the failure it was guarding against (an entry visible with no
    /// page behind it, so a click sets `selected_tab` to an index no opacity
    /// binding matches and the content area goes empty) is now the failure P7
    /// would introduce if it turned the scheduler entry on without adding 4 here.
    #[test]
    fn the_settings_entry_has_a_page_now() {
        assert!(is_sidebar_entry(7));
        assert!(has_a_view(7));
    }

    /// The three entries that still lead nowhere, named. A test that asserts a
    /// negative is easy to leave behind when the thing it names arrives, so it
    /// is written as the positive statement about the ones that *do* have pages
    /// plus this enumeration, and P7 will have to delete the scheduler half.
    #[test]
    fn only_the_scheduler_and_donate_entries_still_lead_nowhere() {
        let without_a_page: Vec<i32> = (0..=8).filter(|tab| !has_a_view(*tab)).collect();
        assert_eq!(
            without_a_page,
            vec![4, 6],
            "scheduler is P7; donate has no phase in spec section 10 at all"
        );
    }

    /// The home page is the one that must never be refused. `has_a_view(0)` is
    /// what the guard consults, and a P3 that answers false here is a P3 that
    /// cannot navigate at all.
    #[test]
    fn the_home_page_is_always_available() {
        assert!(has_a_view(0));
    }

    /// Both halves, and the difference between them, for every index.
    ///
    /// `0..9` is the reference's nine entries (`sidebar.slint`), not a number
    /// chosen to make the test pass; one past the end is a real bug class --
    /// `selected_tab` set to 9 would darken every entry and show no page.
    #[test]
    fn entries_outside_the_nine_are_rejected_and_pages_track_the_list() {
        for tab in -1..=10 {
            assert_eq!(is_sidebar_entry(tab), (0..9).contains(&tab));
            if is_sidebar_entry(tab) {
                assert_eq!(
                    has_a_view(tab),
                    TABS_WITH_A_VIEW.contains(&tab),
                    "tab {tab} disagrees between the two predicates"
                );
            } else {
                assert!(!has_a_view(tab), "tab {tab} had a view without being an entry");
            }
        }
    }
}
