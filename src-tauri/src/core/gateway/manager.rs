//! Gateway Manager
//!
//! Coordinates Gateway process lifecycle and WebSocket communication.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tauri::{Manager, Emitter};

use super::process::{GatewayProcess, GatewayLaunchConfig, DEFAULT_GATEWAY_PORT};
use super::websocket::{GatewayWebSocket, wait_for_gateway_ready};

/// Gateway status for external consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayStatus {
    /// Current state
    pub state: String,
    /// Port number
    pub port: u16,
    /// Process ID (if running)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Connection timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_at: Option<i64>,
    /// Error message (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Reconnection attempts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconnect_attempts: Option<u32>,
}

/// Reconnection configuration
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Maximum reconnection attempts
    pub max_attempts: u32,
    /// Base delay between attempts (ms)
    pub base_delay_ms: u64,
    /// Maximum delay between attempts (ms)
    pub max_delay_ms: u64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

/// Gateway Manager
pub struct GatewayManager {
    /// Gateway process
    process: Arc<RwLock<Option<GatewayProcess>>>,
    /// WebSocket client
    websocket: Arc<RwLock<Option<GatewayWebSocket>>>,
    /// Current status
    status: Arc<RwLock<GatewayStatus>>,
    /// Reconnection configuration
    reconnect_config: ReconnectConfig,
    /// Should auto-reconnect
    should_reconnect: Arc<RwLock<bool>>,
    /// Reconnection attempts counter
    reconnect_attempts: Arc<RwLock<u32>>,
    /// Device identity
    device_identity: Arc<RwLock<Option<Arc<crate::core::auth::DeviceIdentity>>>>,
    /// Tauri app handle for emitting events
    app_handle: Arc<RwLock<Option<tauri::AppHandle>>>,
}

impl GatewayManager {
    /// Create a new Gateway Manager
    pub fn new() -> Self {
        Self {
            process: Arc::new(RwLock::new(None)),
            websocket: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(GatewayStatus {
                state: "stopped".to_string(),
                port: DEFAULT_GATEWAY_PORT,
                pid: None,
                connected_at: None,
                error: None,
                reconnect_attempts: None,
            })),
            reconnect_config: ReconnectConfig::default(),
            should_reconnect: Arc::new(RwLock::new(true)),
            reconnect_attempts: Arc::new(RwLock::new(0)),
            device_identity: Arc::new(RwLock::new(None)),
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the Tauri app handle for emitting events
    pub async fn set_app_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.write().await = Some(handle);
    }

    /// Emit a status update event to the frontend
    async fn emit_status_update(&self) {
        if let Some(app) = self.app_handle.read().await.as_ref() {
            let status = self.status.read().await.clone();
            tracing::debug!("Emitting gateway:status event: {:?}", status);
            app.emit("gateway:status", &status).unwrap_or_else(|e| {
                tracing::warn!("Failed to emit gateway:status event: {}", e);
            });
        }
    }

    /// Get current Gateway status
    pub async fn get_status(&self) -> GatewayStatus {
        self.status.read().await.clone()
    }

    /// Check if Gateway is connected
    pub async fn is_connected(&self) -> bool {
        let status = self.status.read().await;
        status.state == "running"
    }

    /// Initialize device identity
    async fn init_device_identity(&self) -> Result<()> {
        let mut identity_guard = self.device_identity.write().await;
        if identity_guard.is_some() {
            return Ok(());
        }

        let path = crate::core::auth::get_device_identity_path();
        tracing::info!("Loading or creating device identity at: {:?}", path);

        match crate::core::auth::DeviceIdentity::load_or_create(&path) {
            Ok(identity) => {
                tracing::info!("Device identity loaded successfully (device_id={})", identity.device_id);
                *identity_guard = Some(Arc::new(identity));
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to initialize device identity: {}", e);
                Err(e)
            }
        }
    }

    /// Start the Gateway
    pub async fn start(&self, token: String) -> Result<()> {
        // Check current status
        {
            let status = self.status.read().await;
            if status.state == "running" {
                tracing::debug!("Gateway already running, skipping start");
                return Ok(());
            }
        }

        // Update status to starting
        {
            let mut status = self.status.write().await;
            status.state = "starting".to_string();
            status.error = None;
        }
        self.emit_status_update().await;

        tracing::info!("Starting Gateway (port={})", DEFAULT_GATEWAY_PORT);

        // Check if there's already a Gateway running on this port
        if super::process::is_gateway_running_on_port(DEFAULT_GATEWAY_PORT) {
            tracing::warn!("Found existing Gateway process on port {}, attempting to stop it...", DEFAULT_GATEWAY_PORT);

            if let Err(e) = super::process::kill_gateway_on_port(DEFAULT_GATEWAY_PORT) {
                tracing::warn!("Failed to kill existing Gateway process: {}", e);
            } else {
                tracing::info!("Successfully stopped existing Gateway process");
            }
        }

        // Initialize device identity
        if let Err(e) = self.init_device_identity().await {
            tracing::error!("Failed to initialize device identity: {}. Gateway handshake may fail.", e);
            // Don't return error - allow Gateway to start anyway
        }

        // Reset reconnection attempts
        *self.reconnect_attempts.write().await = 0;
        *self.should_reconnect.write().await = true;

        // Create and start process
        let config = GatewayLaunchConfig {
            port: DEFAULT_GATEWAY_PORT,
            token: token.clone(),
            skip_channels: false,
            env: vec![],
        };

        let mut process = GatewayProcess::new(config);
        if let Err(e) = process.start() {
            let mut status = self.status.write().await;
            status.state = "error".to_string();
            status.error = Some(e.to_string());
            return Err(e.context("Failed to start Gateway process"));
        }

        let pid = process.pid();

        // Store process reference
        *self.process.write().await = Some(process);

        // Update status with PID
        {
            let mut status = self.status.write().await;
            status.pid = pid;
        }

        // Wait for Gateway to become ready
        tracing::info!("Waiting for Gateway to become ready...");
        if let Err(e) = wait_for_gateway_ready(DEFAULT_GATEWAY_PORT, 30, 500).await {
            let mut status = self.status.write().await;
            status.state = "error".to_string();
            status.error = Some(format!("Gateway failed to become ready: {}", e));

            // Check if process is still running
            if let Some(mut p) = self.process.write().await.take() {
                if !p.is_running() {
                    tracing::error!("Gateway process exited unexpectedly");
                }
            }

            return Err(e.context("Gateway failed to become ready"));
        }

        tracing::info!("Gateway is ready, connecting WebSocket...");

        // Connect WebSocket with token
        let device_identity = self.device_identity.read().await.clone();
        let mut ws = GatewayWebSocket::new(DEFAULT_GATEWAY_PORT, device_identity, Some(token));
        if let Err(e) = ws.connect().await {
            let mut status = self.status.write().await;
            status.state = "error".to_string();
            status.error = Some(format!("WebSocket connection failed: {}", e));
            return Err(e.context("Failed to connect WebSocket"));
        }

        *self.websocket.write().await = Some(ws);

        // Update status to running
        {
            let mut status = self.status.write().await;
            status.state = "running".to_string();
            status.connected_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
            );
            status.reconnect_attempts = None;
        }

        tracing::info!("Gateway started successfully");
        self.emit_status_update().await;

        Ok(())
    }

    /// Stop the Gateway
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping Gateway");

        *self.should_reconnect.write().await = false;

        // Close WebSocket
        if let Some(mut ws) = self.websocket.write().await.take() {
            ws.close().await;
        }

        // Stop process
        if let Some(mut process) = self.process.write().await.take() {
            process.stop().context("Failed to stop Gateway process")?;
        }

        // Update status
        {
            let mut status = self.status.write().await;
            status.state = "stopped".to_string();
            status.pid = None;
            status.connected_at = None;
            status.error = None;
        }

        tracing::info!("Gateway stopped");
        self.emit_status_update().await;
        Ok(())
    }

    /// Restart the Gateway
    pub async fn restart(&self, token: String) -> Result<()> {
        tracing::debug!("Restarting Gateway");
        self.stop().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        self.start(token).await
    }

    /// Send an RPC request to the Gateway
    pub async fn rpc(&self, method: &str, params: Option<serde_json::Value>, timeout_ms: u64) -> Result<serde_json::Value> {
        let ws_guard = self.websocket.read().await;
        let ws = ws_guard.as_ref().context("Gateway not connected")?;
        ws.rpc(method, params, timeout_ms).await
    }

    /// Check Gateway health
    pub async fn check_health(&self) -> Result<bool> {
        let status = self.status.read().await;
        if status.state != "running" {
            return Ok(false);
        }

        // Check if process is still running
        let mut process_guard = self.process.write().await;
        if let Some(process) = process_guard.as_mut() {
            if !process.is_running() {
                tracing::warn!("Gateway process died unexpectedly");
                return Ok(false);
            }
        }

        // Check if WebSocket is connected
        let ws_guard = self.websocket.read().await;
        if let Some(ws) = ws_guard.as_ref() {
            return Ok(ws.is_connected());
        }

        Ok(false)
    }
}

impl Default for GatewayManager {
    fn default() -> Self {
        Self::new()
    }
}