//! Application information IPC command handlers

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub electron: Option<String>,
    pub tauri: String,
}

/// Get application information
#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        electron: None, // No longer using Electron
        tauri: "2.0.0".to_string(), // TODO: Get actual Tauri version
    }
}

/// Get platform information
#[tauri::command]
pub fn get_platform() -> String {
    std::env::consts::OS.to_string()
}