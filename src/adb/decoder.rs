// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Decoders for `adb shell` command output.
//!
//! Every decoder here takes raw stdout and returns plain data, never a
//! `Result` and never a panic: adb output is whatever the device decided to
//! print, and a malformed line must degrade to "skip it", not take the app
//! down.
//!
//! TDD step 1 (RED): `parse_package_list` is not implemented yet.

#![allow(dead_code)]

/// Name prefixes that mark a package as preinstalled rather than user-added.
///
/// These are matched on a full leading segment (`com.android.` including the
/// trailing dot), so `com.androidfoo.bar` is NOT treated as system.
const SYSTEM_PREFIXES: &[&str] = &[
    "com.android.",
    "android.",
    "com.google.android.",
    "com.amazon.",
];

/// Overlay packages the platform generates at build time and never lists as
/// something the user can open.
const GENERATED_OVERLAY_SUFFIX: &str = ".auto_generated_rro_vendor__";

/// Heuristic system-package test on the package name alone.
///
/// The authoritative signal is `pm list packages -s`, but that is a separate
/// round trip; this keeps a single listing call sufficient for the home page
/// filter. Documented as a heuristic for exactly that reason.
pub fn is_system_package(name: &str) -> bool {
    SYSTEM_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        || name.ends_with(GENERATED_OVERLAY_SUFFIX)
}

/// Parse `adb shell pm list packages` stdout into package names.
///
/// Handles both shapes the platform emits:
///   * `package:<name>`                        (`-3`, `-s`, plain)
///   * `package:<apk path>=<name>`             (`-f`)
///
/// Malformed and blank lines are skipped rather than reported: a decoder that
/// returns an error takes the whole list down for one stray line.
///
/// With `filter_system = true` the heuristic system packages are removed, so
/// the home page shows only user-installed apps.
pub fn parse_package_list(stdout: &str, filter_system: bool) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.strip_prefix("package:"))
        .map(package_name_from_entry)
        .filter(|name| !name.is_empty())
        .filter(|name| !filter_system || !is_system_package(name))
        .collect()
}

/// `package:<name>` -> `<name>`; `package:<apk path>=<name>` -> `<name>`.
///
/// Splits on the LAST `=`: the apk path is the part that may legitimately
/// contain odd characters, the package name never contains `=`.
fn package_name_from_entry(entry: &str) -> String {
    match entry.rsplit_once('=') {
        Some((_, name)) => name.trim().to_string(),
        None => entry.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{is_system_package, parse_package_list};

    /// Captured 2026-10-10 from the local device:
    /// `adb shell pm list packages -3`
    const THIRD_PARTY: &str = "\
package:com.topjohnwu.magisk
package:com.nowcent.ham
package:com.microsoft.launcher
";

    /// Same device, `adb shell pm list packages -f`. Note the shape is
    /// `package:<apk path>=<package name>`, not `package:<name>`.
    const WITH_PATHS: &str = "\
package:/product/app/HomeApp/HomeApp.apk=com.microsoft.windows.homeapp
package:/system/app/SecureElement/SecureElement.apk=com.android.se
package:/product/overlay/NavigationBarModeGesturalExtraWideBack/NavigationBarModeGesturalOverlayExtraWideBack.apk=com.android.internal.systemui.navbar.gestural_extra_wide_back
";

    #[test]
    fn parses_third_party_output() {
        assert_eq!(
            parse_package_list(THIRD_PARTY, false),
            vec![
                "com.topjohnwu.magisk",
                "com.nowcent.ham",
                "com.microsoft.launcher"
            ]
        );
    }

    /// The `-f` shape must yield the package name, not the apk path.
    #[test]
    fn parses_output_that_carries_apk_paths() {
        assert_eq!(
            parse_package_list(WITH_PATHS, false),
            vec![
                "com.microsoft.windows.homeapp",
                "com.android.se",
                "com.android.internal.systemui.navbar.gestural_extra_wide_back"
            ]
        );
    }

    #[test]
    fn skips_blanks_and_unprefixed_lines() {
        let noisy = "\n  package:a.b\n\nnot-a-package-line\n   \npackage:c.d\n";
        assert_eq!(parse_package_list(noisy, false), vec!["a.b", "c.d"]);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(parse_package_list("", false).is_empty());
        assert!(parse_package_list("\n\n   \n", true).is_empty());
    }

    #[test]
    fn system_filter_keeps_third_party_only() {
        assert_eq!(
            parse_package_list(WITH_PATHS, true),
            vec!["com.microsoft.windows.homeapp"]
        );
    }

    /// `filter_system = false` must not drop anything.
    #[test]
    fn no_filter_keeps_everything() {
        assert_eq!(parse_package_list(WITH_PATHS, false).len(), 3);
    }

    #[test]
    fn system_package_prefixes_are_detected() {
        for name in [
            "com.android.se",
            "android.hardware.camera",
            "com.google.android.gms",
            "com.amazon.kindle",
        ] {
            assert!(is_system_package(name), "{name} should be system");
        }
        assert!(!is_system_package("com.topjohnwu.magisk"));
        // Prefix match must be on a full segment, not a raw string prefix.
        assert!(!is_system_package("com.androidfoo.bar"));
    }

    #[test]
    fn generated_overlays_are_system() {
        assert!(is_system_package(
            "com.android.internal.systemui.navbar.auto_generated_rro_vendor__"
        ));
        assert!(!is_system_package("com.example.auto_generated_rro_vendor__x"));
    }
}
