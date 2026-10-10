// SPDX-FileCopyrightText: Copyright (c) 2026 owu <wqh@live.com>
// SPDX-License-Identifier: GPL-3.0-only

use ini::Ini;
use tracing::{debug, warn};

// Get the WSL networking mode from ~/.wslconfig
// Returns "nat" as default if file or setting is missing.
pub fn get_wsl_networking_mode() -> String {
    let home_dir = match dirs::home_dir() {
        Some(path) => path,
        None => {
            warn!("Could not determine home directory, defaulting networkingMode to 'nat'");
            return "nat".to_string();
        }
    };
    
    let wsl_config_path = home_dir.join(".wslconfig");
    if !wsl_config_path.exists() {
        debug!(".wslconfig not found at {:?}, defaulting networkingMode to 'nat'", wsl_config_path);
        return "nat".to_string();
    }
    
    match Ini::load_from_file(&wsl_config_path) {
        Ok(ini) => {
            if let Some(section) = ini.section(Some("wsl2")) {
                if let Some(mode) = section.get("networkingMode") {
                    let mode_lower = mode.to_lowercase();
                    debug!("Detected networkingMode from .wslconfig: {}", mode_lower);
                    return mode_lower;
                }
            }
            debug!("networkingMode not found in [wsl2] section of .wslconfig, defaulting to 'nat'");
            "nat".to_string()
        }
        Err(e) => {
            warn!("Failed to parse .wslconfig at {:?}: {}, defaulting networkingMode to 'nat'", wsl_config_path, e);
            "nat".to_string()
        }
    }
}

// Check if sparseVhd is enabled in ~/.wslconfig
pub fn get_sparse_vhd() -> bool {
    let home_dir = match dirs::home_dir() {
        Some(path) => path,
        None => return false,
    };
    
    let wsl_config_path = home_dir.join(".wslconfig");
    if !wsl_config_path.exists() {
        return false;
    }
    
    match Ini::load_from_file(&wsl_config_path) {
        Ok(ini) => {
            if let Some(section) = ini.section(Some("experimental")) {
                if let Some(val) = section.get("sparseVhd") {
                    return val.to_lowercase() == "true";
                }
            }
            false
        }
        Err(_) => false
    }
}

// Set sparseVhd in ~/.wslconfig
pub fn set_sparse_vhd(enable: bool) -> Result<(), String> {
    let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
    let wsl_config_path = home_dir.join(".wslconfig");
    
    let mut ini = if wsl_config_path.exists() {
        Ini::load_from_file(&wsl_config_path).unwrap_or_else(|_| Ini::new())
    } else {
        Ini::new()
    };
    
    if enable {
        ini.with_section(Some("experimental")).set("sparseVhd", "true");
    } else {
        ini.with_section(Some("experimental")).set("sparseVhd", "false");
    }
    
    ini.write_to_file(&wsl_config_path).map_err(|e| format!("Failed to write .wslconfig: {}", e))
}
