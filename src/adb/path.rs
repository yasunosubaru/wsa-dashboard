// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Which adb binary to run.
//!
//! Lives here rather than in `wsadash.rs` because both `[[bin]]` trees need
//! it: the CLI resolves it once, the GUI must resolve it too or the three-tier
//! screenshots cannot point the app at a stand-in adb (docs/MAPPING.md, spec
//! section 5).

use std::ffi::OsString;
use std::path::PathBuf;

/// Which adb to run.
///
/// `WSADASH_ADB` first: platform-tools is not guaranteed on PATH (Review
/// Focus #1), a machine may carry several, and pointing at a stand-in is the
/// only way to exercise either binary without a real device.
pub fn adb_path() -> PathBuf {
    resolve_adb(std::env::var_os("WSADASH_ADB"))
}

/// Pure half of [`adb_path`], so the rules are testable without touching the
/// environment the test runner happens to carry.
fn resolve_adb(env_value: Option<OsString>) -> PathBuf {
    env_value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("adb"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    /// The environment override wins: a machine may carry several adbs, and
    /// pointing at a stand-in is the only way to exercise either binary
    /// without a real device.
    #[test]
    fn resolve_adb_prefers_the_environment_value() {
        let value = OsString::from("C:/x/adb.exe");
        assert_eq!(
            resolve_adb(Some(value)),
            PathBuf::from("C:/x/adb.exe"),
            "a set WSADASH_ADB must be used verbatim"
        );
    }

    /// An empty override is the same as no override: a blank value in the
    /// environment must not turn into an empty command name.
    #[test]
    fn resolve_adb_treats_an_empty_value_as_unset() {
        let empty = OsString::from("");
        assert_eq!(resolve_adb(Some(empty)), PathBuf::from("adb"));
    }

    /// With no override, fall back to plain `adb` and let the OS resolve it
    /// through PATH.
    #[test]
    fn resolve_adb_falls_back_to_adb_on_path() {
        assert_eq!(resolve_adb(None), PathBuf::from("adb"));
    }

    /// The public entry point reads the real environment. Testing the pure
    /// part above keeps this from becoming a test about whoever ran it.
    #[test]
    fn adb_path_reads_wsadash_adb() {
        let path = adb_path();
        assert!(!path.as_os_str().is_empty());
    }
}
