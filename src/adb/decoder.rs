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

use crate::wsa::models::AppInfo;

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

// ---- `dumpsys package` ------------------------------------------------------

/// Parse `adb shell dumpsys package <name>` stdout into an `AppInfo`.
///
/// Returns `None` when the output does not carry a usable `versionName=`.
/// That single absence signal covers three real cases -- no such package,
/// a `Package [` block we did not recognise, and an apk that declares no
/// version -- and they are all "cannot tell you the version", which is
/// exactly what the caller needs to know. A fabricated default would make
/// them indistinguishable from a real parse.
///
/// `apk_size_bytes` is always 0 here. Deriving it needs a second command
/// (`pm path` plus a `stat`), and a decoder that silently issues adb calls
/// cannot be unit tested or reused; that work belongs to the ops layer.
pub fn parse_package_info(stdout: &str) -> Option<AppInfo> {
    let (package, body) = first_package_block(stdout)?;

    // `versionName` is the presence signal for the whole result.
    let version_name = field_value(&body, "versionName")?.to_string();

    let version_code = field_value(&body, "versionCode")
        .and_then(|raw| raw.split_whitespace().next())
        .and_then(|token| token.parse::<i64>().ok())
        .unwrap_or(0);

    let is_system = system_flags(&body)
        .map(|flags| flags.split_whitespace().any(|w| w == "SYSTEM"))
        // No flags line at all: fall back to the name heuristic rather than
        // defaulting every unknown app to third-party.
        .unwrap_or_else(|| is_system_package(&package));

    Some(AppInfo {
        package,
        version_name,
        version_code,
        apk_size_bytes: 0,
        is_system,
    })
}

/// Locate the first `Package [...]` block under the `Packages:` header.
///
/// Returns the package name and the block body with each line trimmed. The
/// block ends at the next `Package [...]` header or at the first top-level
/// section header (`Queries:`, `Dexopt state:` ...), so fields from a
/// neighbouring package or a following section are never mixed into ours.
fn first_package_block(stdout: &str) -> Option<(String, String)> {
    let mut in_packages_section = false;
    let mut name: Option<String> = None;
    let mut body = String::new();

    for line in stdout.lines() {
        if name.is_none() {
            if !in_packages_section {
                // Only a line that IS the header counts; a stray `Packages:`
                // mention inside prose must not open the section.
                if line.trim() == "Packages:" {
                    in_packages_section = true;
                }
                continue;
            }
            if let Some(found) = package_name_from_header(line) {
                name = Some(found.to_string());
            }
            continue;
        }

        // Inside the target block.
        if package_name_from_header(line).is_some() {
            break; // a later block belongs to another package
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // An unindented line ending in ':' is a new top-level section.
        if !line.starts_with(char::is_whitespace) && trimmed.ends_with(':') {
            break;
        }
        body.push_str(trimmed);
        body.push('\n');
    }

    Some((name?, body))
}

/// `  Package [com.topjohnwu.magisk] (ff14e8b):` -> `com.topjohnwu.magisk`.
///
/// Note `pkg=Package{a2efb68 ...}` does not match: the header needs the
/// bracket, not a brace.
fn package_name_from_header(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("Package [")?;
    let end = rest.find(']')?;
    let name = &rest[..end];
    (!name.is_empty()).then_some(name)
}

/// First `<key>=<value>` line in `body`, trimmed.
///
/// Treats `null` and an empty value as absent: Android writes
/// `versionName=null` for apks that declare none, and showing that literally
/// as an app's version would be wrong.
fn field_value<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    body.lines()
        .find_map(|line| line.trim().strip_prefix(prefix.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
}

/// The `pkgFlags=[ ... ]` / `flags=[ ... ]` line, if either is present.
///
/// `pkgFlags` is preferred: it is the package-level view and is not polluted
/// by per-user flag state. `privateFlags=` never matches, because the match is
/// anchored at the start of the line.
fn system_flags(body: &str) -> Option<&str> {
    let pkg = field_value(body, "pkgFlags");
    pkg.or_else(|| field_value(body, "flags"))
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

    // ---- Task 6: `dumpsys package` --------------------------------------

    use super::parse_package_info;

    /// Real `adb shell dumpsys package com.topjohnwu.magisk`, captured
    /// 2026-10-10. Shared with the CLI tests later on.
    const DUMPSYS: &str = include_str!("../../tests/fixtures/dumpsys_package.txt");

    #[test]
    fn parses_real_dumpsys_output() {
        let info = parse_package_info(DUMPSYS).expect("fixture must parse");
        assert_eq!(info.package, "com.topjohnwu.magisk");
        assert_eq!(info.version_name, "30.6");
        assert_eq!(info.version_code, 30600);
        assert!(!info.is_system);
        // Size is deliberately NOT derived here: it needs a second command
        // (`pm path` + `stat`), which is the ops layer's job.
        assert_eq!(info.apk_size_bytes, 0);
    }

    // The fixture carries `versionCode=30600 minSdk=23 targetSdk=36` on one
    // line, so a naive split would read `30600 minSdk` or the whole tail.
    #[test]
    fn version_code_takes_only_the_first_token() {
        let info = parse_package_info(DUMPSYS).unwrap();
        assert_eq!(info.version_code, 30600);
        assert!(!info.version_name.contains("minSdk"));
    }

    // A package with no `versionName=` must come back as `None`, not as a
    // fabricated `AppInfo` with an empty version -- the caller has to be able
    // to tell "no such package" from "parsed fine".
    #[test]
    fn missing_version_name_is_none_not_a_default() {
        let text = "Packages:\n  Package [com.example.thing] (abc123):\n    versionCode=1 minSdk=21\n    flags=[ HAS_CODE ]\n";
        assert!(parse_package_info(text).is_none());
    }

    // Android prints `versionName=null` when the apk declares none. Showing
    // that literally as the app's version would be wrong.
    #[test]
    fn null_version_name_is_none() {
        let text = "Packages:\n  Package [com.example.thing] (abc123):\n    versionName=null\n    versionCode=1\n";
        assert!(parse_package_info(text).is_none());
    }

    #[test]
    fn absent_packages_section_is_none() {
        assert!(parse_package_info("").is_none());
        assert!(parse_package_info("Unable to find package: 'x'").is_none());
        // `Package [` alone is not enough; the section header must be there.
        assert!(parse_package_info("  Package [com.x.y] (z):\n    versionName=1.0\n").is_none());
    }

    // `dumpsys` can report several packages. The queried one comes first,
    // and we must not blend fields across block boundaries.
    #[test]
    fn reads_only_the_first_package_block() {
        let text = "Packages:\n  Package [com.first.app] (a1):\n    versionName=1.0\n    versionCode=10\n    pkgFlags=[ SYSTEM ]\n  Package [com.second.app] (b2):\n    versionName=2.0\n    versionCode=20\n";
        let info = parse_package_info(text).expect("must parse");
        assert_eq!(info.package, "com.first.app");
        assert_eq!(info.version_name, "1.0");
        assert_eq!(info.version_code, 10);
        assert!(info.is_system);
    }

    // A following top-level section (`Queries:`, `Dexopt state:` ...) must
    // close the block, otherwise its fields get attributed to our package.
    #[test]
    fn a_following_section_closes_the_block() {
        let text = "Packages:\n  Package [com.first.app] (a1):\n    versionName=1.0\n    versionCode=10\nQueries:\n  packageQuery com.other.App\n";
        let info = parse_package_info(text).expect("must parse");
        assert_eq!(info.version_name, "1.0");
        assert!(!info.is_system);
    }

    #[test]
    fn system_flag_marks_is_system() {
        let text = "Packages:\n  Package [com.android.se] (a1):\n    versionName=1.0\n    versionCode=1\n    pkgFlags=[ SYSTEM HAS_CODE ]\n";
        let info = parse_package_info(text).expect("must parse");
        assert!(info.is_system);

        let text = "Packages:\n  Package [com.foo.bar] (a1):\n    versionName=1.0\n    versionCode=1\n    flags=[ HAS_CODE ALLOW_CLEAR_USER_DATA ]\n";
        let info = parse_package_info(text).expect("must parse");
        assert!(!info.is_system);
    }

    // With no flags line at all we still have the package name, so fall back
    // to the name heuristic rather than defaulting every app to third-party.
    #[test]
    fn falls_back_to_the_name_heuristic_without_flags() {
        let text = "Packages:\n  Package [com.android.se] (a1):\n    versionName=1.0\n    versionCode=1\n";
        assert!(parse_package_info(text).unwrap().is_system);

        let text = "Packages:\n  Package [com.topjohnwu.magisk] (a1):\n    versionName=1.0\n    versionCode=1\n";
        assert!(!parse_package_info(text).unwrap().is_system);
    }
}
