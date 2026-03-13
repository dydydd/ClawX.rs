//! Application update IPC command handlers
//!
//! Provides update functionality using tauri-plugin-updater.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// Update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(rename = "releaseNotes")]
    pub release_notes: Option<String>,
}

/// Download progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressInfo {
    pub total: u64,
    pub delta: u64,
    pub transferred: u64,
    pub percent: f64,
    #[serde(rename = "bytesPerSecond")]
    pub bytes_per_second: u64,
}

/// Update status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateStatus {
    Idle,
    Checking,
    Available,
    NotAvailable,
    Downloading,
    Downloaded,
    Error,
}

/// Current update state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
    pub status: UpdateStatus,
    pub info: Option<UpdateInfo>,
    pub progress: Option<ProgressInfo>,
    pub error: Option<String>,
}

/// Get current application version
#[tauri::command]
pub fn update_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get current update status
#[tauri::command]
pub async fn update_status(app: AppHandle) -> Result<UpdateState, String> {
    // For now, return idle status
    // In a full implementation, this would track state from the updater
    Ok(UpdateState {
        status: UpdateStatus::Idle,
        info: None,
        progress: None,
        error: None,
    })
}

/// Check for updates
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<serde_json::Value, String> {
    tracing::info!("Checking for updates...");

    // Emit status change event
    let _ = app.emit("update:status-changed", UpdateState {
        status: UpdateStatus::Checking,
        info: None,
        progress: None,
        error: None,
    });

    let updater = app.updater().map_err(|e| e.to_string())?;

    match updater.check().await {
        Ok(Some(update)) => {
            tracing::info!("Update available: {}", update.version);

            let info = UpdateInfo {
                version: update.version.clone(),
                release_date: None,
                release_notes: None,
            };

            let state = UpdateState {
                status: UpdateStatus::Available,
                info: Some(info),
                progress: None,
                error: None,
            };

            let _ = app.emit("update:status-changed", &state);

            Ok(serde_json::json!({
                "success": true,
                "status": state
            }))
        }
        Ok(None) => {
            tracing::info!("No updates available");

            let state = UpdateState {
                status: UpdateStatus::NotAvailable,
                info: None,
                progress: None,
                error: None,
            };

            let _ = app.emit("update:status-changed", &state);

            Ok(serde_json::json!({
                "success": true,
                "status": state
            }))
        }
        Err(e) => {
            tracing::error!("Update check failed: {}", e);

            let state = UpdateState {
                status: UpdateStatus::Error,
                info: None,
                progress: None,
                error: Some(e.to_string()),
            };

            let _ = app.emit("update:status-changed", &state);

            Ok(serde_json::json!({
                "success": false,
                "error": e.to_string(),
                "status": state
            }))
        }
    }
}

/// Download update
#[tauri::command]
pub async fn update_download(app: AppHandle) -> Result<serde_json::Value, String> {
    tracing::info!("Downloading update...");

    let updater = app.updater().map_err(|e| e.to_string())?;

    // Emit downloading status
    let _ = app.emit("update:status-changed", UpdateState {
        status: UpdateStatus::Downloading,
        info: None,
        progress: None,
        error: None,
    });

    match updater.check().await {
        Ok(Some(update)) => {
            // Download with progress
            let version = update.version.clone();

            match update.download_and_install(|_, delta| {
                let downloaded = delta.unwrap_or(0);
                let progress = ProgressInfo {
                    total: 0,
                    delta: downloaded,
                    transferred: downloaded,
                    percent: 0.0,
                    bytes_per_second: 0,
                };
                let _ = app.emit("update:status-changed", UpdateState {
                    status: UpdateStatus::Downloading,
                    info: None,
                    progress: Some(progress),
                    error: None,
                });
            }, || {}).await {
                Ok(()) => {
                    tracing::info!("Update downloaded");

                    let state = UpdateState {
                        status: UpdateStatus::Downloaded,
                        info: Some(UpdateInfo {
                            version,
                            release_date: None,
                            release_notes: None,
                        }),
                        progress: None,
                        error: None,
                    };

                    let _ = app.emit("update:status-changed", &state);

                    Ok(serde_json::json!({ "success": true }))
                }
                Err(e) => {
                    tracing::error!("Download failed: {}", e);

                    let state = UpdateState {
                        status: UpdateStatus::Error,
                        info: None,
                        progress: None,
                        error: Some(e.to_string()),
                    };

                    let _ = app.emit("update:status-changed", &state);

                    Ok(serde_json::json!({
                        "success": false,
                        "error": e.to_string()
                    }))
                }
            }
        }
        Ok(None) => {
            Ok(serde_json::json!({
                "success": false,
                "error": "No update available to download"
            }))
        }
        Err(e) => {
            Ok(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}

/// Install update (restarts the app)
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    tracing::info!("Installing update...");

    // The tauri-plugin-updater handles installation on restart
    // We just need to restart the app
    app.restart();
    #[allow(unreachable_code)]
    Ok(())
}

/// Set update channel (stable/beta/dev)
#[tauri::command]
pub async fn update_set_channel(_channel: String) -> Result<(), String> {
    // TODO: Implement channel switching if supported
    tracing::warn!("Update channel switching not yet implemented");
    Ok(())
}

/// Set auto-download preference
#[tauri::command]
pub async fn update_set_auto_download(_enabled: bool) -> Result<(), String> {
    // TODO: Implement auto-download preference storage
    tracing::warn!("Auto-download preference not yet implemented");
    Ok(())
}

/// Cancel auto-install countdown
#[tauri::command]
pub async fn update_cancel_auto_install(app: AppHandle) -> Result<(), String> {
    let _ = app.emit("update:auto-install-countdown", serde_json::json!({
        "seconds": 0,
        "cancelled": true
    }));
    Ok(())
}