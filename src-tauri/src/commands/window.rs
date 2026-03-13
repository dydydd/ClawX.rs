//! Window management IPC command handlers

use tauri::{AppHandle, Manager};

/// Minimize the window
#[tauri::command]
pub async fn minimize_window(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Window not found")?;
    window.minimize().map_err(|e| e.to_string())
}

/// Maximize or unmaximize the window
#[tauri::command]
pub async fn maximize_window(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Window not found")?;
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

/// Close the window
#[tauri::command]
pub async fn close_window(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Window not found")?;
    window.close().map_err(|e| e.to_string())
}