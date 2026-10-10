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

use std::collections::HashMap;

use crate::wsa::models::{AppInfo, SubsystemState, SubsystemStatus};

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

/// True when adb's reply contained no `package:` line at all.
///
/// Review Focus #3: a reply we did not understand and a genuinely empty
/// listing are the same bytes on the way out -- zero names either way -- and
/// quietly reporting the first as "no apps installed" is the failure that
/// takes the longest to notice. The UI would draw an empty grid, and the
/// natural conclusion would be that WSA has nothing in it.
///
/// Deliberately about the *reply* rather than about the parsed result:
/// `filter_system` may legitimately remove every name from a well-formed
/// listing, and warning there would cry wolf on a machine that simply has no
/// user-installed apps. An empty reply stays silent for the same reason.
///
/// Returns as soon as one `package:` line is seen, so extra trailing noise
/// does not turn an understood reply into a warning.
pub fn listing_is_unparseable(stdout: &str) -> bool {
    let mut saw_other_content = false;
    for line in stdout.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("package:") {
            return false;
        }
        saw_other_content = true;
    }
    saw_other_content
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

// ---- `getprop` -----------------------------------------------------------

/// Parse `adb shell getprop` stdout into a key/value map.
///
/// One `[key]: [value]` pair per line, which is the only shape the platform
/// emits. Anything else is dropped rather than reported: the probe runs on
/// whatever adb hands back, and a single warning line must not take the
/// health check down.
///
/// Duplicate keys keep the last value -- the platform does not produce any,
/// and last-wins matches how a reader would scan the output.
pub fn parse_props(stdout: &str) -> HashMap<String, String> {
    stdout.lines().filter_map(parse_prop_line).collect()
}

/// `[key]: [value]` -> `(key, value)`.
///
/// Splits on the first `]` so a value that itself contains `]: ` still lands
/// in the value half, then strips exactly one enclosing bracket.
fn parse_prop_line(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix('[')?;
    let key_end = rest.find(']')?;
    let key = &rest[..key_end];
    if key.is_empty() {
        return None;
    }
    let tail = rest[key_end + 1..].strip_prefix(':')?.trim_start();
    let inner = tail.strip_prefix('[')?;
    let value = inner.strip_suffix(']').unwrap_or(inner);
    Some((key.to_string(), value.to_string()))
}

/// Derive subsystem readiness. Pure state derivation -- it issues no commands,
/// so the whole state machine is unit testable.
///
/// Three inputs decide three states:
///   * `adb_online == false` -> `Stopped`. This is the installed-but-not-
///     running case, and it must never be reported as `NotInstalled`: the
///     caller already knows WSA is installed by the time it asks.
///   * online but `sys.boot_completed` is not `1` -> `Booting`.
///   * both -> `Ready`.
///
/// `sys.boot_completed` is the health signal on purpose; `mCurrentFocus` is
/// not usable (spec 1.4).
pub fn subsystem_status(
    props: &HashMap<String, String>,
    adb_online: bool,
    port: u16,
) -> SubsystemStatus {
    let boot_completed = props
        .get("sys.boot_completed")
        .map(|v| v.trim())
        .is_some_and(|v| v == "1");

    let state = if !adb_online {
        SubsystemState::Stopped
    } else if boot_completed {
        SubsystemState::Ready
    } else {
        SubsystemState::Booting
    };

    SubsystemStatus {
        state,
        adb_port: port,
        boot_completed,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_system_package, listing_is_unparseable, parse_package_list};

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

    // -- Review Focus #3: an unreadable reply must not read as "no apps" -----

    #[test]
    fn an_empty_reply_is_not_unparseable() {
        assert!(!listing_is_unparseable(""));
        assert!(!listing_is_unparseable("\n\n   \n"));
    }

    #[test]
    fn a_listing_of_any_length_is_not_unparseable() {
        assert!(!listing_is_unparseable("package:com.a.b\npackage:com.c.d\n"));
        // One understood line is enough: trailing noise does not retroactively
        // make a reply we successfully read into one we did not.
        assert!(!listing_is_unparseable("package:com.a.b\nsomething else\n"));
    }

    #[test]
    fn a_reply_with_no_package_line_is_unparseable() {
        assert!(listing_is_unparseable("error: no devices/emulators found\n"));
        assert!(listing_is_unparseable("adb: unrecognized command\n"));
        assert!(listing_is_unparseable(
            "List of devices attached\n127.0.0.1:58526\toffline\n"
        ));
    }

    #[test]
    fn unparseable_is_about_the_reply_not_about_the_filtered_result() {
        // A well-formed listing whose every name the system heuristic removes
        // must stay silent, or a machine with no user apps would warn on every
        // refresh.
        let only_system = "package:com.android.settings\npackage:com.android.systemui\n";
        assert!(parse_package_list(only_system, true).is_empty());
        assert!(!listing_is_unparseable(only_system));
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

    // ---- Task 7: `getprop` + subsystem state machine ---------------------

    use super::{parse_props, subsystem_status};
    use crate::wsa::models::{SubsystemState, SubsystemStatus};

    /// Subset of real `adb shell getprop` output from the local device,
    /// 2026-10-10. The values matter: the model string contains `(` `)` and
    /// the release value is a bare `13`.
    const GETPROP: &str = "\
[init.svc.zygote]: [running]
[ro.build.type]: [user]
[ro.build.version.release]: [13]
[ro.debuggable]: [0]
[ro.product.model]: [Subsystem for Android(TM)]
[sys.boot_completed]: [1]
";

    #[test]
    fn parses_getprop_pairs() {
        let props = parse_props(
            "[ro.build.type]: [user]\n[ro.debuggable]: [0]\n",
        );
        assert_eq!(props.len(), 2);
        assert_eq!(props["ro.build.type"], "user");
        assert_eq!(props["ro.debuggable"], "0");
    }

    #[test]
    fn parses_real_getprop_output() {
        let props = parse_props(GETPROP);
        assert_eq!(props["sys.boot_completed"], "1");
        assert_eq!(props["ro.product.model"], "Subsystem for Android(TM)");
        assert_eq!(props["ro.build.version.release"], "13");
        assert_eq!(props.len(), 6);
    }

    /// adb output is not guaranteed to be clean -- a warning line, a stray
    /// prompt or an empty line must be skipped, not panic the probe.
    #[test]
    fn malformed_getprop_lines_are_skipped() {
        let props = parse_props("garbage\n\n   \n[truncated]\n[ok.key]: [v]\n[nested]: [a]: [b]\n");
        assert_eq!(props["ok.key"], "v");
        assert_eq!(props.len(), 2);
    }

    #[test]
    fn empty_prop_value_is_kept_as_empty() {
        let props = parse_props("[some.key]: []\n");
        assert_eq!(props["some.key"], "");
        assert!(props.contains_key("some.key"));
    }

    #[test]
    fn empty_input_yields_no_props() {
        assert!(parse_props("").is_empty());
    }

    // -- the state machine. This is Review Focus #2: an installed but
    // -- stopped subsystem must report "not running", never "not installed".

    fn status(adb_online: bool, boot_completed: &str) -> SubsystemStatus {
        let mut props = std::collections::HashMap::new();
        props.insert("sys.boot_completed".to_string(), boot_completed.to_string());
        subsystem_status(&props, adb_online, 58526)
    }

    #[test]
    fn offline_subsystem_is_stopped_not_not_installed() {
        let s = status(false, "");
        assert_eq!(s.state, SubsystemState::Stopped);
        assert!(!s.boot_completed);
        assert!(!s.is_ready());
    }

    #[test]
    fn online_but_boot_not_done_is_booting() {
        assert_eq!(status(true, "").state, SubsystemState::Booting);
        assert_eq!(status(true, "0").state, SubsystemState::Booting);
        assert_eq!(status(true, "yes").state, SubsystemState::Booting);
    }

    #[test]
    fn online_and_boot_done_is_ready() {
        let s = status(true, "1");
        assert_eq!(s.state, SubsystemState::Ready);
        assert!(s.boot_completed);
        assert!(s.is_ready());
    }

    /// Whitespace around the value must not flip the state.
    #[test]
    fn boot_completed_value_is_trimmed() {
        let mut props = std::collections::HashMap::new();
        props.insert("sys.boot_completed".to_string(), " 1 ".to_string());
        let s = subsystem_status(&props, true, 58526);
        assert_eq!(s.state, SubsystemState::Ready);
    }

    #[test]
    fn status_carries_the_resolved_port() {
        assert_eq!(status(false, "").adb_port, 58526);
        assert_eq!(status(true, "1").adb_port, 58526);
        let s = subsystem_status(&std::collections::HashMap::new(), true, 12345);
        assert_eq!(s.adb_port, 12345);
        assert_eq!(s.state, SubsystemState::Booting);
    }
}
