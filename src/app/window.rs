// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// `find_main_window`, `set_always_on_top` and `activate_window_by_hwnd` are
// called by the GUI build -- the last of them by `single_instance`. Under the
// CLI's `#[cfg(test)] #[path]` mount only the tests are compiled against this
// file, so the handles are unreachable by construction from there, not dead.
// Same reason and same shape as `scheduler_task/models.rs` and `config/mod.rs`.
#![allow(dead_code)]

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
//! What is deliberately absent, and where it went instead:
//!
//!   `activate_window_by_hwnd` and `set_skip_taskbar` were listed here through
//!   P6 and P7 came to fetch them -- the first for `single_instance`, the second
//!   for the tray, which has to be able to take the window off the taskbar and
//!   put it back.
//!
//!   `show_and_center` is **not** here, and that is a deliberate split rather
//!   than an omission. The reference's version takes `&AppWindow` and drives it
//!   (`set_minimized`, `set_position`, `show`); ours lives in
//!   `ui/handlers/window.rs` beside the other callbacks that name the window, so
//!   that this file stays free of the Slint types and the CLI can mount it. See
//!   that file for the three functions the one became.
//!
//!   `set_window_opacity*` and `rename_app_windows` are still absent. The
//!   reference's `show_and_center` needs them for a reveal sequence that fades a
//!   window in from opacity 0; ours reveals in one step through
//!   [`activate_window_by_hwnd`], so there is nothing to fade and no helper
//!   window to rename.
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
use tracing::{error, info, warn};
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

/// Bring this process's window back: visible, on screen, and on top.
///
/// The counterpart of the reference's `activate_window_by_hwnd`
/// (`window.rs`:329-373), which P7 came to collect -- this file's header listed
/// it as deliberately absent from P3 onward. Copied with the same four steps in
/// the same order, and the order matters: a window that was minimised to the tray
/// has both its extended style and its layered alpha changed, and restoring the
/// style *after* showing it leaves a taskbar-less window invisible for a frame.
///
/// The reference does its own window lookup by title before calling this. Ours is
/// handed a handle by `find_main_window`, which finds it by process id -- see
/// `single_instance.rs` for why that replaced the title list.
///
/// One deliberate change inside: the reference centres a parked window on
/// `MONITOR_DEFAULTTOPRIMARY`, and this centres it on `MONITOR_DEFAULTTONEAREST`.
/// Restoring a window the reader had on their second monitor onto the primary one
/// is how a program that "restored" them to the wrong screen gets remembered as
/// broken; `MonitorFromWindow` already knows which monitor the handle belongs to.
#[cfg(target_os = "windows")]
pub fn activate_window_by_hwnd(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::LWA_ALPHA;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, GetWindowRect, SetForegroundWindow, SetLayeredWindowAttributes,
        SetWindowLongW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOP, SWP_FRAMECHANGED,
        SWP_NOMOVE, SWP_NOSIZE, SW_RESTORE, SW_SHOW, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    unsafe {
        // STYLE FIRST: not a tool window, and an app window, so it appears in the
        // taskbar. This is the step a tray-hidden window needs before anything else
        // touches it.
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let style = (style & !WS_EX_TOOLWINDOW.0) | WS_EX_APPWINDOW.0;
        SetWindowLongW(hwnd, GWL_EXSTYLE, style as i32);

        // CLEAR TRANSPARENCY: the tray path sets alpha low, and a restored window
        // that is still faded reads as a rendering bug rather than as a state.
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);

        // RESTORE POSITION: a window parked off-screen sits at around -32000, and
        // restoring it where it was means restoring it where the reader cannot see
        // it. Centred on whichever monitor it belonged to, not the primary one.
        let mut rect = windows::Win32::Foundation::RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            if rect.left < -10000 || rect.top < -10000 {
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut info = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if GetMonitorInfoW(monitor, &mut info).as_bool() {
                    let work = info.rcWork;
                    let x = work.left + (work.right - work.left - width) / 2;
                    let y = work.top + (work.bottom - work.top - height) / 2;
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOP),
                        x,
                        y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_FRAMECHANGED,
                    );
                }
            } else {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
                );
            }
        }

        // Then, and only then, activation.
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn activate_window_by_hwnd(_hwnd: windows::Win32::Foundation::HWND) {}

/// Hide the window so completely that no taskbar button is left behind.
///
/// `SW_HIDE` alone is not enough: a window that is merely hidden keeps its
/// `WS_EX_APPWINDOW` style, and the shell leaves its button in place. The style
/// has to become `WS_EX_TOOLWINDOW` for the button to go away, and the window has
/// to be parked off screen so that a later `SW_SHOW` cannot put it back in front
/// of the reader by accident.
#[cfg(target_os = "windows")]
fn hide_window_completely(hwnd: HWND) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos, ShowWindow,
        GWL_EXSTYLE, HWND_TOP, LWA_ALPHA, SW_HIDE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW,
        WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    };
    unsafe {
        // Off the taskbar.
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let style = (style & !WS_EX_APPWINDOW.0) | WS_EX_TOOLWINDOW.0;
        SetWindowLongW(hwnd, GWL_EXSTYLE, style as i32);

        // And transparent, so the frame cannot flash on the way out.
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 | WS_EX_LAYERED.0;
        SetWindowLongW(hwnd, GWL_EXSTYLE, style as i32);
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);

        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            -32000,
            -32000,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

/// Show or hide the taskbar button, hiding the window itself when `skip`.
///
/// Takes no `AppWindow`, unlike the reference's `set_skip_taskbar(&app, skip)`,
/// and the reason is that the reference's parameter is unused -- its body reaches
/// the window through `find_main_window` alone. Dropping it is what keeps this
/// file free of the Slint types, which is what lets the CLI mount it and run the
/// tests below.
///
/// Spawned rather than done inline, the reference's shape: at startup the window
/// handle does not exist yet, and the caller cannot wait for it -- the tray's
/// click has to return to the event loop. Twenty tries at 100 ms is the
/// reference's budget and is roughly how long Slint takes to put the handle up.
#[cfg(target_os = "windows")]
pub fn set_skip_taskbar(skip: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    info!(skip, "set_skip_taskbar");
    std::thread::spawn(move || {
        for _ in 0..20 {
            if let Some(hwnd) = find_main_window() {
                unsafe {
                    // Hide first, always: the shell only re-reads a window's
                    // taskbar style when its visibility changes, so flipping the
                    // style on a visible window leaves the button where it was.
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                if skip {
                    hide_window_completely(hwnd);
                } else {
                    activate_window_by_hwnd(hwnd);
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        warn!("set_skip_taskbar({skip}): no window handle within 2s");
    });
}

#[cfg(not(target_os = "windows"))]
pub fn set_skip_taskbar(_skip: bool) {}

/// The one thing about `activate_window_by_hwnd` a test can hold: that the two
/// `cfg` variants keep the same signature, so the file still compiles on the
/// platform nobody here builds for. The behaviour -- does the window actually
/// come back on screen -- needs a desktop and a second process, and is checked by
/// hand (MAPPING section 8's method).
#[cfg(all(target_os = "windows", test))]
mod activate_tests {
    use super::activate_window_by_hwnd;

    #[test]
    fn the_function_exists_for_the_non_windows_build_too() {
        let _ = activate_window_by_hwnd;
    }
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
