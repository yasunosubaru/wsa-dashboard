// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The state the whole window shares.
//!
//! The reference declares this in `src/main.rs` and reaches it as
//! `crate::AppState` (`src/main.rs:22`). It wraps the whole thing in
//! `Arc<Mutex<...>>` because its `WslDashboard` field is behind a lock it
//! owns; here the dashboard already holds the only lock it needs
//! (`dashboard/mod.rs` explains why a second one would be a second place for
//! the answer to change), so this is plain and the field is never wrapped
//! again.
//!
//! The GUI bin only. `wsadash` has no window and no logs of its own, and
//! dragging `tracing-appender` into the CLI to satisfy a type would be a
//! dependency in service of nothing.

use std::sync::atomic::AtomicBool;

use tracing_appender::non_blocking::WorkerGuard;

use crate::wsa::dashboard::WsaDashboard;

pub struct AppState {
    /// The one dashboard. Every read the home page makes goes through it.
    pub wsa_dashboard: WsaDashboard,
    /// Whether the window is on screen.
    ///
    /// The reference sets this from its minimize/restore hooks; P2's tick is
    /// the only reader, and `should_refresh` is what gives it teeth: a window
    /// in the tray is not a reason to keep spending adb round trips.
    pub is_visible: AtomicBool,
    /// Held, not read.
    ///
    /// Dropping this ends the background writer thread and with it every log
    /// line still queued, so it lives exactly as long as the state does --
    /// which is the whole of the window's life. Keeping it here rather than
    /// `mem::forget`ing it in `launcher` is the difference between a resource
    /// with an owner and one that happens not to have been cleaned up.
    pub _logging: WorkerGuard,
}

impl AppState {
    pub fn new(wsa_dashboard: WsaDashboard, logging: WorkerGuard) -> Self {
        Self {
            wsa_dashboard,
            is_visible: AtomicBool::new(true),
            _logging: logging,
        }
    }
}
