// SPDX-FileCopyrightText: Copyright (c) 2026 owu <wqh@live.com>
// SPDX-License-Identifier: GPL-3.0-only

use tracing::{info, warn, error};
use crate::wsl::executor::WslCommandExecutor;

pub async fn apply_sparse_vhdx(
    executor: &WslCommandExecutor,
    distro_name: &str,
    enable_sparse_flag: bool,
    initial_is_sparse: bool,
) {
    // If not requested and wasn't sparse, nothing to do
    if !enable_sparse_flag && !initial_is_sparse {
        return;
    }
    
    info!("Applying/Restoring sparse mode for {} (target=true, initial={})...", distro_name, initial_is_sparse);
    
    let mut result = executor.execute_command(&["--manage", distro_name, "--set-sparse", "true", "--allow-unsafe"]).await;
    
    let mut retries = 0;
    while !result.success && 
          (result.output.contains("ERROR_SHARING_VIOLATION") || 
           result.output.contains("0x80070020") ||
           result.error.as_ref().map_or(false, |e| e.contains("ERROR_SHARING_VIOLATION") || e.contains("0x80070020"))) && 
          retries < 5 
    {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        info!("Retrying set-sparse due to sharing violation for {}... ({}/5)", distro_name, retries + 1);
        
        let _ = executor.execute_command(&["--terminate", distro_name]).await;
        
        if retries >= 2 {
            warn!("Sharing violation persistent. Attempting wsl --shutdown as last resort...");
            let _ = executor.execute_command(&["--shutdown"]).await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        result = executor.execute_command(&["--manage", distro_name, "--set-sparse", "true", "--allow-unsafe"]).await;
        retries += 1;
    }

    if result.success {
        info!("Successfully set sparse mode for {}.", distro_name);
    } else {
        error!("Failed to set sparse mode for {}: {}", distro_name, result.error.unwrap_or_else(|| "Unknown error".to_string()));
    }
}
