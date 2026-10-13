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
//! | P3 | 0 | home (the only one P3 can show a page for) |
//! | P4 | 1, 2 | install APK, device |
//! | P5 | 3, 5 | network, storage |
//! | P6 | 7, 8 | settings, about |
//! | P7 | 4 | task scheduler |
//! | — | 6 | donate: spec section 3 maps it 1:1, but section 10 assigns it to no
//! phase, so it stays out until something does |
pub const TABS_WITH_A_VIEW: &[i32] = &[0];

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

    /// The state of P3, stated as a fact rather than implied by a handler body:
    /// one page exists, and the side bar offers more than one entry.
    #[test]
    fn p3_has_exactly_one_tab_with_a_page() {
        assert_eq!(TABS_WITH_A_VIEW, &[0]);
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

    /// The settings entry is visible in P3 while its page is P6. If either
    /// half of that changes without the other -- the entry gaining a page before
    /// it is added here, or being removed from here while it is still visible --
    /// clicking Settings blanks the screen.
    #[test]
    fn settings_is_offered_but_not_yet_available() {
        assert!(is_sidebar_entry(7));
        assert!(!has_a_view(7));
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
