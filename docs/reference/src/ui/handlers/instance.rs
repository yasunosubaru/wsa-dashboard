// SPDX-FileCopyrightText: Copyright (c) 2026 owu <wqh@live.com>
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;
use slint::{ModelRc, VecModel};
use crate::{AppWindow, AppState, RootFSHelpItem};

use crate::app::VSCodeExtensionData;

pub fn setup(app: &AppWindow, app_handle: slint::Weak<AppWindow>, _app_state: Arc<Mutex<AppState>>) {
    let ah = app_handle.clone();
    app.on_show_rootfs_help_clicked(move || {
        debug!("RootFS help icon clicked, showing dialog and fetching latest data");
        
        // 1. Immediately show the dialog (with existing/default data)
        if let Some(app) = ah.upgrade() {
            app.set_show_rootfs_help(true);
        }

        // 2. Fetch latest data
        let ah_fetch = ah.clone();
        tokio::spawn(async move {
            refresh_rootfs_help(ah_fetch).await;
        });
    });
}

pub async fn refresh_rootfs_help(ah: slint::Weak<AppWindow>) {
    debug!("Refreshing RootFS help data from wslui helper");
    let data = tokio::task::spawn_blocking(|| {
        crate::api::common::wslui_helper_install()
    }).await.unwrap_or_default();
    
    let items: Vec<RootFSHelpItem> = data.rootfs_help.into_iter().map(|d| {
        RootFSHelpItem {
            name: d.name.into(),
            url: d.url.into(),
        }
    }).collect();

    let _ = slint::invoke_from_event_loop(move || {
        if let Some(app) = ah.upgrade() {
            let model = VecModel::from(items);
            app.set_rootfs_help_list(ModelRc::from(std::rc::Rc::new(model)));
            debug!("RootFS help list updated in UI");
        }
    });
}

pub async fn refresh_vscode_extension(as_ptr: Arc<Mutex<AppState>>) {
    debug!("Refreshing VS Code extension info from wslui helper");
    let data = tokio::task::spawn_blocking(|| {
        crate::api::common::wslui_helper_distro()
    }).await.unwrap_or_default();
    
    let ext = VSCodeExtensionData {
        name: data.vscode_extension.name,
        url: data.vscode_extension.url,
    };
    
    let mut state = as_ptr.lock().await;
    state.vscode_extension = Some(ext);
    debug!("VS Code extension info updated in AppState");
}

pub async fn refresh_startup_script(ah: slint::Weak<AppWindow>) {
    debug!("Refreshing distro startup script URL from wslui helper");
    let data = tokio::task::spawn_blocking(|| {
        crate::api::common::wslui_helper_distro()
    }).await.unwrap_or_default();
    
    let url = data.startup_script.url;
    if !url.is_empty() {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = ah.upgrade() {
                app.set_settings_startup_script_url(url.into());
                debug!("Distro startup script URL updated in UI");
            }
        });
    }
}
