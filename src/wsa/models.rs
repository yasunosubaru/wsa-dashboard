// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Domain models shared by the CLI (`wsadash`) and the GUI.
//!
//! Bin-only crate, so these live here rather than in a lib target -- see the
//! note in docs/superpowers/plans/2026-10-10-p0-p1-skeleton-and-domain.md
//! Task 3 for why a lib target would break line-by-line diffing against the
//! reference.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// One installed Android package, as produced by the `pm list packages`
/// decoder and enriched by `dumpsys package`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    /// Fully qualified package name, e.g. `com.topjohnwu.magisk`.
    pub package: String,
    /// `versionName` as reported by the platform; empty when unavailable.
    pub version_name: String,
    /// `versionCode`; 0 when unavailable.
    pub version_code: i64,
    /// APK size in bytes; 0 when unavailable.
    pub apk_size_bytes: u64,
    /// True for `/system`, `/vendor` and other read-only partitions.
    pub is_system: bool,
}

impl AppInfo {
    /// Human-readable name derived from the package name.
    ///
    /// The platform does not expose an application label (`pm list packages`
    /// and `dumpsys` both return the package name only -- spec 1.4 / risk R3),
    /// so the name is regularised from the package name itself.
    ///
    /// Rule (spec 1.7):
    ///   1. take the last `.`-separated segment (`com.topjohnwu.magisk` -> `magisk`)
    ///   2. split it on `_` and `.` (the final segment may still contain `_`)
    ///   3. uppercase the first letter of each non-empty word, leave the rest as-is
    ///   4. join with a single space
    ///
    /// This name also seeds the Q29 placeholder block: its first letter is drawn
    /// on a solid colour chosen by hashing the full package name.
    pub fn display_name(&self) -> String {
        display_name_of(&self.package)
    }
}

/// [`AppInfo::display_name`] as a function of the one field it reads.
///
/// Pulled out because the handler that names the app in a busy toast has a
/// package string, not an `AppInfo`, and building a five-field struct to read
/// one of them back out is a shape that invites getting the others wrong.
pub fn display_name_of(package: &str) -> String {
    let last = package.rsplit('.').next().unwrap_or("");
    last.split(['_', '.'])
        .filter(|word| !word.is_empty())
        .map(capitalize_first)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Uppercase only the first character; the remainder is left untouched so that
/// already-cased names such as `Magisk` are not mangled into `Magisk`'s tail.
fn capitalize_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Adb transport state, from `adb devices`.
///
/// Note the deliberate difference in granularity from [`SubsystemState`]:
/// this describes *any* adb device, while `SubsystemState` is WSA-specific
/// and folds "no adb entry at all" into [`SubsystemState::Stopped`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdbDeviceState {
    Device,
    Offline,
    Unauthorized,
}

/// One entry from `adb devices -l`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// e.g. `127.0.0.1:58526`.
    pub serial: String,
    pub state: AdbDeviceState,
    /// From the `model:` field of `devices -l`; empty when not reported.
    pub model: String,
    /// From the `product:` field of `devices -l`; empty when not reported.
    pub product: String,
}

/// Readiness of the Android subsystem itself.
///
/// Derived from two independent probes (spec 1.2 / 1.4):
///   * is `127.0.0.1:<port>` present in `adb devices` as `device`?
///   * does `adb shell getprop sys.boot_completed` read back `1`?
///
/// `sys.boot_completed` is the health signal; `mCurrentFocus` is NOT usable
/// (spec 1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubsystemState {
    /// WSA package / manifest not found at all.
    NotInstalled,
    /// Installed, but adb has no usable entry (stopped, or port not listening).
    Stopped,
    /// adb is up but `sys.boot_completed` is not yet `1`.
    Booting,
    /// adb up and boot completed. Only this counts as ready.
    Ready,
    /// adb reports `unauthorized`; the user must accept the key prompt.
    Unauthorized,
    /// adb has an entry for the endpoint but no working session with it.
    ///
    /// Spec 1.3 measured this as what killing `WsaClient.exe` leaves behind,
    /// before the entry ages out entirely (spec 1.4). It is deliberately not
    /// folded into [`SubsystemState::Stopped`]: spec 5 puts a stale entry in
    /// the T2 tier while `Stopped` is T3, and pressing start on a subsystem
    /// that is already running does nothing useful, so the two must not
    /// render the same. Reconnection is automatic rather than a button —
    /// `probe()` reconnects an absent entry, and the 5 s poll retries it.
    Offline,
}

/// spec 5's three tiers.
///
/// The reference has no counterpart: it reads a `Running`/`Stopped` column
/// straight out of `wsl -l -v` and shows nothing when the list empties, so it
/// only ever has two conditions. We have to name a third, because `Booting`,
/// `Unauthorized` and `Offline` are all "wait and retry" while `Stopped` is
/// "start it", and the home page must not render those identically.
///
/// The tiers are deliberately *not* one-to-one with [`SubsystemState`]: they
/// describe how the UI degrades, and three different states degrade the same
/// way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationTier {
    /// Usable. The home page renders the app list exactly as the reference
    /// does, with no degraded element at all.
    Ready,
    /// Waiting on something: boot, a key prompt, or a reconnecting endpoint.
    /// A stale snapshot may still be shown, greyed.
    Partial,
    /// Not usable and not going to become usable by waiting: stopped or not
    /// installed. Offers the start action.
    Down,
}

impl SubsystemState {
    /// Which of spec 5's three tiers this state renders as.
    ///
    /// `Offline` is `Partial` rather than `Down` for the reason recorded on
    /// the variant: a stale entry is still a subsystem that is *running*, and
    /// pressing start on it does nothing while it ages out.
    pub fn tier(&self) -> DegradationTier {
        match self {
            SubsystemState::Ready => DegradationTier::Ready,
            SubsystemState::Booting
            | SubsystemState::Unauthorized
            | SubsystemState::Offline => DegradationTier::Partial,
            SubsystemState::Stopped | SubsystemState::NotInstalled => DegradationTier::Down,
        }
    }
}

/// Combined subsystem readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubsystemStatus {
    pub state: SubsystemState,
    /// Resolved from the WSA manifest `LocalPort`, falling back to 58526.
    pub adb_port: u16,
    /// Raw result of `getprop sys.boot_completed == 1`.
    pub boot_completed: bool,
}

impl SubsystemStatus {
    /// True only for the single state that is actually usable.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, SubsystemState::Ready)
    }
}

/// Filesystem usage for one mount, from `adb shell df`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageInfo {
    /// e.g. `/data` or `/`.
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

impl StorageInfo {
    /// Percentage of `total_bytes` that is used, rounded to whole percent.
    ///
    /// Returns 0 rather than dividing by zero when the platform reports a
    /// zero total.
    pub fn used_percent(&self) -> u64 {
        if self.total_bytes == 0 {
            return 0;
        }
        (self.used_bytes.saturating_mul(100) / self.total_bytes).min(100)
    }
}

/// Direction of an adb port rule.
///
/// Mirrors `adb forward` (host -> device) and `adb reverse` (device -> host).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    Forward,
    Reverse,
}

/// A configured adb port rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortRule {
    /// The port on the side that is *not* the device for forward, and on the
    /// device for reverse.
    pub local_port: u16,
    /// Remote endpoint, e.g. `tcp:8080`. Kept as a string because adb accepts
    /// `tcp:`, `localabstract:`, `localreserved:` and `localfilesystem:`.
    pub remote: String,
    pub direction: PortDirection,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> AppInfo {
        AppInfo {
            package: String::new(),
            version_name: String::new(),
            version_code: 0,
            apk_size_bytes: 0,
            is_system: false,
        }
    }

    /// One assertion per state, because a table-driven test that gets the
    /// expected value wrong hides two mistakes instead of naming one.
    #[test]
    fn ready_is_the_only_ready_tier() {
        assert_eq!(SubsystemState::Ready.tier(), DegradationTier::Ready);
    }

    #[test]
    fn booting_is_partial_because_boot_is_not_finished() {
        assert_eq!(SubsystemState::Booting.tier(), DegradationTier::Partial);
    }

    #[test]
    fn unauthorized_is_partial_while_waiting_for_the_key_prompt() {
        assert_eq!(SubsystemState::Unauthorized.tier(), DegradationTier::Partial);
    }

    #[test]
    fn offline_is_partial_not_down_because_a_stale_entry_needs_reconnect() {
        assert_eq!(SubsystemState::Offline.tier(), DegradationTier::Partial);
    }

    #[test]
    fn stopped_is_down() {
        assert_eq!(SubsystemState::Stopped.tier(), DegradationTier::Down);
    }

    #[test]
    fn not_installed_is_down() {
        assert_eq!(SubsystemState::NotInstalled.tier(), DegradationTier::Down);
    }

    #[test]
    fn display_name_from_last_package_segment() {
        let a = AppInfo {
            package: "com.topjohnwu.magisk".into(),
            version_name: "30.6".into(),
            version_code: 30600,
            apk_size_bytes: 54389465,
            is_system: false,
        };
        assert_eq!(a.display_name(), "Magisk");
        assert!(!a.is_system);
    }

    #[test]
    fn display_name_splits_underscores() {
        let a = AppInfo {
            package: "com.example.my_app".into(),
            ..blank()
        };
        assert_eq!(a.display_name(), "My App");
    }

    #[test]
    fn display_name_of_bare_package_name() {
        let a = AppInfo {
            package: "magisk".into(),
            ..blank()
        };
        assert_eq!(a.display_name(), "Magisk");
    }

    /// An empty package must not panic and must produce an empty name.
    #[test]
    fn display_name_of_empty_package() {
        assert_eq!(blank().display_name(), "");
    }

    /// Spec 1.7 step 3: only the first letter is touched.
    #[test]
    fn display_name_leaves_inner_case_alone() {
        let a = AppInfo {
            package: "com.example.iPood".into(),
            ..blank()
        };
        assert_eq!(a.display_name(), "IPood");
    }

    #[test]
    fn storage_percent_of_zero_total_is_zero() {
        let s = StorageInfo {
            mount_point: "/data".into(),
            total_bytes: 0,
            used_bytes: 0,
            available_bytes: 0,
        };
        assert_eq!(s.used_percent(), 0);
    }

    #[test]
    fn storage_percent_is_capped_at_100() {
        // A `df` line can report used > total once reserved blocks are counted.
        let s = StorageInfo {
            mount_point: "/data".into(),
            total_bytes: 100,
            used_bytes: 105,
            available_bytes: 0,
        };
        assert_eq!(s.used_percent(), 100);
    }

    #[test]
    fn only_ready_counts_as_ready() {
        for (state, ready) in [
            (SubsystemState::NotInstalled, false),
            (SubsystemState::Stopped, false),
            (SubsystemState::Booting, false),
            (SubsystemState::Ready, true),
            (SubsystemState::Unauthorized, false),
            (SubsystemState::Offline, false),
        ] {
            let status = SubsystemStatus {
                state,
                adb_port: 58526,
                boot_completed: state == SubsystemState::Ready,
            };
            assert_eq!(status.is_ready(), ready, "state {state:?}");
        }
    }
}
