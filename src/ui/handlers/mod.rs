// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Wiring between the window's callbacks and the rest of the program.
//!
//! The counterpart of the reference's `src/ui/handlers/mod.rs`, which
//! registers thirteen page modules' worth of callbacks. Seven exist here:
//!
//!   `home`    the home page's actions (P2)
//!   `common`  side-bar clicks and the collapse (P3)
//!   `window`  the title bar's own: close, minimise, drag, pin (P3)
//!   `device`  the adb-targets page (P4)
//!   `app`     the install-APK page (P4)
//!   `disk`    the storage page (P5a)
//!   `network` the port-forwarding and proxy page (P5b)
//!   `settings` the settings page, its language picker and `settings.toml` (P6)
//!   `about`    the about page -- the reference's is mostly a fetch this build
//!              does not make (spec Q6), and what is left is here (P6)
//!
//! The rest arrive with the pages that own them. The reference's `setup`
//! signature is kept so that adding one is one line rather than a rethink.
//!
//! Not every callback in `app.slint` is registered by any of these, and that is
//! not a gap -- a Slint callback with no handler is a no-op, which is what
//! `toggle_theme` is in P3: the switch already flips `Theme.dark_mode` in
//! `.slint`, and the *persistence* of that choice is P6's `settings.rs`, which
//! is where the reference registers it too. The largest group of them arrived
//! with P5b: five of the network page's eleven callbacks name steps in a
//! staging model `adb` does not have, four more wait on its add and proxy
//! dialogs, and only two are wired in that task -- because P9 asks the page to
//! line up with the reference component for component. `network.rs` and MAPPING
//! 14 say which are which.

use std::sync::Arc;

use slint::Weak;

use crate::AppWindow;
use crate::app::state::AppState;

pub mod about;
pub mod app;
pub mod common;
pub mod device;
pub mod disk;
pub mod home;
pub mod network;
pub mod settings;
pub mod window;

/// `async` to match the reference's signature (`handlers/mod.rs:23`). Nothing
/// awaits here yet; the pages that do will.
pub async fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    home::setup(app, app_handle.clone(), app_state.clone());
    app::setup(app, app_handle.clone(), app_state.clone());
    device::setup(app, app_handle.clone(), app_state.clone());
    disk::setup(app, app_handle.clone(), app_state.clone());
    network::setup(app, app_handle.clone(), app_state.clone());
    settings::setup(app, app_handle.clone(), app_state.clone());
    about::setup(app, app_handle.clone(), app_state.clone());
    about::setup_callbacks(app, app_handle.clone());
    common::setup(app, app_handle.clone(), app_state.clone());
    window::setup(app, app_handle.clone());
}
