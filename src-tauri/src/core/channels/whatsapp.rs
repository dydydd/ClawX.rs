//! WhatsApp integration module
//!
//! This module provides WhatsApp login functionality using a QR code-based
//! authentication flow. It manages the WhatsApp session state and emits events
//! for QR code updates and connection status changes.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::Duration;

use super::config::{ChannelManager, ChannelStatus};

/// WhatsApp login state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WhatsAppLoginState {
    /// Not logged in
    Idle,
    /// Waiting for QR code scan
    AwaitingQr,
    /// QR code scanned, connecting
    Connecting,
    /// Connected and authenticated
    Connected,
    /// Login failed or logged out
    Error,
}

impl Default for WhatsAppLoginState {
    fn default() -> Self {
        WhatsAppLoginState::Idle
    }
}

/// QR Code data for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QRCodeData {
    /// Base64-encoded PNG image
    pub base64_png: String,
    /// Raw QR code string
    pub raw: String,
    /// Timestamp when QR was generated
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// WhatsApp account session info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppSession {
    /// Account ID
    pub account_id: String,
    /// Phone number (populated after login)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    /// Push name (display name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_name: Option<String>,
    /// Whether session is authenticated
    pub authenticated: bool,
}

/// WhatsApp login manager
///
/// Manages the WhatsApp login process using a Node.js subprocess
/// that runs the Baileys library for QR code generation.
pub struct WhatsAppManager {
    channel_manager: Arc<ChannelManager>,
    /// Active login processes by account ID
    active_logins: Arc<RwLock<HashMap<String, WhatsAppLoginHandle>>>,
    /// Credentials base directory
    credentials_dir: PathBuf,
}

/// Handle to an active WhatsApp login process
struct WhatsAppLoginHandle {
    /// Account ID being logged in
    account_id: String,
    /// Current login state
    state: WhatsAppLoginState,
    /// Latest QR code data
    qr_code: Option<QRCodeData>,
    /// Error message if any
    error_message: Option<String>,
    /// Sender for forwarding events to the caller
    event_tx: mpsc::Sender<WhatsAppLoginEvent>,
    /// Child process handle (stored in Arc<Mutex<>> for shared access)
    #[allow(dead_code)]
    _child: Arc<Mutex<Option<Child>>>,
}

/// Events emitted during WhatsApp login
#[derive(Debug, Clone)]
pub enum WhatsAppLoginEvent {
    /// QR code received
    QrCode(QRCodeData),
    /// Connection opened
    Connected { account_id: String },
    /// Connection closed/error
    Error { message: String },
    /// Login process stopped
    Stopped,
}

impl WhatsAppManager {
    /// Create a new WhatsApp manager
    pub async fn new(channel_manager: Arc<ChannelManager>) -> Result<Self> {
        let credentials_dir = Self::get_credentials_dir()?;

        // Ensure credentials directory exists
        tokio::fs::create_dir_all(&credentials_dir)
            .await
            .context("Failed to create WhatsApp credentials directory")?;

        Ok(Self {
            channel_manager,
            active_logins: Arc::new(RwLock::new(HashMap::new())),
            credentials_dir,
        })
    }

    /// Get the credentials directory path
    fn get_credentials_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        Ok(home.join(".openclaw").join("credentials").join("whatsapp"))
    }

    /// Get the auth directory for a specific account
    pub fn get_account_auth_dir(&self, account_id: &str) -> PathBuf {
        self.credentials_dir.join(account_id)
    }

    /// Check if an account has existing credentials
    pub async fn has_credentials(&self, account_id: &str) -> bool {
        let auth_dir = self.get_account_auth_dir(account_id);
        if !auth_dir.exists() {
            return false;
        }

        // Check for creds.json which indicates a valid session
        let creds_file = auth_dir.join("creds.json");
        creds_file.exists()
    }

    /// Start WhatsApp login process
    pub async fn start_login(
        &self,
        account_id: &str,
        _app_handle: Option<tauri::AppHandle>,
    ) -> Result<mpsc::Receiver<WhatsAppLoginEvent>> {
        // Check if already logging in
        {
            let active_logins = self.active_logins.read().await;
            if active_logins.contains_key(account_id) {
                anyhow::bail!("Login already in progress for account: {}", account_id);
            }
        }

        // Create or update the channel
        let channel_id = format!("whatsapp-{}", account_id);
        let channel = self
            .channel_manager
            .get_or_create_channel(channel_id.clone(), "whatsapp".to_string())
            .await?;

        // Update channel status to connecting
        self.channel_manager
            .update_channel_status(&channel_id, ChannelStatus::Connecting, Some("Starting login...".to_string()))
            .await?;

        // Create event channel
        let (event_tx, event_rx) = mpsc::channel(100);

        // Start the Node.js process for WhatsApp login
        let _child = self
            .spawn_login_process(account_id, event_tx.clone(), _app_handle)
            .await?;

        // Create a separate channel for the caller
        let (caller_tx, caller_rx) = mpsc::channel(100);

        // Store the login handle with caller's sender
        let handle = WhatsAppLoginHandle {
            account_id: account_id.to_string(),
            state: WhatsAppLoginState::AwaitingQr,
            qr_code: None,
            error_message: None,
            event_tx: caller_tx,
            _child: Arc::new(Mutex::new(Some(_child))),
        };

        {
            let mut active_logins = self.active_logins.write().await;
            active_logins.insert(account_id.to_string(), handle);
        }

        // Clone for the spawned task
        let active_logins_clone = Arc::clone(&self.active_logins);
        let channel_manager_clone = Arc::clone(&self.channel_manager);
        let account_id_clone = account_id.to_string();

        // Spawn a task to handle events and forward to caller
        tokio::spawn(async move {
            Self::handle_login_events(
                &account_id_clone,
                event_rx,
                active_logins_clone,
                channel_manager_clone,
            )
            .await;
        });

        Ok(caller_rx)
    }

    /// Spawn the Node.js login process
    async fn spawn_login_process(
        &self,
        account_id: &str,
        event_tx: mpsc::Sender<WhatsAppLoginEvent>,
        _app_handle: Option<tauri::AppHandle>,
    ) -> Result<Child> {
        let auth_dir = self.get_account_auth_dir(account_id);

        // Ensure auth directory exists
        tokio::fs::create_dir_all(&auth_dir)
            .await
            .context("Failed to create auth directory")?;

        // For now, we simulate the login process since integrating Baileys
        // requires a more complex setup. In a real implementation, this would:
        // 1. Spawn a Node.js process running a Baileys-based script
        // 2. Communicate via stdin/stdout or WebSocket
        // 3. Parse events and forward them to the event channel

        // Simulate the QR code generation and connection flow
        let account_id_clone = account_id.to_string();
        tokio::spawn(async move {
            // Simulate QR code generation delay
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Generate a fake QR code for demonstration
            // In real implementation, this would come from Baileys
            let fake_qr = "FAKE_QR_CODE_FOR_TESTING".to_string();
            let qr_data = QRCodeData {
                base64_png: generate_placeholder_qr(&fake_qr),
                raw: fake_qr,
                timestamp: chrono::Utc::now(),
            };

            let _ = event_tx.send(WhatsAppLoginEvent::QrCode(qr_data)).await;

            // Note: In real implementation, we would wait for user to scan QR
            // and then receive connection events from Baileys
        });

        // Return a dummy child process (not used in simulation)
        // In real implementation, this would be the actual Node.js process
        let dummy_child = Command::new("echo")
            .arg("WhatsApp login process placeholder")
            .spawn()
            .context("Failed to spawn dummy process")?;

        Ok(dummy_child)
    }

    /// Handle login events
    async fn handle_login_events(
        account_id: &str,
        mut event_rx: mpsc::Receiver<WhatsAppLoginEvent>,
        active_logins: Arc<RwLock<HashMap<String, WhatsAppLoginHandle>>>,
        channel_manager: Arc<ChannelManager>,
    ) {
        let channel_id = format!("whatsapp-{}", account_id);

        while let Some(event) = event_rx.recv().await {
            match &event {
                WhatsAppLoginEvent::QrCode(qr) => {
                    tracing::info!("Received QR code for {}", account_id);

                    // Update login handle
                    {
                        let mut logins = active_logins.write().await;
                        if let Some(handle) = logins.get_mut(account_id) {
                            handle.state = WhatsAppLoginState::AwaitingQr;
                            handle.qr_code = Some(qr.clone());
                        }
                    }

                    // Update channel status
                    let _ = channel_manager
                        .update_channel_status(
                            &channel_id,
                            ChannelStatus::Connecting,
                            Some("Waiting for QR scan...".to_string()),
                        )
                        .await;
                }
                WhatsAppLoginEvent::Connected { account_id: _ } => {
                    tracing::info!("WhatsApp connected for {}", account_id);

                    // Update login handle
                    {
                        let mut logins = active_logins.write().await;
                        if let Some(handle) = logins.get_mut(account_id) {
                            handle.state = WhatsAppLoginState::Connected;
                        }
                    }

                    // Update channel status
                    let _ = channel_manager
                        .update_channel_status(
                            &channel_id,
                            ChannelStatus::Connected,
                            Some("Connected".to_string()),
                        )
                        .await;

                    break;
                }
                WhatsAppLoginEvent::Error { message } => {
                    tracing::error!("WhatsApp login error for {}: {}", account_id, message);

                    // Update login handle
                    {
                        let mut logins = active_logins.write().await;
                        if let Some(handle) = logins.get_mut(account_id) {
                            handle.state = WhatsAppLoginState::Error;
                            handle.error_message = Some(message.clone());
                        }
                    }

                    // Update channel status
                    let _ = channel_manager
                        .update_channel_status(
                            &channel_id,
                            ChannelStatus::Error,
                            Some(message.clone()),
                        )
                        .await;

                    break;
                }
                WhatsAppLoginEvent::Stopped => {
                    tracing::info!("WhatsApp login stopped for {}", account_id);
                    break;
                }
            }
        }

        // Remove from active logins
        {
            let mut logins = active_logins.write().await;
            logins.remove(account_id);
        }
    }

    /// Stop the login process for an account
    pub async fn stop_login(&self, account_id: &str) -> Result<()> {
        let mut active_logins = self.active_logins.write().await;

        if let Some(handle) = active_logins.remove(account_id) {
            // In real implementation, we would kill the child process here
            tracing::info!("Stopped WhatsApp login for {}", account_id);
        }

        Ok(())
    }

    /// Get current QR code for an account (if available)
    pub async fn get_qr_code(&self, account_id: &str) -> Option<QRCodeData> {
        let active_logins = self.active_logins.read().await;
        active_logins
            .get(account_id)
            .and_then(|h| h.qr_code.clone())
    }

    /// Get login state for an account
    pub async fn get_login_state(&self, account_id: &str) -> WhatsAppLoginState {
        let active_logins = self.active_logins.read().await;
        active_logins
            .get(account_id)
            .map(|h| h.state.clone())
            .unwrap_or(WhatsAppLoginState::Idle)
    }

    /// Logout and remove credentials for an account
    pub async fn logout(&self, account_id: &str) -> Result<()> {
        // Stop any active login
        self.stop_login(account_id).await.ok();

        // Remove credentials directory
        let auth_dir = self.get_account_auth_dir(account_id);
        if auth_dir.exists() {
            tokio::fs::remove_dir_all(&auth_dir)
                .await
                .context("Failed to remove credentials directory")?;
        }

        // Update channel status
        let channel_id = format!("whatsapp-{}", account_id);
        self.channel_manager
            .update_channel_status(&channel_id, ChannelStatus::Disconnected, None)
            .await?;

        tracing::info!("Logged out WhatsApp account: {}", account_id);
        Ok(())
    }

    /// List all WhatsApp accounts (based on credential directories)
    pub async fn list_accounts(&self) -> Vec<String> {
        let mut accounts = Vec::new();

        if let Ok(mut entries) = tokio::fs::read_dir(&self.credentials_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(file_type) = entry.file_type().await {
                    if file_type.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            accounts.push(name.to_string());
                        }
                    }
                }
            }
        }

        accounts
    }

    /// Check if any login is in progress
    pub async fn is_login_in_progress(&self, account_id: &str) -> bool {
        let active_logins = self.active_logins.read().await;
        active_logins.contains_key(account_id)
    }
}

/// Generate a placeholder QR code PNG (base64 encoded)
///
/// In a real implementation, this would use a QR code library like `qrcode`
/// to generate an actual QR code image.
fn generate_placeholder_qr(data: &str) -> String {
    // This is a minimal 1x1 transparent PNG as placeholder
    // In production, use a proper QR code generation library
    let _ = data;
    let png_bytes = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1 dimensions
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, // 8-bit RGBA
        0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x08, 0xD7, 0x63, 0xF8, 0x0F, 0x00, 0x00,
        0x01, 0x01, 0x00, 0x05, 0x18, 0xD8, 0x4E, 0x00,
        0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, // IEND chunk
        0x42, 0x60, 0x82,
    ];
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(&png_bytes)
}

/// Generate a real QR code PNG using the `qrcode` crate
///
/// This function generates a proper QR code image with the specified data.
/// It returns a base64-encoded PNG string suitable for display in the frontend.
#[cfg(feature = "qrcode")]
pub fn generate_qr_code_png(data: &str, size: u32) -> Result<String> {
    use qrcode::QrCode;
    use qrcode::render::svg;

    let code = QrCode::new(data.as_bytes())
        .context("Failed to create QR code")?;

    // Generate SVG representation
    let svg = code.render::<svg::Color>()
        .min_dimensions(size, size)
        .build();

    // For PNG output, we'd use image crate, but SVG is more compact
    // and works well for QR codes in web contexts
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(svg.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whatsapp_login_state_serialization() {
        let state = WhatsAppLoginState::AwaitingQr;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"awaiting_qr\"");

        let deserialized: WhatsAppLoginState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, state);
    }
}
