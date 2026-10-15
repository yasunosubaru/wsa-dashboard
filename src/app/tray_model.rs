// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The tray's menu and tooltip, as data.
//!
//! Split out of `tray.rs` for the reason `disk_model` is split out of
//! `handlers/disk.rs`: `tray.rs` names `AppWindow` and `tray_icon`, so it only
//! compiles in the GUI binary, and a test written there is a test nothing runs.
//! What is left here is the part that is a decision rather than a Win32 call --
//! which rows the menu has, in what order, and what the tooltip says.
//!
//! # The coupling worth pinning
//!
//! The menu is built from [`entries`] and the clicks are matched against the same
//! ids in `tray.rs`. The reference writes the id twice -- once in
//! `MenuItem::with_id("show", ...)` and once in `match event.id.as_ref() { "show"
//! => ... }` -- and a rename in one of the two places produces a menu item that
//! looks right, translates right, and does nothing at all when clicked. Here the
//! id is a constant used by both, and [`entries`] is what the builder iterates, so
//! the two cannot drift.
//!
//! Nothing is ever disabled. The reference passes `true` for both items, and the
//! test below says so out loud rather than leaving it to a default: a greyed-out
//! row in a two-item tray menu is a control that exists to explain that it cannot
//! be used, and there is nothing in this program that would earn one.

/// One row of the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayEntry {
    /// The id `tray-icon` hands back when the row is clicked. `tray.rs` matches
    /// on this, and it is not a translated string -- it is a key, and it stays
    /// English for the same reason every other identifier does (Q12).
    pub id: &'static str,
    /// The i18n key the visible label is resolved from.
    pub label_key: &'static str,
    /// Whether the row can be clicked.
    pub enabled: bool,
}

/// The id of the row that brings the window back.
pub const SHOW_ID: &str = "show";
/// The id of the row that ends the program.
pub const EXIT_ID: &str = "exit";

/// The menu, in order. Show first, exit last, and a separator between them is the
/// builder's business rather than this list's.
pub fn entries() -> [TrayEntry; 2] {
    [
        TrayEntry {
            id: SHOW_ID,
            label_key: "tray.show_window",
            enabled: true,
        },
        TrayEntry {
            id: EXIT_ID,
            label_key: "tray.exit",
            enabled: true,
        },
    ]
}

/// The tooltip: the program's name and this build's version.
///
/// Takes both rather than reading them, so the shape is testable without a
/// compiled-in version and so the caller is the one place that decides where the
/// name comes from (`app::constants::APP_NAME`, which is the same string the
/// window caption and the About page use).
pub fn tooltip(app_name: &str, version: &str) -> String {
    format!("{app_name} v{version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two rows, in this order. The order is the whole of the menu's layout, and
    /// the reference's is the same.
    #[test]
    fn the_menu_is_show_then_exit() {
        let entries = entries();
        assert_eq!(entries.len(), 2, "the reference's tray has two rows");
        assert_eq!(entries[0].id, SHOW_ID);
        assert_eq!(entries[1].id, EXIT_ID);
        assert_eq!(entries[0].id, "show");
        assert_eq!(entries[1].id, "exit");
    }

    /// The ids are matched by `tray.rs` and are also Windows menu ids. An empty
    /// one, one with a space, or two that are equal would each produce a menu
    /// whose rows cannot be told apart at the click.
    #[test]
    fn the_ids_are_distinct_and_shaped_like_ids() {
        let entries = entries();
        assert_ne!(entries[0].id, entries[1].id, "two rows, one id");
        for entry in entries {
            assert!(!entry.id.is_empty(), "an empty id cannot be matched");
            assert!(
                !entry.id.contains(char::is_whitespace),
                "{:?} contains whitespace",
                entry.id
            );
        }
    }

    /// A disabled row would be a menu entry whose only job is to say it does
    /// nothing. The reference enables both; this says so rather than inheriting a
    /// default that could change under it.
    #[test]
    fn every_row_is_clickable() {
        for entry in entries() {
            assert!(entry.enabled, "{} is greyed out", entry.id);
        }
    }

    /// The one thing a data list cannot check by itself: that the keys it names
    /// are keys the table actually has. A typo here is an empty menu label, which
    /// `tray-icon` renders as a blank row rather than an error.
    #[test]
    fn every_label_key_is_in_the_english_table() {
        for entry in entries() {
            assert!(
                crate::i18n::ENGLISH
                    .iter()
                    .any(|(key, _)| *key == entry.label_key),
                "{} is not a key the English table has",
                entry.label_key
            );
        }
    }

    /// The two keys are the reference's own spellings, so the 49 translated packs
    /// carry them and the menu is translated on a machine that is not English.
    #[test]
    fn the_label_keys_are_the_reference_spellings() {
        let keys: Vec<&str> = entries().iter().map(|entry| entry.label_key).collect();
        assert_eq!(keys, ["tray.show_window", "tray.exit"]);
    }

    /// The tooltip is the one piece of tray text that is not a menu label, and it
    /// is what a reader sees on hover before opening anything.
    #[test]
    fn the_tooltip_is_the_name_and_the_version() {
        assert_eq!(tooltip("WSA Dashboard", "0.1.0"), "WSA Dashboard v0.1.0");
        assert!(
            tooltip("WSA Dashboard", "0.1.0").contains("v0.1.0"),
            "the version is what tells two builds apart on the same machine"
        );
    }
}
