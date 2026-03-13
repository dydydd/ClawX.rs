//! Log IPC command handlers

use std::sync::Arc;
use tauri::State;
use crate::core::logging::{Logger, LogFileInfo};

/// Get log directory
#[tauri::command]
pub async fn get_log_dir(
    logger: State<'_, Arc<Logger>>,
) -> Result<String, String> {
    Ok(logger.get_log_dir().display().to_string())
}

/// Read log file content
#[tauri::command]
pub async fn read_log_file(
    tail_lines: Option<usize>,
    logger: State<'_, Arc<Logger>>,
) -> Result<String, String> {
    let lines = tail_lines.unwrap_or(200);
    logger.read_log_file(lines).await.map_err(|e| e.to_string())
}

/// List log files
#[tauri::command]
pub async fn list_log_files(
    logger: State<'_, Arc<Logger>>,
) -> Result<Vec<LogFileInfo>, String> {
    logger.list_log_files().await.map_err(|e| e.to_string())
}

/// Get recent logs from memory buffer
#[tauri::command]
pub async fn get_recent_logs(
    count: Option<usize>,
    min_level: Option<String>,
    logger: State<'_, Arc<Logger>>,
) -> Result<Vec<String>, String> {
    let level = min_level.and_then(|l| match l.to_uppercase().as_str() {
        "DEBUG" => Some(crate::core::logging::LogLevel::Debug),
        "INFO" => Some(crate::core::logging::LogLevel::Info),
        "WARN" => Some(crate::core::logging::LogLevel::Warn),
        "ERROR" => Some(crate::core::logging::LogLevel::Error),
        _ => None,
    });

    Ok(logger.get_recent_logs(count, level))
}