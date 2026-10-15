// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// `set_dashboard_autostart`, `apply_tray_autostart`, `read_autostart` and the
// registry primitives are called by `runner.rs` in the GUI build. Under the
// CLI's `#[cfg(test)] #[path]` mount of this module only the tests are compiled
// against it -- the four pure functions they cover -- so the rest is
// unreachable by construction from there, not dead. Same reason and same shape
// as `scheduler_task/models.rs` and `config/mod.rs`.
#![allow(dead_code)]

//! Starting with Windows, which is a value under one registry key.
//!
//! The counterpart of the reference's `src/app/autostart.rs` (144 lines), which
//! reads the `Run` key back after writing it, deletes it, or repairs the path
//! when the executable has moved. All of that is still here; none of it was
//! rewritten.
//!
//! # The one naming difference
//!
//! The reference writes its value under the name `"WSLDashboard"`. Ours is
//! [`APP_ID`](super::constants::APP_ID) -- `"wsadashboard"`. This is not a
//! cosmetic choice: the `Run` key is shared by every program on the machine, and
//! a value named `WSLDashboard` in it would point at *this* executable. A reader
//! who had used the reference before would find an entry for a program that is
//! not installed, and an uninstaller that deleted `WSLDashboard` would leave ours
//! behind to launch at every logon.
//!
//! Q12 says identifiers keep the reference's names. This one is the reference's
//! *product* name rather than an identifier of ours, and the same rule carried
//! over produces exactly the collision above. MAPPING 16.2 records it.

use tracing::{debug, info, warn};

/// A Rust string as a null-terminated wide string, for every Win32 call that
/// wants one.
///
/// One function rather than the same three lines at each call site: the
/// terminator is the part that is easy to leave off, and the failure it causes is
/// a registry write of a truncated string rather than an error.
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The `Run` key under `HKEY_CURRENT_USER` -- per-user, so it needs no elevation.
///
/// `HKLM` would run at logon for every user and needs an administrator. The
/// reference uses `HKCU` too, and it is what the tray switches mean by
/// "automatically start".
pub fn run_subkey() -> &'static str {
    r"Software\Microsoft\Windows\CurrentVersion\Run"
}

/// The value name in that key.
pub fn run_value_name() -> &'static str {
    super::constants::APP_ID
}

/// The command line a Run entry should hold for this build.
///
/// Quoted, because the install path contains `Program Files`, and not quoted once
/// every few months: an unquoted path there makes the entry launch whatever
/// `Program.exe` happens to resolve to, and the reader sees nothing happen.
///
/// `--minimized` is appended when the tray switch asks for it. The flag is this
/// program's own and `tray_start_minimized` is what sets it -- which is why this
/// function takes it rather than reading the file itself.
pub fn autostart_command(exe_path: &str, start_minimized: bool) -> String {
    let mut command = format!("\"{exe_path}\"");
    if start_minimized {
        command.push_str(" --minimized");
    }
    command
}

/// Whether the Run entry should be written at all.
///
/// `false` when the existing entry already says the same thing. Writing an
/// identical value on every launch is what makes a registry value's modified
/// stamp lie -- and it is why the reference has a separate `repair_autostart_path`
/// pass that has to run at all.
pub fn should_write_autostart(current: Option<&str>, exe_path: &str, start_minimized: bool) -> bool {
    match current {
        Some(existing) => existing != autostart_command(exe_path, start_minimized),
        None => true,
    }
}

/// Set or clear the entry. Returns whether anything was written.
pub fn set_dashboard_autostart(
    enable: bool,
    start_minimized: bool,
) -> Result<bool, std::io::Error> {
    let Ok(exe_path) = std::env::current_exe() else {
        // Signalled at `warn` by the caller: without the path there is nothing
        // that could be written, and this is not the reader's mistake.
        return Err(std::io::Error::other("current_exe() failed"));
    };
    let exe_path = exe_path.to_string_lossy().into_owned();

    let current = read_autostart()?;
    if !enable {
        if current.is_none() {
            return Ok(false);
        }
        debug!("clearing the Run entry");
        return Ok(delete_autostart()?);
    }

    if !should_write_autostart(current.as_deref(), &exe_path, start_minimized) {
        debug!("the Run entry already says the right thing");
        return Ok(false);
    }

    let command = autostart_command(&exe_path, start_minimized);
    debug!(command = %command, "writing the Run entry");
    write_autostart(&command)?;
    Ok(true)
}

/// What the Run entry says now.
pub fn read_autostart() -> Result<Option<String>, std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        let key = open_key(KEY_READ.0)?;
        let Some(key) = key else {
            return Ok(None);
        };
        let value = wide(run_value_name());

        // Two passes, because the size is not known in advance: the first sizes
        // the buffer, the second fills it. That is what the null data pointer is
        // for, and the reason a one-pass read with a fixed buffer silently
        // truncates a long path instead of failing.
        let mut bytes: u32 = 0;
        let mut kind = windows::Win32::System::Registry::REG_VALUE_TYPE::default();
        let _ = unsafe {
            windows::Win32::System::Registry::RegQueryValueExW(
                key,
                windows::core::PCWSTR(value.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut bytes),
            )
        };
        if bytes == 0 {
            return Ok(None);
        }
        // A value in the Run key that is not a string is not ours to interpret,
        // and handing it back as a command line would compare a DWORD against a
        // path and call that a change.
        if kind != windows::Win32::System::Registry::REG_SZ {
            return Ok(None);
        }

        let mut buffer = vec![0u8; bytes as usize];
        let filled = unsafe {
            windows::Win32::System::Registry::RegQueryValueExW(
                key,
                windows::core::PCWSTR(value.as_ptr()),
                None,
                Some(&mut kind),
                Some(buffer.as_mut_ptr()),
                Some(&mut bytes),
            )
        };
        close_key(key);
        if filled.is_err() || bytes < 2 {
            return Ok(None);
        }
        buffer.truncate(bytes as usize);
        let text: Vec<u16> = buffer
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        Ok(Some(String::from_utf16_lossy(&text).trim_end_matches('\0').to_string()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(None)
    }
}

/// Write the value, creating the key if the machine has never had one.
fn write_autostart(command: &str) -> Result<(), std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        let key = open_creating_key(write_access())?;
        let value = wide(run_value_name());
        let text = wide(command);
        let bytes = unsafe {
            std::slice::from_raw_parts(text.as_ptr() as *const std::ffi::c_void, text.len() * 2)
        };
        let written = unsafe {
            windows::Win32::System::Registry::RegSetKeyValueW(
                key,
                None,
                windows::core::PCWSTR(value.as_ptr()),
                windows::Win32::System::Registry::REG_SZ.0,
                Some(bytes.as_ptr() as *const std::ffi::c_void),
                (text.len() * 2) as u32,
            )
        };
        close_key(key);
        if written.is_err() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = command;
        Ok(())
    }
}

/// Delete the value.
///
/// `false` when it was not there, which is the state a delete asked for anyway:
/// `set_dashboard_autostart` reads the entry first, but a value can vanish
/// between the read and this call, and a race that produced an error would be a
/// race the reader could do nothing about.
fn delete_autostart() -> Result<bool, std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        // `KEY_WOW64_64KEY`: a 64-bit build writing the 64-bit view is what the
        // reader sees in Task Manager's startup tab, and writing the 32-bit view
        // instead would create an entry this program cannot read back. The
        // reference passes no such flag.
        let key = open_key(write_access())?;
        let Some(key) = key else {
            return Ok(false);
        };
        let value = wide(run_value_name());
        let deleted = unsafe {
            windows::Win32::System::Registry::RegDeleteValueW(
                key,
                windows::core::PCWSTR(value.as_ptr()),
            )
        };
        close_key(key);
        Ok(deleted.is_ok())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

// -- the registry primitives, one place each -----------------------------------
//
// Every function below repeats the same open/create/close skeleton, and the
// skeleton is what the tests cannot reach. It is written out three times rather
// than hidden behind an owning type, because a type that owns a `HKEY` needs a
// `Drop` that closes it, and a `Drop` needs `unsafe impl Send`, and neither buys
// anything here: the key is open for the length of exactly one call.
//
// Only the names used bare are imported. The four registry calls that take a
// value are written out fully qualified (`windows::Win32::System::Registry::`)
// so that a reader sees the whole API name at the call site, which is where it
// matters, rather than a short name next to an `unsafe` block.
#[cfg(target_os = "windows")]
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, REG_SAM_FLAGS, HKEY, HKEY_CURRENT_USER,
    KEY_READ,
};

/// `KEY_READ | KEY_WRITE`, widened so a 64-bit build writes the same value it can
/// read back.
#[cfg(target_os = "windows")]
fn write_access() -> u32 {
    (KEY_READ | windows::Win32::System::Registry::KEY_WRITE).0
        | windows::Win32::System::Registry::KEY_WOW64_64KEY.0
}

/// The Windows API's own `HKEY`, which is a `HANDLE` here and not an owned type.
#[cfg(target_os = "windows")]
type Key = HKEY;

/// Open the key. `Ok(None)` when it is not there, which is an answer rather than
/// a failure: a machine that has never set a Run entry has no key to find.
#[cfg(target_os = "windows")]
fn open_key(access: u32) -> Result<Option<Key>, std::io::Error> {
    let mut key = HKEY::default();
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(wide(run_subkey()).as_ptr()),
            Some(0),
            REG_SAM_FLAGS(access),
            &mut key,
        )
    };
    if opened.is_err() {
        return Ok(None);
    }
    Ok(Some(key))
}

/// Open the key, creating it if it is not there.
#[cfg(target_os = "windows")]
fn open_creating_key(access: u32) -> Result<Key, std::io::Error> {
    let mut key = HKEY::default();
    let created = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(wide(run_subkey()).as_ptr()),
            Some(0),
            None,
            windows::Win32::System::Registry::REG_OPTION_NON_VOLATILE,
            REG_SAM_FLAGS(access),
            None,
            &mut key,
            None,
        )
    };
    if created.is_err() {
        return Err(std::io::Error::last_os_error());
    }
    Ok(key)
}

#[cfg(target_os = "windows")]
fn close_key(key: Key) {
    let _ = unsafe { RegCloseKey(key) };
}

/// Called at startup, after the tray switches have been read.
///
/// Returns whether anything changed, purely so the caller can log once per boot
/// rather than once per read.
pub async fn apply_tray_autostart(autostart: bool, start_minimized: bool) -> bool {
    match set_dashboard_autostart(autostart, start_minimized) {
        Ok(changed) => {
            if changed {
                info!(autostart, start_minimized, "Run entry updated");
            }
            changed
        }
        Err(error) => {
            // `warn` and not `error`: a machine whose Run key cannot be written is
            // unusual but recoverable, and the reader can set the switch by hand.
            warn!(%error, "could not set the Run entry");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_value_name_is_the_app_id_and_not_the_reference_product_name() {
        assert_eq!(run_value_name(), "wsadashboard");
        assert_ne!(
            run_value_name(),
            "WSLDashboard",
            "a Run entry named for the reference would point at this executable"
        );
    }

    #[test]
    fn the_subkey_is_the_per_user_run_key() {
        assert_eq!(run_subkey(), r"Software\Microsoft\Windows\CurrentVersion\Run");
    }

    /// `Program Files` is in the install path. An unquoted command line there
    /// launches `Program.exe`, and the reader sees a logon that does nothing.
    #[test]
    fn the_command_line_is_quoted_because_the_install_path_has_spaces() {
        let command = autostart_command(r"C:\Program Files\WSA Dashboard\wsadashboard.exe", false);
        assert_eq!(command, r#""C:\Program Files\WSA Dashboard\wsadashboard.exe""#);
        assert!(command.starts_with('"'), "must be quoted");
        assert!(command.ends_with('"'), "must be quoted");
    }

    #[test]
    fn the_minimised_flag_is_appended_only_when_asked_for() {
        let exe = r"C:\apps\wsadashboard.exe";
        assert_eq!(autostart_command(exe, false), r#""C:\apps\wsadashboard.exe""#);
        assert_eq!(
            autostart_command(exe, true),
            r#""C:\apps\wsadashboard.exe" --minimized"#,
            "the tray's start-minimised switch is what this flag is for"
        );
    }

    /// A write that changes nothing still touches the registry's modified stamp,
    /// and that is why the reference has a separate repair pass.
    #[test]
    fn an_entry_that_already_says_the_right_thing_is_not_written() {
        let exe = r"C:\apps\wsadashboard.exe";
        let with_dash = r#""C:\apps\wsadashboard.exe" --minimized"#;
        let plain = r#""C:\apps\wsadashboard.exe""#;
        assert!(should_write_autostart(None, exe, false), "nothing there yet");
        assert!(!should_write_autostart(Some(plain), exe, false), "identical value");
        assert!(should_write_autostart(Some(with_dash), exe, false), "switch went off");
        assert!(should_write_autostart(Some(plain), exe, true), "switch went on");
        assert!(
            should_write_autostart(Some(r#""C:\old\wsadashboard.exe""#), exe, false),
            "the executable moved"
        );
    }

    /// An empty command is not a command, and the Run key is a place an empty
    /// string can genuinely end up -- a value left behind by a failed install.
    #[test]
    fn an_empty_entry_is_not_what_this_program_would_write() {
        assert!(should_write_autostart(Some(""), r"C:\apps\a.exe", false));
    }

    /// What we would write is, by definition, already written.
    #[test]
    fn the_command_round_trips_through_the_decision() {
        let exe = r"C:\Program Files\WSA Dashboard\wsadashboard.exe";
        for minimised in [false, true] {
            let written = autostart_command(exe, minimised);
            assert!(
                !should_write_autostart(Some(&written), exe, minimised),
                "what we would write is, by definition, already written"
            );
        }
    }
}
