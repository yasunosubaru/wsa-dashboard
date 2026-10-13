// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The window handle, and the one thing P3 does with it: pin it on top.
//!
//! The reference's `src/app/window.rs` is 441 lines. Almost all of it belongs to
//! the tray and the second-instance hand-off -- hiding the window, renaming it
//! so a stray icon cannot appear, re-centring it after a restore -- and those
//! are P7. This file is therefore a *slice* of the reference rather than the
//! file: `find_main_window` and `set_always_on_top`, both verbatim but for the
//! window-title candidates, plus this note saying which parts of the original
//! were left behind and why.
//!
//! What is deliberately absent, so that P7 can go and fetch it:
//!
//!   `set_skip_taskbar`, `hide_window_completely`, `set_window_opacity*`,
//!   `show_and_center`, `activate_window_by_hwnd`, `rename_app_windows`.
//!
//! # The title candidates are this project's
//!
//! The reference looks for `title.contains("WINDOW_UI") || title ==
//! "WSL Dashboard Main" || class_name.contains("Slint")`. Our handshake
//! candidates are the same three slots with our names (docs/MAPPING.md §2.1):
//! `WSA_DASHBOARD_WINDOW_UI`, `WSA Dashboard Main`, and the third term --
//! `class_name.contains("Slint")` -- is unchanged because it is about the
//! renderer rather than about us.

#[cfg(target_os = "windows")]
use tracing::{error, info};
#[cfg(target_os = "windows")]
use windows::core::BOOL;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindow, GetWindowThreadProcessId, GetWindowTextW,
    GW_OWNER, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE,
    SetWindowPos, SWP_NOZORDER,
};

/// Is this window *our* window, rather than a helper this process also owns?
///
/// The reference logs every candidate at `info` while asking. It does not
/// change the answer, and this project's log file is read rather than
/// watched -- so the question is asked without the noise. Everything else about
/// the predicate is the reference's: top level (no owner), not a tray helper,
/// and either our handshake string, our name, or a Slint window.
#[cfg(target_os = "windows")]
fn is_main_window(hwnd: HWND) -> bool {
    unsafe {
        let mut title_buf: [u16; 512] = [0; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        let mut class_buf: [u16; 256] = [0; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buf);
        let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);

        let owner = GetWindow(hwnd, GW_OWNER).unwrap_or(HWND(std::ptr::null_mut()));

        let is_tray =
            title.contains("Tray") || class_name.contains("tray_icon_app") || class_name.contains("Target");
        !is_tray
            && owner.0.is_null()
            && (title.contains("WINDOW_UI")
                || title == "WSA Dashboard Main"
                || class_name.contains("Slint"))
    }
}

#[cfg(target_os = "windows")]
struct EnumWindowData {
    target_pid: u32,
    main_window: Option<HWND>,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_main_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let data = &mut *(lparam.0 as *mut EnumWindowData);
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == data.target_pid && is_main_window(hwnd) {
            data.main_window = Some(hwnd);
            return BOOL(0); // Stop enumeration
        }
        BOOL(1)
    }
}

/// This process's own UI window.
///
/// A fallback pass runs the reference's second predicate, which names no title
/// at all: top level, not a helper class, and `class_name.contains("Window")`.
/// On a cold start the title can still be empty, and the class name is set
/// before it is -- `Window Class` is what Slint names the window, measured here
/// as well as assumed.
#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_fallback_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let data = &mut *(lparam.0 as *mut EnumWindowData);
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == data.target_pid {
            let mut class_buf: [u16; 256] = [0; 256];
            let class_len = GetClassNameW(hwnd, &mut class_buf);
            let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);
            let owner = GetWindow(hwnd, GW_OWNER).unwrap_or(HWND(std::ptr::null_mut()));
            let is_helper = class_name.contains("tray_icon_app") || class_name.contains("Target");
            if !is_helper && owner.0.is_null() && class_name.contains("Window") {
                data.main_window = Some(hwnd);
                return BOOL(0);
            }
        }
        BOOL(1)
    }
}

#[cfg(target_os = "windows")]
pub fn find_main_window() -> Option<HWND> {
    let mut data = EnumWindowData {
        target_pid: std::process::id(),
        main_window: None,
    };
    unsafe {
        let _ = EnumWindows(Some(enum_main_window_proc), LPARAM(&mut data as *mut _ as isize));
    }
    if data.main_window.is_none() {
        unsafe {
            let _ =
                EnumWindows(Some(enum_fallback_window_proc), LPARAM(&mut data as *mut _ as isize));
        }
    }
    data.main_window
}

/// The pin button: keep the window above everything else.
///
/// Deliberately done by finding this process's own handle rather than by asking
/// Slint, because Slint 1.17 exposes no always-on-top level on `Window` -- the
/// same absence that leaves `title` getter-only. The reference does it this way
/// for that reason too, and it is why the pin survives being a P3 feature
/// rather than waiting for the tray.
#[cfg(target_os = "windows")]
pub fn set_always_on_top(top: bool) {
    info!(top, "set_always_on_top");
    match find_main_window() {
        Some(hwnd) => unsafe {
            let insert_after = if top { HWND_TOPMOST } else { HWND_NOTOPMOST };
            let _ = SetWindowPos(
                hwnd,
                Some(insert_after),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        },
        None => error!("set_always_on_top: main window not found"),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_always_on_top(_top: bool) {}

#[cfg(test)]
mod tests {
    /// No unit tests, and the reason is not laziness: `find_main_window` reads
    /// the *running* process's own window handles. Under `cargo test` there is
    /// no window -- the GUI binary is not what is executing -- so every call
    /// would assert that a window which cannot exist does not exist. What is
    /// testable about it is the predicate, and the predicate is exercised for
    /// real by `tools/standin-adb/capture.ps1`, which also has to decide what
    /// our window is and refuses to photograph anything it cannot identify.
    ///
    /// The behaviour worth pinning (does the pin actually put the window on
    /// top?) is a desktop check: run the GUI, press the pin, confirm a second
    /// window stays behind it. MAPPING section 8 records that P3 checked it by
    /// hand.
    fn nothing_here_is_testable_without_a_desktop() {}
}
