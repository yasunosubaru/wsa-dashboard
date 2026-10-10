// SPDX-FileCopyrightText: Copyright (c) 2026 owu <wqh@live.com>
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::{AppState, AppWindow};

pub mod utils;
pub use utils::*;
pub mod rules;
pub mod proxy;
pub mod service;

pub fn setup(app: &AppWindow, app_handle: slint::Weak<AppWindow>, app_state: Arc<Mutex<AppState>>) {
    // 1. Initial Data Refresh
    let ah_refresh = app_handle.clone();
    let as_refresh = app_state.clone();
    tokio::spawn(async move {
        utils::refresh_network_view_data(ah_refresh, as_refresh).await;
    });

    // 2. Setup Sub-handlers
    rules::setup(app, app_handle.clone(), app_state.clone());
    proxy::setup(app, app_handle.clone(), app_state.clone());
    service::setup(app, app_handle, app_state);
}
