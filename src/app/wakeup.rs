// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The server half of the single-instance handshake.
//!
//! `single_instance` owns the names and the client half: a second launch opens
//! `\\.\pipe\wsadashboard_wakeup_pipe` and treats a successful open as "the
//! running instance has been signalled". Until this module existed there was
//! nobody on the other end -- the open failed on every launch, the fallback
//! window lookup searched the *second* process's own windows (which cannot
//! exist before it creates a window), and the launch exited with "the mutex is
//! owned but nothing answered". A double-click on the exe while the app sat in
//! the tray therefore did nothing at all.
//!
//! The server is a plain thread: one pipe instance at a time, a connection is
//! the signal (the client writes nothing), and the window work is marshalled to
//! the UI thread with `invoke_from_event_loop` -- the same bridge the refresh
//! pipeline uses. A failed create is logged and retried, because a wakeup
//! channel that dies silently puts the reader back in the "double-click does
//! nothing" state this module exists to end.
//!
//! GUI binary only: it names `AppWindow` and the window handlers, so the CLI's
//! test build does not mount it.

#[cfg(target_os = "windows")]
use tracing::{info, warn};

#[cfg(target_os = "windows")]
use crate::AppWindow;

/// Start the listening thread. Called once, after the window exists.
#[cfg(target_os = "windows")]
pub fn spawn(weak: slint::Weak<AppWindow>) {
    let _ = std::thread::Builder::new()
        .name("single-instance-wakeup".into())
        .spawn(move || serve(weak))
        .map_err(|error| warn!(%error, "could not start the wakeup pipe thread"));
}

#[cfg(target_os = "windows")]
fn serve(weak: slint::Weak<AppWindow>) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_PIPE_CONNECTED, GetLastError,
    };
    use windows::Win32::Storage::FileSystem::{ReadFile, PIPE_ACCESS_INBOUND};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
        PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    let name = crate::app::single_instance::pipe_name();
    let name_u16: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    info!(pipe = %name, "wakeup pipe server listening");

    loop {
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(name_u16.as_ptr()),
                PIPE_ACCESS_INBOUND,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,    // one instance at a time: a wakeup is a knock, not a stream
                0,    // outbound buffer, unused
                4096, // inbound buffer
                0,    // default timeout
                None,
            )
        };
        if handle.is_invalid() {
            warn!(
                error = ?unsafe { GetLastError() },
                "could not create the wakeup pipe; retrying"
            );
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        }

        // A client that connected between CreateNamedPipeW and here is the
        // ERROR_PIPE_CONNECTED case, not a failure.
        let connected = unsafe { ConnectNamedPipe(handle, None) }.is_ok()
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;

        if connected {
            // The connection itself is the message; drain what the client wrote
            // so its close completes cleanly.
            let mut buffer = [0u8; 16];
            let mut read = 0u32;
            let _ = unsafe { ReadFile(handle, Some(&mut buffer), Some(&mut read), None) };
            info!("wakeup pipe answered; showing the window");
            let weak = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    crate::ui::handlers::window::show_and_center(&app);
                }
            });
        }

        unsafe {
            let _ = DisconnectNamedPipe(handle);
            let _ = CloseHandle(handle);
        }
    }
}
