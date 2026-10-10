// SPDX-FileCopyrightText: Copyright (c) 2026 owu <wqh@live.com>
// SPDX-License-Identifier: GPL-3.0-only

use windows::Win32::Foundation::{HWND, HANDLE, ERROR_ALREADY_EXISTS};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, IsIconic,
};
use windows::Win32::System::Threading::CreateMutexW;
use tracing::{info, warn, error};

// Tries to activate an existing instance if one is running.
// Returns true if an existing instance was found and activated.
pub fn try_activate_existing_instance() -> bool {
    {
        let pipe_name = r"\\.\pipe\wsldashboard_wakeup_pipe_v0.3";
        let pipe_connected = match std::fs::OpenOptions::new().write(true).open(pipe_name) {
            Ok(_) => {
                info!("Successfully sent wakeup signal via Named Pipe");
                true
            }
            Err(e) => {
                warn!("Failed to connect to Named Pipe: {}", e);
                false
            }
        };

        // Try multiple possible titles (Main vs Internal UI)
        let titles = ["WSL Dashboard Main", "WSL_DASHBOARD_WINDOW_UI", "WSL Dashboard"];
        let mut hwnd = HWND(std::ptr::null_mut());

        for title_str in titles {
            let window_title: Vec<u16> = title_str
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            
            let hwnd_result = unsafe {
                FindWindowW(
                    windows::core::PCWSTR::null(),
                    windows::core::PCWSTR(window_title.as_ptr()),
                )
            };

            if let Ok(h) = hwnd_result {
                if !h.0.is_null() {
                    hwnd = h;
                    break;
                }
            }
        }

        if hwnd.0.is_null() {
            if pipe_connected {
                info!("No existing window found, but Named Pipe wakeup succeeded");
                return true;
            }
            info!("No existing window found with known titles");
            return false;
        }

        info!("Found existing instance window (HWND: {:?}), activating...", hwnd);

        // Always invoke activate_window_by_hwnd first to restore position, opacity and window style
        crate::app::window::activate_window_by_hwnd(hwnd);

        // If the window is minimized, restore it
        if unsafe { IsIconic(hwnd).as_bool() } {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }

        // Bring the window to foreground
        let result = unsafe { SetForegroundWindow(hwnd) };
        
        if result.as_bool() {
            info!("Successfully activated existing instance via SetForegroundWindow");
        } else {
            warn!("SetForegroundWindow returned false, relying on Named Pipe and activate_window_by_hwnd");
        }

        // Return true since we found the window or connected the pipe
        true
    }
}

// A wrapper for a Windows Mutex to ensure single instance of the application.
pub struct SingleInstance {
    handle: Option<HANDLE>,
}

impl SingleInstance {
    // Creates a new SingleInstance check with the given unique name.
    pub fn new(name: &str) -> Self {
        let name_u16: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        
        unsafe {
            let handle = CreateMutexW(
                None,
                true,
                windows::core::PCWSTR(name_u16.as_ptr())
            );

            match handle {
                Ok(h) => {
                    if windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS {
                        // Another instance holds the mutex
                        info!("Another instance is already running (Mutex exists)");
                        Self { handle: None }
                    } else {
                        // We are the first instance
                        info!("Single instance Mutex created successfully");
                        Self { handle: Some(h) }
                    }
                }
                Err(e) => {
                    error!("Failed to create single instance Mutex: {}", e);
                    Self { handle: None }
                }
            }
        }
    }

    // Returns true if this is the only instance running.
    pub fn is_single(&self) -> bool {
        self.handle.is_some()
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        if let Some(h) = self.handle {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(h);
            }
        }
    }
}
