// SPDX-FileCopyrightText: Copyright (c) 2026 owu <wqh@live.com>
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::launcher::AppContext;
use crate::app::single_instance::SingleInstance;
use tracing::info;

// Maintenance and check logic before running the app (single instance, log cleanup, autostart repair)
// Returns Ok(SingleInstance) if can continue, Err(i32) if need to exit
pub async fn pre_run_maintenance(ctx: &AppContext, is_silent_mode: bool) -> Result<SingleInstance, i32> {
    // 6. Single Instance check (after log initialization to capture errors)
    let instance = SingleInstance::new("wsldashboard-v0.3-lock");
    if !instance.is_single() {
        if !is_silent_mode {
            if crate::app::single_instance::try_activate_existing_instance() {
                info!("Activated existing instance, exiting...");
            } else {
                eprintln!("Another instance is already running. Exiting.");
            }
        } else {
            eprintln!("Another instance is already running (silent mode). Exiting.");
        }
        return Err(0);
    }
    
    // 7. Cleanup expired logs
    crate::utils::logging::cleanup_expired_logs(
        &ctx.settings.logs_location, 
        ctx.settings.log_days
    );

    // 8. Path automatic repair: update registry if the exe has been moved
    let tray_settings = ctx.config_manager.get_tray_settings();
    crate::app::autostart::repair_autostart_path(
        tray_settings.autostart, 
        tray_settings.start_minimized
    ).await;

    if is_silent_mode {
        info!("Silent mode detected via /silent parameter");
    }
    
    Ok(instance)
}
