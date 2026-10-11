// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! English text, and the two functions the UI calls to get at it.
//!
//! The signatures and the key names are the reference's (`src/i18n/mod.rs`
//! lines 275 and 282), so `.slint` files can carry `AppI18n.t("main.loading",
//! ...)` from the first day and be diffed line by line against the
//! reference at review.
//!
//! This table is deliberately the home page's slice, not the reference's 563
//! keys across 50 languages. P6 swaps this static array for the real
//! `en.toml` loader without touching a single call site -- that is the whole
//! reason the signatures are fixed now.

// Compiled into the CLI only so `cargo test --release --bin wsadash` can run
// these tests; the GUI is the only caller.
#![allow(dead_code)]

/// Resolve one key. A missing key is reported and then shown verbatim: a
/// blank string would hide the problem, and a panic would take the window
/// with it.
pub fn t(key: &str) -> String {
    match lookup(key) {
        Some(value) => value.to_string(),
        None => {
            tracing::warn!(key, "no translation; showing the key itself");
            key.to_string()
        }
    }
}

/// Resolve one key and fill its `{0}`, `{1}`, ... placeholders.
///
/// More arguments than placeholders are ignored and fewer are not an error:
/// a translation that outgrew its caller must not crash the window over it.
pub fn tr(key: &str, args: &[String]) -> String {
    let mut text = t(key);
    for (index, value) in args.iter().enumerate() {
        text = text.replace(&format!("{{{index}}}"), value);
    }
    text
}

/// P6 replaces this with the real table. No language we ship is RTL, and the
/// reference's RTL branch stays compiled but unreachable until one is.
pub fn is_rtl(_lang: &str) -> bool {
    false
}

/// P6 replaces this with the stored preference.
pub fn current_lang() -> String {
    "en".into()
}

fn lookup(key: &str) -> Option<&'static str> {
    ENGLISH
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, value)| *value)
}

/// Key names are the reference's. Values are ours wherever WSA has no
/// counterpart to what the reference labels, and that substitution is what
/// `docs/MAPPING.md` section 4.1 records.
pub static ENGLISH: &[(&str, &str)] = &[
    ("common.app_name", "WSA Dashboard"),
    // -- main: the home page ---------------------------------------------
    ("main.loading", "Loading..."),
    // The reference names a WSL distribution; we list Android packages, so
    // the sentence is ours. Q28.
    ("main.distro_not_found", "No apps to show. Install an app first."),
    ("main.guide_link", "View Installation Guide"),
    // The three-tier degradation (spec section 5). None of these has a
    // reference counterpart: it reads Running/Stopped off `wsl -l -v`.
    ("main.subsystem_booting", "WSA is starting..."),
    (
        "main.subsystem_unauthorized",
        "Waiting for authorization on the device",
    ),
    ("main.subsystem_offline", "Connection lost"),
    ("main.subsystem_stopped", "WSA is not running"),
    ("main.subsystem_not_installed", "WSA is not installed"),
    ("main.start_subsystem", "Start WSA"),
    ("main.stale_data", "Data may be out of date"),
    ("main.no_apps", "No apps to show"),
    ("main.search_placeholder", "Search apps"),
    // -- distro: one card -------------------------------------------------
    ("distro.start", "Start"),
    ("distro.stop", "Stop"),
    ("distro.restart", "Restart"),
    ("distro.compress", "Compress"),
    ("distro.terminal", "Terminal"),
    ("distro.vscode", "VS Code"),
    ("distro.explorer", "Explorer"),
    ("distro.expand", "Expand"),
    ("distro.collapse", "Collapse"),
    ("distro.information", "Information"),
    ("distro.export", "Export"),
    ("distro.clone", "Clone"),
    ("distro.move", "Move"),
    ("distro.delete", "Delete"),
    ("distro.settings", "Settings"),
    ("distro.configs", "Configs"),
    ("distro.running", "Running"),
    ("distro.stopped", "Stopped"),
    // -- operation: the single busy slot -----------------------------------
    ("operation.starting", "Starting ..."),
    ("operation.stopping", "Stopping ..."),
    ("operation.restarting", "Restarting ..."),
    ("operation.deleting", "Deleting ..."),
    ("operation.fetching_info", "Fetching information..."),
    // The reference never launches or installs from its UI, so it has no key
    // for either; both follow its naming and its trailing spaces.
    ("operation.launching", "Launching ..."),
    ("operation.installing", "Installing ..."),
    // -- toast -------------------------------------------------------------
    // The reference's sentence names a distro; ours names an app and drops
    // the lecture, because the home page says it every few seconds while
    // anything is busy.
    ("toast.distro_busy", "{0} is busy ({1})"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_reference_key() {
        assert_eq!(t("distro.running"), "Running");
        assert_eq!(t("distro.start"), "Start");
    }

    #[test]
    fn fills_placeholders_left_to_right() {
        assert_eq!(
            tr("toast.distro_busy", &["Foo".into(), "Starting ...".into()]),
            "Foo is busy (Starting ...)"
        );
    }

    /// A key nobody filled in must not blank the window.
    #[test]
    fn a_missing_key_shows_itself_and_does_not_panic() {
        assert_eq!(t("no.such.key"), "no.such.key");
        assert_eq!(tr("no.such.key", &["a".into()]), "no.such.key");
    }

    /// An extra argument is harmless: a translation that outgrew its caller
    /// must not take the window down.
    #[test]
    fn too_many_arguments_is_not_an_error() {
        assert_eq!(
            tr("distro.start", &["a".into(), "b".into(), "c".into()]),
            "Start"
        );
    }

    #[test]
    fn too_few_arguments_leaves_the_placeholder_visible() {
        assert_eq!(tr("toast.distro_busy", &["Foo".into()]), "Foo is busy ({1})");
    }

    #[test]
    fn no_key_is_empty_or_whitespace() {
        for (key, value) in ENGLISH {
            assert!(!key.trim().is_empty(), "blank key");
            assert!(!value.trim().is_empty(), "{key} has no text");
        }
    }

    /// A key listed twice is a typo waiting to be read as a lookup miss.
    #[test]
    fn every_key_is_listed_once() {
        let mut seen = std::collections::HashSet::new();
        for (key, _) in ENGLISH {
            assert!(seen.insert(*key), "{key} is listed twice");
        }
    }

    #[test]
    fn no_key_is_prefixed_with_a_section_twice() {
        for (key, _) in ENGLISH {
            assert!(
                !key.starts_with("main.main.") && !key.starts_with("distro.distro."),
                "{key} looks like a flattened section was applied twice"
            );
        }
    }

    #[test]
    fn rtl_and_language_are_stubs_until_p6() {
        assert!(!is_rtl("en"));
        assert_eq!(current_lang(), "en");
    }
}
