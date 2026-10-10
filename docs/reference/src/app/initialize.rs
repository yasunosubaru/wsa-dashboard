// SPDX-FileCopyrightText: Copyright (c) 2026 owu <wqh@live.com>
// SPDX-License-Identifier: GPL-3.0-only

use tracing::{info, error};
use crate::network::scheduler;

// Execute initialization logic: create scheduled tasks and scripts
pub async fn run_initialize() {
    info!(">>> [START] Executing initialization process (/initialize) <<<");

    match scheduler::register_task_with_elevation() {
        Ok(_) => {
            info!("Successfully registered elevated scheduled task and created script.");
        },
        Err(e) => {
            error!("Failed to initialize task scheduler: {}", e);
            eprintln!("Initialization failed: {}", e);
            return;
        }
    }

    info!(">>> [FINISH] Initialization process completed <<<");
    println!("Initialization successful.");
}
