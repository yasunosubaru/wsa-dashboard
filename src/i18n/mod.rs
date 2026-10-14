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

/// Every language this build can render, in the order the picker shows them.
///
/// One entry until P6. The list is declared beside `ENGLISH` rather than
/// derived from it: a table scan would have to guess at a language from a key
/// prefix, and a wrong guess is a picker option that resolves to nothing.
pub fn available_languages() -> Vec<String> {
    LANGUAGES.iter().map(|lang| (*lang).to_string()).collect()
}

/// P6 adds a row per `.toml` file it finds.
const LANGUAGES: &[&str] = &["en"];

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
    // -- sidebar: the nine entries (P3) -------------------------------------
    // Key names are the reference's; the three whose page means something else
    // here carry our words, which is Q12's whole rule:
    //   add      -> spec 3 maps the reference's "Add an instance" (a WSL distro)
    //               onto "install an APK"
    //   usb      -> mapped onto a device page; it lists adb targets, not WSL USB
    //   mount    -> the reference mounts physical disks into WSL; ours is
    //               storage management (spec 3), so "Storage"
    // `home` and `settings` are the two P3 actually shows; the rest are here so
    // the phases that turn their `show_*` on find them already written.
    ("sidebar.home", "Home"),
    ("sidebar.add", "Install APK"),
    ("sidebar.usb", "Devices"),
    ("sidebar.network", "Network"),
    ("sidebar.scheduler", "Task scheduler"),
    ("sidebar.mount", "Storage"),
    ("sidebar.settings", "Settings"),
    ("sidebar.about", "About"),
    ("sidebar.donate", "Donate"),
    // -- usb: the device page (P4) ------------------------------------------
    // Key names are the reference's own `[usb]` block (Q12: identifiers stay),
    // and the view they label was renamed in spec section 3 because what it
    // shows changed from usbip targets to adb targets. The values are ours:
    //   empty   the reference counts USB devices on the host; we list adb's own targets
    //   attach  the reference attaches a USB device to WSL; we open a session
    //           to a target ("Attach to WSL" -> "Connect" is the same verb in
    //           a different domain, and renaming the key would only lose the
    //           line that ties ours to theirs)
    //   detach  and its opposite
    // `usb.bind`/`unbind` and the four usbip settings keys are deliberately not
    // carried over: there is nothing to bind. See MAPPING section 11.
    ("usb.empty", "No adb targets connected"),
    ("usb.loading", "Loading devices..."),
    ("usb.not_found_error", "adb was not found. Install Android SDK platform-tools, then restart."),
    ("usb.attach", "Connect"),
    ("usb.detach", "Disconnect"),
    // Ours: the reference has no subsystem row to distinguish, because every
    // row of its list is a USB device and none of them is the OS's own.
    ("usb.subsystem", "Windows Subsystem for Android"),
    ("usb.device_info", "Device properties"),
    ("usb.detail", "Properties"),
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
    // The reference's DistroInfo asks for this as its THIRD state -- anything
    // that is neither Running nor Stopped -- but never defines it, so the
    // reference renders the literal string "distro.installing" on screen. The
    // key name is kept (Q12) and the value is ours: for us the third state is
    // an app whose state is unknown because the subsystem is unreachable, and
    // "Installing" would be a claim the data cannot support.
    ("distro.installing", "Unavailable"),
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

    // -- the .slint side -----------------------------------------------------
    //
    // Slint cannot type-check a string, so a typo'd key does not fail the
    // build: it renders the raw key to the user, on a screen nobody is
    // watching until someone reports it. This closes that hole from the other
    // end.

    /// Every `.slint` file under `src/ui`, recursively.
    fn slint_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return found;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(slint_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "slint") {
                found.push(path);
            }
        }
        found
    }

    /// Every key a `.slint` file asks `AppI18n.t` for, in order of appearance.
    fn keys_asked_for(text: &str) -> Vec<String> {
        const NEEDLE: &str = "AppI18n.t(\"";
        let mut keys = Vec::new();
        let mut offset = 0;
        while let Some(found) = text[offset..].find(NEEDLE) {
            let value_start = offset + found + NEEDLE.len();
            let tail = &text[value_start..];
            let end = tail
                .find('"')
                .expect("a quoted key always has a closing quote");
            keys.push(tail[..end].to_string());
            offset = value_start + end;
        }
        keys
    }

    /// A key the UI asks for and the table does not have is the worst version
    /// of the failure above: the window keeps running and shows the key. This
    /// test is the only thing that would notice before a user does.
    #[test]
    fn every_key_a_slint_file_asks_for_is_in_the_table() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("ui");
        assert!(
            root.is_dir(),
            "{} does not exist; the scan below would pass by finding nothing",
            root.display()
        );

        let mut missing = Vec::new();
        let mut scanned = 0;
        for path in slint_files(&root) {
            let text = std::fs::read_to_string(&path).expect("read a .slint file");
            for key in keys_asked_for(&text) {
                scanned += 1;
                if lookup(&key).is_none() {
                    missing.push(format!("{}: {key}", path.display()));
                }
            }
        }

        // Without this the test is one deleted directory away from asserting
        // nothing at all, which is worse than not having it.
        assert!(scanned > 0, "the scan found no AppI18n.t calls");

        assert!(
            missing.is_empty(),
            "keys the UI asks for that the table does not have:\n  {}",
            missing.join("\n  ")
        );
    }

    #[test]
    fn the_key_scanner_reports_what_it_cannot_follow() {
        // The scanner keys on the literal text `AppI18n.t("`, so an alias for
        // that function is invisible to it. Nothing here uses one; if someone
        // introduces one, this documents the hole they opened.
        assert!(keys_asked_for("t(\"main.loading\")").is_empty());

        // It also stops at the first quote it meets rather than reasoning
        // about how Slint parses adjacent strings, so a key written in two
        // pieces would be scanned as only its first piece.
        assert_eq!(keys_asked_for("AppI18n.t(\"a\" \".b\");"), ["a"]);
    }
}
