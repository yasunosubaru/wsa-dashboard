// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The state the whole window shares.
//!
//! The reference declares this in `src/main.rs` and reaches it as
//! `crate::AppState` (`src/main.rs:22`); it is a `Arc<Mutex<AppState>>` there
//! because its `WslDashboard` field is behind a lock it owns. Here the
//! dashboard already holds the only lock it needs (`executor`), so this is
//! plain and the field is never wrapped again -- see `dashboard/mod.rs` for why
//! a second lock would be a second place for the answer to change.

use std::sync::atomic::AtomicBool;

use crate::wsa::dashboard::WsaDashboard;

pub struct AppState {
    /// The one dashboard. Every read the home page makes goes through it.
    pub wsa_dashboard: WsaDashboard,
    /// Whether the window is on screen.
    ///
    /// The reference sets this from its minimize/restore hooks; P2's tick is the
    /// only reader, and [`crate::ui::refresh::should_refresh`] is what gives it
    /// teeth: a window in the tray is not a reason to keep spending adb round
    /// trips.
    pub is_visible: AtomicBool,
}

impl AppState {
    pub fn new(wsa_dashboard: WsaDashboard) -> Self {
        Self {
            wsa_dashboard,
            is_visible: AtomicBool::new(true),
        }
    }
}
