// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Wiring between the window's callbacks and the rest of the program.
//!
//! The counterpart of the reference's `src/ui/handlers/mod.rs`, which
//! registers thirteen page modules' worth of callbacks. Three exist here:
//!
//!   `home`    the home page's actions (P2)
//!   `common`  side-bar clicks and the collapse (P3)
//!   `window`  the title bar's own: close, minimise, drag, pin (P3)
//!
//! The rest arrive with the pages that own them. The reference's `setup`
//! signature is kept so that adding one is one line rather than a rethink.
//!
//! Not every callback in `app.slint` is registered by any of these, and that is
//! not a gap -- a Slint callback with no handler is a no-op, which is what
//! `toggle_theme` is in P3: the switch already flips `Theme.dark_mode` in
//! `.slint`, and the *persistence* of that choice is P6's `settings.rs`, which
//! is where the reference registers it too.

use std::sync::Arc;

use slint::Weak;

use crate::AppWindow;
use crate::app::state::AppState;

pub mod app;
pub mod common;
pub mod device;
pub mod home;
pub mod window;

/// `async` to match the reference's signature (`handlers/mod.rs:23`). Nothing
/// awaits here yet; the pages that do will.
pub async fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    home::setup(app, app_handle.clone(), app_state.clone());
    app::setup(app, app_handle.clone(), app_state.clone());
    device::setup(app, app_handle.clone(), app_state.clone());
    common::setup(app, app_handle.clone(), app_state.clone());
    window::setup(app, app_handle.clone());
}
