//! Channel configuration IPC command handlers
//!
//! Provides commands for managing channel configurations including:
//! - CRUD operations for channels
//! - Enable/disable channels
//! - Channel credential validation
//! - WhatsApp QR code login flow

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::core::channels::{
    Channel, ChannelConfig, ChannelManager, ChannelStatus, QRCodeData,
    WhatsAppManager,
};
use crate::core::config::{
    delete_channel_account_config, delete_channel_config, get_channel_config,
    list_configured_channels, save_channel_config, set_channel_enabled, ChannelConfigData,
};

/// Channel information for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub channel_type: String,
    pub enabled: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    pub config: serde_json::Value,
}

impl From<Channel> for ChannelInfo {
    fn from(channel: Channel) -> Self {
        Self {
            id: channel.id.clone(),
            name: channel.id.clone(), // Use ID as name, can be customized later
            channel_type: channel.channel_type.clone(),
            enabled: channel.enabled,
            status: format!("{:?}", channel.status).to_lowercase(),
            status_message: channel.status_message,
            config: serde_json::to_value(&channel.config).unwrap_or_default(),
        }
    }
}

/// Channel validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// QR code response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QRCodeResponse {
    pub base64_png: String,
    pub raw: String,
    pub timestamp: i64,
}

impl From<QRCodeData> for QRCodeResponse {
    fn from(data: QRCodeData) -> Self {
        Self {
            base64_png: data.base64_png,
            raw: data.raw,
            timestamp: data.timestamp.timestamp_millis(),
        }
    }
}

/// WhatsApp login status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppLoginStatus {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qr_code: Option<QRCodeResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

// ==================== Legacy OpenClaw Config Commands ====================

/// List all configured channels (from OpenClaw config)
#[tauri::command]
pub async fn list_channels() -> Result<Vec<String>, String> {
    list_configured_channels()
        .await
        .map_err(|e| e.to_string())
}

/// Get channel configuration (from OpenClaw config)
#[tauri::command]
pub async fn get_channel(
    channel_type: String,
    account_id: Option<String>,
) -> Result<Option<ChannelConfigData>, String> {
    get_channel_config(&channel_type, account_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Save a channel configuration (to OpenClaw config)
#[tauri::command]
pub async fn save_channel(
    channel_type: String,
    config: ChannelConfigData,
    account_id: Option<String>,
) -> Result<(), String> {
    save_channel_config(&channel_type, config, account_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Delete a channel configuration
#[tauri::command]
pub async fn delete_channel(channel_type: String) -> Result<(), String> {
    delete_channel_config(&channel_type)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a channel account configuration
#[tauri::command]
pub async fn delete_channel_account(
    channel_type: String,
    account_id: String,
) -> Result<(), String> {
    delete_channel_account_config(&channel_type, &account_id)
        .await
        .map_err(|e| e.to_string())
}

/// Set channel enabled status
#[tauri::command]
pub async fn set_channel_enabled_cmd(
    channel_type: String,
    enabled: bool,
) -> Result<(), String> {
    set_channel_enabled(&channel_type, enabled)
        .await
        .map_err(|e| e.to_string())
}

// ==================== New Channel Manager Commands ====================

/// List all channels from the channel manager
#[tauri::command]
pub async fn list_all_channels(
    channel_manager: State<'_, Arc<ChannelManager>>,
) -> Result<Vec<ChannelInfo>, String> {
    channel_manager
        .list_channels()
        .await
        .into_iter()
        .map(ChannelInfo::from)
        .collect::<Vec<_>>()
        .pipe(Ok)
}

/// Get a specific channel by ID
#[tauri::command]
pub async fn get_channel_by_id(
    channel_manager: State<'_, Arc<ChannelManager>>,
    id: String,
) -> Result<Option<ChannelInfo>, String> {
    let channel = channel_manager.get_channel(&id).await;
    Ok(channel.map(ChannelInfo::from))
}

/// Enable a channel
#[tauri::command]
pub async fn enable_channel(
    channel_manager: State<'_, Arc<ChannelManager>>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    channel_manager
        .set_channel_enabled(&id, true)
        .await
        .map_err(|e| e.to_string())?;

    // Emit event for frontend
    let _ = app_handle.emit("channel:enabled", serde_json::json!({ "id": id }));

    Ok(())
}

/// Disable a channel
#[tauri::command]
pub async fn disable_channel(
    channel_manager: State<'_, Arc<ChannelManager>>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    channel_manager
        .set_channel_enabled(&id, false)
        .await
        .map_err(|e| e.to_string())?;

    // Emit event for frontend
    let _ = app_handle.emit("channel:disabled", serde_json::json!({ "id": id }));

    Ok(())
}

/// Create or update a channel
#[tauri::command]
pub async fn create_channel(
    channel_manager: State<'_, Arc<ChannelManager>>,
    app_handle: AppHandle,
    id: String,
    channel_type: String,
    config: serde_json::Value,
    enabled: Option<bool>,
) -> Result<ChannelInfo, String> {
    let mut channel = channel_manager
        .get_or_create_channel(id.clone(), channel_type)
        .await
        .map_err(|e| e.to_string())?;

    // Update config
    let channel_config: ChannelConfig =
        serde_json::from_value(config).map_err(|e| e.to_string())?;
    channel.set_config(channel_config);

    // Update enabled status if specified
    if let Some(enabled) = enabled {
        if enabled {
            channel.enable();
        } else {
            channel.disable();
        }
    }

    // Save channel
    channel_manager
        .save_channel(channel.clone())
        .await
        .map_err(|e| e.to_string())?;

    // Emit event
    let _ = app_handle.emit(
        "channel:updated",
        serde_json::json!({ "id": id, "type": channel.channel_type }),
    );

    Ok(ChannelInfo::from(channel))
}

/// Delete a channel by ID
#[tauri::command]
pub async fn remove_channel(
    channel_manager: State<'_, Arc<ChannelManager>>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    channel_manager
        .delete_channel(&id)
        .await
        .map_err(|e| e.to_string())?;

    // Emit event
    let _ = app_handle.emit("channel:deleted", serde_json::json!({ "id": id }));

    Ok(())
}

/// Update channel configuration
#[tauri::command]
pub async fn update_channel_config(
    channel_manager: State<'_, Arc<ChannelManager>>,
    app_handle: AppHandle,
    id: String,
    config: serde_json::Value,
) -> Result<ChannelInfo, String> {
    let mut channel = channel_manager
        .get_channel(&id)
        .await
        .ok_or_else(|| "Channel not found".to_string())?;

    let channel_config: ChannelConfig =
        serde_json::from_value(config).map_err(|e| e.to_string())?;
    channel.set_config(channel_config);

    channel_manager
        .save_channel(channel.clone())
        .await
        .map_err(|e| e.to_string())?;

    // Emit event
    let _ = app_handle.emit("channel:updated", serde_json::json!({ "id": id }));

    Ok(ChannelInfo::from(channel))
}

/// Update channel status
#[tauri::command]
pub async fn update_channel_status_cmd(
    channel_manager: State<'_, Arc<ChannelManager>>,
    app_handle: AppHandle,
    id: String,
    status: String,
    message: Option<String>,
) -> Result<(), String> {
    let status = match status.as_str() {
        "disconnected" => ChannelStatus::Disconnected,
        "connecting" => ChannelStatus::Connecting,
        "connected" => ChannelStatus::Connected,
        "error" => ChannelStatus::Error,
        _ => return Err("Invalid status".to_string()),
    };

    channel_manager
        .update_channel_status(&id, status, message)
        .await
        .map_err(|e| e.to_string())?;

    // Emit event
    let _ = app_handle.emit(
        "channel:status-changed",
        serde_json::json!({ "id": id, "status": "updated" }),
    );

    Ok(())
}

// ==================== Validation Commands ====================

/// Validate channel credentials
#[tauri::command]
pub async fn validate_channel_credentials(
    channel_type: String,
    config: serde_json::Value,
) -> Result<ValidationResult, String> {
    let mut result = ValidationResult {
        valid: true,
        errors: Vec::new(),
        warnings: Vec::new(),
        details: None,
    };

    match channel_type.as_str() {
        "discord" => {
            // Validate Discord bot token
            if let Some(token) = config.get("token").and_then(|v| v.as_str()) {
                if token.is_empty() {
                    result.valid = false;
                    result.errors.push("Bot token is required".to_string());
                } else if !token.starts_with("Bot ") && token.len() < 50 {
                    // Basic format check
                    result.warnings.push("Token format may be invalid".to_string());
                }
            } else {
                result.valid = false;
                result.errors.push("Bot token is required".to_string());
            }
        }
        "telegram" => {
            // Validate Telegram bot token
            if let Some(token) = config.get("botToken").and_then(|v| v.as_str()) {
                if token.is_empty() {
                    result.valid = false;
                    result.errors.push("Bot token is required".to_string());
                } else if !token.contains(':') {
                    result.valid = false;
                    result.errors.push(
                        "Invalid token format. Should be like '123456:ABC-DEF...'".to_string(),
                    );
                }
            } else {
                result.valid = false;
                result.errors.push("Bot token is required".to_string());
            }

            // Validate allowed users
            if let Some(users) = config.get("allowedUsers").and_then(|v| v.as_str()) {
                if users.is_empty() {
                    result.warnings.push("No allowed users specified".to_string());
                }
            }
        }
        "feishu" => {
            // Validate Feishu credentials
            if config.get("appId").is_none() {
                result.valid = false;
                result.errors.push("App ID is required".to_string());
            }
            if config.get("appSecret").is_none() {
                result.valid = false;
                result.errors.push("App Secret is required".to_string());
            }
        }
        "whatsapp" => {
            // WhatsApp doesn't require config validation (uses QR login)
            result.warnings.push("WhatsApp uses QR code login".to_string());
        }
        _ => {
            result.warnings.push(format!(
                "No validation available for channel type: {}",
                channel_type
            ));
        }
    }

    Ok(result)
}

// ==================== WhatsApp Commands ====================

/// Start WhatsApp login process
#[tauri::command]
pub async fn start_whatsapp_login(
    whatsapp_manager: State<'_, Arc<WhatsAppManager>>,
    app_handle: AppHandle,
    account_id: String,
) -> Result<(), String> {
    // Start login process
    let mut event_rx = whatsapp_manager
        .start_login(&account_id, Some(app_handle.clone()))
        .await
        .map_err(|e| e.to_string())?;

    // Spawn a task to forward events to the frontend
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        use crate::core::channels::WhatsAppLoginEvent;

        while let Some(event) = event_rx.recv().await {
            match event {
                WhatsAppLoginEvent::QrCode(qr) => {
                    let _ = app_handle_clone.emit(
                        "whatsapp:qr",
                        serde_json::json!({
                            "accountId": account_id,
                            "qrCode": QRCodeResponse::from(qr)
                        }),
                    );
                }
                WhatsAppLoginEvent::Connected { account_id } => {
                    let _ = app_handle_clone.emit(
                        "whatsapp:connected",
                        serde_json::json!({ "accountId": account_id }),
                    );
                    break;
                }
                WhatsAppLoginEvent::Error { message } => {
                    let _ = app_handle_clone.emit(
                        "whatsapp:error",
                        serde_json::json!({ "accountId": account_id, "error": message }),
                    );
                    break;
                }
                WhatsAppLoginEvent::Stopped => {
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Stop WhatsApp login process
#[tauri::command]
pub async fn stop_whatsapp_login(
    whatsapp_manager: State<'_, Arc<WhatsAppManager>>,
    account_id: String,
) -> Result<(), String> {
    whatsapp_manager
        .stop_login(&account_id)
        .await
        .map_err(|e| e.to_string())
}

/// Get WhatsApp login status
#[tauri::command]
pub async fn get_whatsapp_login_status(
    whatsapp_manager: State<'_, Arc<WhatsAppManager>>,
    account_id: String,
) -> Result<WhatsAppLoginStatus, String> {
    let state = whatsapp_manager.get_login_state(&account_id).await;
    let qr_code = whatsapp_manager.get_qr_code(&account_id).await;

    Ok(WhatsAppLoginStatus {
        state: format!("{:?}", state).to_lowercase(),
        qr_code: qr_code.map(QRCodeResponse::from),
        error_message: None,
    })
}

/// Check if WhatsApp has credentials
#[tauri::command]
pub async fn has_whatsapp_credentials(
    whatsapp_manager: State<'_, Arc<WhatsAppManager>>,
    account_id: String,
) -> Result<bool, String> {
    Ok(whatsapp_manager.has_credentials(&account_id).await)
}

/// Logout WhatsApp account
#[tauri::command]
pub async fn logout_whatsapp(
    whatsapp_manager: State<'_, Arc<WhatsAppManager>>,
    app_handle: AppHandle,
    account_id: String,
) -> Result<(), String> {
    whatsapp_manager
        .logout(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app_handle.emit(
        "whatsapp:logged-out",
        serde_json::json!({ "accountId": account_id }),
    );

    Ok(())
}

/// List WhatsApp accounts
#[tauri::command]
pub async fn list_whatsapp_accounts(
    whatsapp_manager: State<'_, Arc<WhatsAppManager>>,
) -> Result<Vec<String>, String> {
    Ok(whatsapp_manager.list_accounts().await)
}

// ==================== Helper Functions ====================

/// Helper trait for method chaining
pub trait Pipe: Sized {
    fn pipe<B, F>(self, f: F) -> B
    where
        F: FnOnce(Self) -> B,
    {
        f(self)
    }
}

impl<T> Pipe for T {}
