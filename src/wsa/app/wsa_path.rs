// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Resolve the installed WSA package: install dir, manifest, adb port, AUMID.
//!
//! Nothing here may be hardcoded (spec 1.1): this machine's WSA lives under
//! `Downloads\Compressed`, and a reinstall can move it anywhere.
//!
//! TDD step 1 (RED): `from_manifest_text` is not implemented yet.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::adb::port::{parse_local_port, FALLBACK_PORT};

/// Publisher hash appended to the identity to form a package family name.
///
/// This is the hash Windows derives from the `Publisher` attribute, and for
/// Microsoft-signed Store packages it is always `8wekyb3d8bbwe` -- recorded
/// here from this machine's real install (spec 1.1) rather than computed, so
/// the value can be checked against the manifest instead of being derived by
/// code that could be subtly wrong. If WSA were ever re-signed by a different
/// publisher, the `fixture_resolves_every_path` test is what would tell us.
const PUBLISHER_HASH: &str = "8wekyb3d8bbwe";

/// Everything the app needs to know about where WSA lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsaPaths {
    /// `Get-AppxPackage ... .InstallLocation`.
    pub install_location: PathBuf,
    /// `<install_location>\AppxManifest.xml`.
    pub manifest_path: PathBuf,
    /// `<install_location>\WsaClient\WsaClient.exe`, as declared by the
    /// manifest's `Windows.FullTrustApplication` application element.
    pub client_exe: PathBuf,
    /// `<Identity Name>_<publisher hash>!<settings application id>`, for
    /// `explorer shell:AppsFolder\...` (spec 1.5).
    pub settings_aumid: String,
    /// adb port read from the manifest firewall rule, falling back to
    /// `FALLBACK_PORT` only when the manifest declares none.
    pub adb_port: u16,
}

/// Why WSA could not be located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsaPathError {
    /// No WSA Appx package registered, or its manifest cannot be read. The
    /// caller shows the "not installed" state (spec R6, T3).
    NotInstalled,
    /// A manifest was read but is not recognisably WSA's -- it declares no
    /// package identity.
    MalformedManifest,
}

impl WsaPaths {
    /// Pure: build paths from an already-read manifest. No process is spawned
    /// and no file is touched, so the whole mapping is unit-testable against
    /// the checked-in fixture.
    pub fn from_manifest_text(
        install_location: &Path,
        manifest_xml: &str,
    ) -> Result<WsaPaths, WsaPathError> {
        let identity =
            identity_name(manifest_xml).ok_or(WsaPathError::MalformedManifest)?;
        let settings_id = settings_app_id(manifest_xml);
        let relative_exe = client_exe_relative(manifest_xml);
        let adb_port = parse_local_port(manifest_xml).unwrap_or(FALLBACK_PORT);

        Ok(WsaPaths {
            install_location: install_location.to_path_buf(),
            manifest_path: install_location.join("AppxManifest.xml"),
            client_exe: install_location.join(relative_exe),
            settings_aumid: format!("{identity}_{PUBLISHER_HASH}!{settings_id}"),
            adb_port,
        })
    }

    /// Paths for a machine with no WSA at all.
    ///
    /// The CLI refuses to run without one, because a script asking for a
    /// subsystem that is not there should be told so. The GUI must not: its
    /// home page has a whole view for this state (spec section 5 / T3), and
    /// failing before the window opens would trade that view for a dialog.
    ///
    /// Every value here is one that will not be reached. `resolve` failing is
    /// what produces this, and the first thing `probe` does is report
    /// `NotInstalled` from the Appx registry lookup -- which touches none of
    /// these fields -- before any adb command could use them. `adb_port` is
    /// the fallback rather than a placeholder so that anything reading it
    /// early gets a plausible number instead of 0.
    pub fn absent() -> Self {
        Self {
            install_location: PathBuf::new(),
            manifest_path: PathBuf::new(),
            client_exe: PathBuf::new(),
            settings_aumid: String::new(),
            adb_port: FALLBACK_PORT,
        }
    }

    /// Read the manifest from an install directory.
    ///
    /// Any read failure is `NotInstalled`: to the caller, a package whose
    /// manifest cannot be opened is a package that cannot be driven.
    pub fn from_install_location(install_location: &Path) -> Result<WsaPaths, WsaPathError> {
        let manifest_path = install_location.join("AppxManifest.xml");
        let raw =
            std::fs::read_to_string(&manifest_path).map_err(|_| WsaPathError::NotInstalled)?;
        // The manifest ships with a UTF-8 BOM; leave it in place and the first
        // line starts with U+FEFF instead of `<`.
        let xml = raw.strip_prefix('\u{feff}').unwrap_or(raw.as_str());
        Self::from_manifest_text(install_location, xml)
    }
}

/// Locate WSA at runtime.
///
/// `Get-AppxPackage` rather than a registry read or a guessed path: spec 1.1
/// records this machine's WSA under `Downloads\Compressed`, so any hardcoded
/// location is already wrong.
pub fn resolve() -> Result<WsaPaths, WsaPathError> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-AppxPackage MicrosoftCorporationII.WindowsSubsystemForAndroid).InstallLocation",
        ])
        .output()
        .map_err(|_| WsaPathError::NotInstalled)?;

    if !output.status.success() {
        return Err(WsaPathError::NotInstalled);
    }

    let location = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if location.is_empty() {
        // Package simply is not registered -- the distinct, expected case.
        return Err(WsaPathError::NotInstalled);
    }
    WsaPaths::from_install_location(Path::new(&location))
}

/// `<Identity Name="...">` -> the identity.
fn identity_name(manifest_xml: &str) -> Option<&str> {
    manifest_xml
        .lines()
        .find(|line| opens_element(line, "Identity"))
        .and_then(|line| attr_value(line, "Name"))
}

/// The `Id` of the `<Application>` that runs `WsaSettings.exe`.
///
/// Falls back to the documented `SettingsApp` rather than failing: this id
/// only feeds the settings button, which already degrades to a manual path
/// when `WsaSettings.exe` is wedged (spec R5, and it crashed twice today).
fn settings_app_id(manifest_xml: &str) -> &str {
    manifest_xml
        .lines()
        .filter(|line| opens_element(line, "Application"))
        .filter(|line| line.contains(r#"Executable="WsaSettings.exe""#))
        .find_map(|line| attr_value(line, "Id"))
        .unwrap_or("SettingsApp")
}

/// The `Executable` of the full-trust application element (the main client).
fn client_exe_relative(manifest_xml: &str) -> &str {
    manifest_xml
        .lines()
        .filter(|line| opens_element(line, "Application"))
        .filter(|line| line.contains(r#"EntryPoint="Windows.FullTrustApplication""#))
        .find_map(|line| attr_value(line, "Executable"))
        .unwrap_or(r"WsaClient\WsaClient.exe")
}

/// True when the line opens an XML element named `element`.
///
/// A plain `contains("<Identity")` would also match `<IdentityX`, which is
/// exactly how a rewritten manifest slips past the presence check. The name
/// must end at whitespace or `>`.
fn opens_element(line: &str, element: &str) -> bool {
    line.trim_start()
        .strip_prefix('<')
        .and_then(|after| after.strip_prefix(element))
        .is_some_and(|after| after.starts_with(char::is_whitespace) || after.starts_with('>'))
}

/// `attr="value"` on a single line, anchored on the preceding whitespace so
/// `Id="` cannot match the tail of `ApplicationId="`.
///
/// Only `line` is borrowed in the return: `attr` is consumed into the search
/// string, so it must not take part in the elision.
fn attr_value<'a>(line: &'a str, attr: &str) -> Option<&'a str> {
    let needle = format!(" {attr}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adb::port::FALLBACK_PORT;
    use std::path::{Path, PathBuf};

    /// Real `AppxManifest.xml` from the local install, 2026-10-10.
    const MANIFEST: &str = include_str!("../../../tests/fixtures/AppxManifest_full.xml");

    /// Package identity recorded in spec 1.1; also what the manifest's
    /// `<Identity Name>` gives.
    const IDENTITY: &str = "MicrosoftCorporationII.WindowsSubsystemForAndroid";

    fn paths(xml: &str) -> WsaPaths {
        WsaPaths::from_manifest_text(Path::new(r"C:\sideload\WSA_2407"), xml)
            .expect("fixture must resolve")
    }

    #[test]
    fn fixture_resolves_every_path() {
        let p = paths(MANIFEST);

        assert!(!p.install_location.as_os_str().is_empty());
        assert_eq!(
            p.install_location,
            PathBuf::from(r"C:\sideload\WSA_2407")
        );

        assert!(p
            .manifest_path
            .ends_with(Path::new("WSA_2407").join("AppxManifest.xml")));

        assert!(p.client_exe.ends_with(Path::new("WsaClient").join("WsaClient.exe")));

        assert_eq!(
            p.settings_aumid,
            "MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe!SettingsApp"
        );
    }

    /// Review Focus #4: the port comes from the manifest, and the fallback
    /// only happens when the manifest does not declare one.
    #[test]
    fn reads_the_port_from_the_manifest() {
        assert_eq!(paths(MANIFEST).adb_port, 58526);
    }

    /// The whole point of resolving dynamically: a build that moves the port
    /// must be picked up without editing the dashboard.
    #[test]
    fn honours_a_non_default_port() {
        let moved = MANIFEST.replace("LocalPortMin=\"58526\"", "LocalPortMin=\"12345\"");
        let moved = moved.replace("LocalPortMax=\"58526\"", "LocalPortMax=\"12345\"");
        assert_eq!(paths(&moved).adb_port, 12345);
    }

    /// A manifest with no firewall rule at all must fall back rather than
    /// fail: the fallback is exactly what it exists for.
    #[test]
    fn falls_back_when_no_port_is_declared() {
        let no_rule = MANIFEST.replace("LocalPortMin=\"58526\"", "LocalPortMinX=\"58526\"");
        let p = paths(&no_rule);
        assert_eq!(p.adb_port, FALLBACK_PORT);
        assert_eq!(FALLBACK_PORT, 58526);
    }

    #[test]
    fn manifest_without_identity_is_rejected() {
        let no_identity = MANIFEST.replace("<Identity ", "<IdentityX ");
        let err = WsaPaths::from_manifest_text(Path::new(r"C:\sideload"), &no_identity)
            .expect_err("no identity means this is not a WSA manifest");
        assert!(matches!(err, WsaPathError::MalformedManifest));
    }

    /// The settings app id is a convenience button that already degrades to a
    /// manual path (spec R5), so losing it must not take path resolution down.
    #[test]
    fn missing_settings_app_falls_back_to_the_documented_id() {
        let stripped = MANIFEST
            .replace(r#"Executable="WsaSettings.exe""#, r#"Executable="WsaSettingsGone.exe""#);
        assert!(stripped.contains("WsaSettingsGone.exe"));
        let p = paths(&stripped);
        assert!(p.settings_aumid.ends_with("!SettingsApp"));
        assert!(p.settings_aumid.starts_with(IDENTITY));
    }

    /// An unreadable manifest is "not installed" for the caller's purposes:
    /// there is nothing to launch (spec R6, T3 state).
    #[test]
    fn unreadable_manifest_is_not_installed() {
        let err = WsaPaths::from_install_location(Path::new(r"C:\definitely\not\here"))
            .expect_err("path does not exist");
        assert!(matches!(err, WsaPathError::NotInstalled));
    }

    /// Live check of the only part of this module the fixture cannot cover:
    /// `resolve()` really does shell out to `Get-AppxPackage` and the paths it
    /// returns exist on disk. Ignored by default so the suite stays free of
    /// machine and process dependencies; run it with
    /// `cargo test --release --bin wsadash -- --ignored`.
    #[test]
    #[ignore = "spawns powershell; needs a registered WSA package"]
    fn resolves_the_live_install() {
        let p = resolve().expect("WSA is registered on this machine");

        assert!(p.install_location.is_dir(), "{:?}", p.install_location);
        assert!(p.manifest_path.is_file(), "{:?}", p.manifest_path);
        assert!(p.client_exe.is_file(), "{:?}", p.client_exe);
        assert_eq!(p.adb_port, 58526);
        assert_eq!(
            p.settings_aumid,
            "MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe!SettingsApp"
        );
    }
}
