//! File operations IPC command handlers

use std::path::PathBuf;

/// Read a file's contents
#[tauri::command]
pub async fn read_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(&path);

    // Security: Ensure we're only reading from allowed directories
    // TODO: Implement proper sandboxing

    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))
}

/// Write content to a file
#[tauri::command]
pub async fn write_file(path: String, content: String) -> Result<(), String> {
    let path = PathBuf::from(&path);

    // Security: Ensure we're only writing to allowed directories
    // TODO: Implement proper sandboxing

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))
}