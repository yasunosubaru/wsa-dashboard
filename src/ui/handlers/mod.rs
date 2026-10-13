// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Wiring between the window's callbacks and the rest of the program.
//!
//! The counterpart of the reference's `src/ui/handlers/mod.rs`, which
//! registers thirteen page modules' worth of callbacks. One page exists in
//! P2, so one module does: `home`. The rest arrive with the pages that own
//! them, and the `setup` signature keeps the reference's so that adding one is
//! one line rather than a rethink.

use std::sync::Arc;

use slint::Weak;

use crate::AppWindow;
use crate::app::state::AppState;

pub mod home;

/// `async` to match the reference's signature (`handlers/mod.rs:23`). Nothing
/// awaits here yet; the pages that do will.
pub async fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    home::setup(app, app_handle.clone(), app_state.clone());
}
