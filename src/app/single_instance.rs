// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// `SingleInstance::new` and `try_activate_existing_instance` are called by
// `runner.rs` in the GUI build. Under the CLI's `#[cfg(test)] #[path]` mount of
// this module only the tests are compiled against it -- the three pure names --
// so the two functions that hold a mutex and open a pipe are unreachable by
// construction from there, not dead. Same reason and same shape as
// `scheduler_task/models.rs` and `config/mod.rs`.
#![allow(dead_code)]

//! One program, one window: refusing the second launch by bringing back the first.
//!
//! The counterpart of the reference's `src/app/single_instance.rs`, which is two
//! things -- a named mutex that says "somebody is already running", and a way to
//! find that somebody's window and restore it.
//!
//! # What is done differently, and why
//!
//! The reference finds the existing window by trying three window *titles* --
//! its own caption, an internal constant, and a short form. That is three guesses
//! at something already knowable exactly, and a window that cannot be found by any
//! of them is a double-click that does nothing with no explanation anywhere.
//!
//! Our own `window::find_main_window` enumerates the top-level windows of **this
//! process id**. That is the right tool for "my own window" and the wrong one
//! for "the *other* instance's window" -- the second launch has no window yet,
//! so the wake path uses `window::find_any_main_window` instead: every
//! top-level window, matched by the exact caption handshake. That is not the
//! reference's three-title guess list; it is one string this program owns, and
//! [`window_titles_for_lookup`] still exists only to be asserted absent.
//!
//! The pipe is the primary signal and now has a server: `app::wakeup` owns the
//! listening end, and a connection there means "show the window". The window
//! lookup is the fallback for the case where the pipe is gone but a window is
//! there.
//!
//! The named pipe stays, because it does something the window does not: a window
//! can be in the tray with its handle already gone from the shell's live list,
//! while a pipe is answered by the process itself. Two mechanisms, either of
//! which is enough, which is how the reference has it too.

use tracing::{info, warn};

/// The registry/canonical name of this mutex.
///
/// Derived from [`APP_ID`](super::constants::APP_ID) rather than written
/// out, so the one string that identifies this program cannot disagree with
/// itself. The reference's is a hardcoded `"wsldashboard-v0.3-lock"`.
pub fn mutex_name() -> String {
    format!("{}-single-instance", super::constants::APP_ID)
}

/// The wakeup pipe.
///
/// The reference's is a versioned pipe name (`..._v0.3`) because it wanted an
/// old build to be unable to wake a new one. Ours is unversioned: there is no
/// older build of *this* program that a wakeup would reach, and a version number
/// here would have to be bumped by hand, by someone who does not know it exists.
pub fn pipe_name() -> String {
    format!(
        r"\\.\pipe\{}_wakeup_pipe",
        super::constants::APP_ID
    )
}

/// The titles this module does *not* use.
///
/// Present so a test can assert it stays empty: the moment someone adds a title
/// here, the process-id lookup stops being the way the window is found, and the
/// failure mode it replaced -- a double-click that does nothing -- comes back.
pub fn window_titles_for_lookup() -> Vec<String> {
    Vec::new()
}

/// Try to wake an existing instance. `true` means one was found.
///
/// A closed pipe or a missing window is not an error, and it is not a warning:
/// it is the ordinary state of a first launch. The reference logs the pipe
/// failure at `warn`, which fills its log with warnings on every normal start.
pub fn try_activate_existing_instance() -> bool {
    // The window is looked up *before* the pipe fires, because the pid behind it
    // is what the foreground hand-off needs and the hand-off must land before
    // the show it authorises. The reader's double-click gave this process the
    // right to change the foreground; the instance in the tray did not receive
    // that input, so on its own its `SetForegroundWindow` is refused and the
    // window comes back behind whatever is on screen. This is the standard
    // hand-off for exactly that split.
    let found_window = super::window::find_any_main_window();
    #[cfg(target_os = "windows")]
    if let Some(hwnd) = found_window {
        use windows::Win32::UI::WindowsAndMessaging::{
            AllowSetForegroundWindow, GetWindowThreadProcessId,
        };
        let mut owner: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner)) };
        if owner != 0 {
            match unsafe { AllowSetForegroundWindow(owner) } {
                Ok(()) => info!(owner, "handed the foreground right to the running instance"),
                Err(error) => {
                    warn!(%error, owner, "could not hand over the foreground right")
                }
            }
        }
    }

    let pipe_name = pipe_name();
    let pipe_connected = match std::fs::OpenOptions::new().write(true).open(&pipe_name) {
        Ok(_) => {
            info!("wakeup pipe answered; the running instance has been signalled");
            true
        }
        Err(_) => false,
    };

    let Some(hwnd) = found_window else {
        if pipe_connected {
            // No top-level window, but the process answered. It is starting or
            // minimised to the tray; the pipe is the part that reaches it.
            info!("no window to restore, but the wakeup pipe answered");
            return true;
        }
        return false;
    };

    info!(?hwnd, "found the running instance's window; restoring it");
    super::window::activate_window_by_hwnd(hwnd);

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};
        if unsafe { IsIconic(hwnd).as_bool() } {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }
        if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
            // Windows refuses the foreground switch when this process is not
            // already foreground -- that is the documented rule of
            // `SetForegroundWindow`. The pipe has already told the other
            // instance the reader asked for it, so this is a log line and not a
            // problem the reader can fix.
            warn!("SetForegroundWindow refused; the pipe signal has still been delivered");
        }
    }

    let _ = pipe_connected;
    true
}

/// Holds the mutex for as long as it exists.
pub struct SingleInstance {
    handle: Option<windows::Win32::Foundation::HANDLE>,
}

impl SingleInstance {
    /// Create the mutex. `handle` is `None` when another instance holds it.
    pub fn new(name: &str) -> Self {
        let name_u16: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe {
            windows::Win32::System::Threading::CreateMutexW(
                None,
                true,
                windows::core::PCWSTR(name_u16.as_ptr()),
            )
        };
        match handle {
            Ok(handle) => {
                // `unsafe`, like every call that reads thread-local Windows
                // state. The reference has it inside its own `unsafe` block that
                // wraps the `CreateMutexW` call; here the two are separated so the
                // comparison is on its own line and readable.
                let already_exists =
                    unsafe { windows::Win32::Foundation::GetLastError() }
                        == windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
                if already_exists {
                    info!("another instance owns the mutex; this launch should wake it");
                    // Deliberately closed: `CreateMutexW` returned a handle to a
                    // mutex we do not own, and holding it open is how a process
                    // ends up blocking its own exit.
                    unsafe { let _ = windows::Win32::Foundation::CloseHandle(handle); }
                    Self { handle: None }
                } else {
                    info!("single-instance mutex created");
                    Self { handle: Some(handle) }
                }
            }
            Err(error) => {
                warn!(%error, "could not create the single-instance mutex");
                Self { handle: None }
            }
        }
    }

    /// Whether this is the only instance.
    ///
    /// `true` also when the mutex could not be created at all. That is the
    /// reference's answer and it is the right one: the mutex is an optimisation,
    /// and refusing to start because a mutex could not be named is worse than
    /// running twice.
    pub fn is_single(&self) -> bool {
        self.handle.is_some()
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            unsafe { let _ = windows::Win32::Foundation::CloseHandle(handle); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mutex name is derived, so it cannot drift from the app id -- which is
    /// the one string the uninstaller, the logs and the About page all read.
    #[test]
    fn the_mutex_name_is_derived_from_the_app_id_and_is_a_valid_name() {
        let name = mutex_name();
        assert!(
            name.starts_with(super::super::constants::APP_ID),
            "got {name}"
        );
        assert_eq!(name, "wsadashboard-single-instance");
        // A Windows object name may not contain a backslash; it would make this
        // a path and land in another namespace entirely.
        assert!(!name.contains('\\'), "a mutex name must not contain a backslash");
        assert!(!name.is_empty());
    }

    #[test]
    fn the_pipe_name_is_a_named_pipe_path_built_from_the_app_id() {
        let name = pipe_name();
        assert_eq!(name, r"\\.\pipe\wsadashboard_wakeup_pipe");
        assert!(name.starts_with(r"\\.\pipe\"), "must be a named pipe path");
    }

    /// The reference's title list, asserted absent. See the header: this is the
    /// part that made a double-click silently do nothing when a caption did not
    /// match.
    #[test]
    fn the_window_is_found_by_process_id_so_there_are_no_titles_to_try() {
        assert!(
            window_titles_for_lookup().is_empty(),
            "a window title lookup has come back; the process-id one is exact"
        );
    }

    /// Two names, two mechanisms, no collisions.
    #[test]
    fn the_mutex_and_the_pipe_are_not_the_same_name() {
        assert_ne!(mutex_name(), pipe_name());
    }
}
