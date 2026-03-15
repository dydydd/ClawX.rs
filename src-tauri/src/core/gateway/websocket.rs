//! WebSocket client for Gateway communication
//!
//! Handles WebSocket connection, handshake protocol, and message routing.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{timeout, sleep, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

use crate::core::auth::DeviceIdentity;

/// Default WebSocket connection timeout
const CONNECT_TIMEOUT_SECS: u64 = 10;

/// Default handshake timeout
const HANDSHAKE_TIMEOUT_SECS: u64 = 10;

/// Gateway WebSocket URL
fn gateway_ws_url(port: u16) -> String {
    format!("ws://localhost:{}/ws", port)
}

/// Gateway protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum GatewayMessage {
    /// Request message
    Req {
        id: String,
        method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
    },
    /// Response message
    Res {
        id: String,
        #[serde(default)]
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<GatewayError>,
    },
    /// Event message (notification)
    Event {
        event: String,
        #[serde(default)]
        payload: Value,
    },
}

/// Gateway error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
}

/// Pending RPC request
struct PendingRequest {
    tx: oneshot::Sender<Result<Value>>,
    timeout: Instant,
}

/// Connect challenge event payload
#[derive(Debug, Clone, Deserialize)]
struct ConnectChallenge {
    #[serde(default)]
    nonce: String,
}

/// Gateway WebSocket client state
pub struct GatewayWebSocket {
    /// WebSocket URL
    url: String,
    /// Device identity for handshake
    device_identity: Option<Arc<DeviceIdentity>>,
    /// Gateway token for authentication
    gateway_token: Option<String>,
    /// Platform identifier
    platform: String,
    /// Pending requests waiting for responses
    pending_requests: Arc<RwLock<HashMap<String, PendingRequest>>>,
    /// Message sender channel
    tx: Option<mpsc::Sender<GatewayMessage>>,
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl GatewayWebSocket {
    /// Create a new WebSocket client
    pub fn new(port: u16, device_identity: Option<Arc<DeviceIdentity>>, gateway_token: Option<String>) -> Self {
        Self {
            url: gateway_ws_url(port),
            device_identity,
            gateway_token,
            platform: std::env::consts::OS.to_string(),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            tx: None,
            shutdown_tx: None,
        }
    }

    /// Connect to the Gateway and perform handshake
    pub async fn connect(&mut self) -> Result<()> {
        tracing::info!("Connecting to Gateway WebSocket: {}", self.url);

        // Connect with timeout
        let ws_result = timeout(
            Duration::from_secs(CONNECT_TIMEOUT_SECS),
            connect_async(&self.url),
        )
        .await;

        let (ws_stream, _) = match ws_result {
            Ok(Ok(result)) => {
                tracing::info!("WebSocket TCP connection established");
                result
            }
            Ok(Err(e)) => {
                tracing::error!("WebSocket connection failed: {}", e);
                return Err(e).context("Failed to connect WebSocket");
            }
            Err(e) => {
                tracing::error!("WebSocket connection timeout after {} seconds", CONNECT_TIMEOUT_SECS);
                return Err(e).context("WebSocket connection timeout");
            }
        };

        tracing::info!("WebSocket connected, waiting for connect.challenge...");

        // Create channels for message handling
        let (tx, mut rx) = mpsc::channel::<GatewayMessage>(100);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        self.tx = Some(tx.clone());
        self.shutdown_tx = Some(shutdown_tx);

        // Split WebSocket into read and write halves
        let (write, mut read) = ws_stream.split();

        // Spawn message sender task
        let pending = self.pending_requests.clone();
        let write_task = tokio::spawn(async move {
            let mut write = write;
            while let Some(msg) = rx.recv().await {
                let json = serde_json::to_string(&msg).unwrap();
                if write.send(WsMessage::Text(json)).await.is_err() {
                    break;
                }
            }
        });

        // Wait for connect.challenge event
        let challenge = timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
            async {
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(WsMessage::Text(text)) => {
                            tracing::trace!("Received WebSocket message: {}", text);
                            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                                if json.get("type") == Some(&Value::String("event".to_string()))
                                    && json.get("event") == Some(&Value::String("connect.challenge".to_string()))
                                {
                                    tracing::info!("Received connect.challenge event");
                                    if let Some(payload) = json.get("payload") {
                                        let challenge: ConnectChallenge =
                                            serde_json::from_value(payload.clone())?;
                                        return Ok(challenge);
                                    }
                                }
                            }
                        }
                        Ok(WsMessage::Ping(data)) => {
                            // Respond to ping with pong
                        }
                        Ok(WsMessage::Close(_)) => {
                            return Err(anyhow::anyhow!("WebSocket closed before handshake"));
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!("WebSocket error during handshake: {}", e));
                        }
                        _ => {}
                    }
                }
                Err(anyhow::anyhow!("WebSocket stream ended before handshake"))
            },
        )
        .await
        .context("Handshake timeout")?
        .context("Handshake failed")?;

        let nonce = challenge.nonce;
        tracing::info!("Received connect.challenge, nonce={}", nonce);

        // Send connect handshake
        let connect_frame = self.build_connect_frame(&nonce);
        tracing::info!("Sending connect handshake");
        tx.send(connect_frame).await.context("Failed to send connect frame")?;

        // Wait for connect response
        let connect_result = timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
            async {
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(WsMessage::Text(text)) => {
                            if let Ok(response) = serde_json::from_str::<GatewayMessage>(&text) {
                                match response {
                                    GatewayMessage::Res { id, ok, payload, error } => {
                                        if id.starts_with("connect-") {
                                            if ok {
                                                return Ok(());
                                            } else {
                                                let err_msg = error
                                                    .map(|e| e.message)
                                                    .unwrap_or_else(|| "Unknown error".to_string());
                                                return Err(anyhow::anyhow!("Handshake failed: {}", err_msg));
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(WsMessage::Close(_)) => {
                            return Err(anyhow::anyhow!("WebSocket closed during handshake"));
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!("WebSocket error: {}", e));
                        }
                        _ => {}
                    }
                }
                Err(anyhow::anyhow!("WebSocket stream ended during handshake"))
            },
        )
        .await
        .context("Connect response timeout")?;

        connect_result?;

        tracing::info!("Gateway WebSocket handshake completed");

        // Spawn message receiver task
        let pending_clone = self.pending_requests.clone();
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        if let Ok(message) = serde_json::from_str::<GatewayMessage>(&text) {
                            Self::handle_message(&pending_clone, message).await;
                        }
                    }
                    Ok(WsMessage::Ping(data)) => {
                        // Pong is handled automatically by tungstenite
                    }
                    Ok(WsMessage::Close(_)) => {
                        tracing::debug!("WebSocket closed");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// Build the connect handshake frame
    fn build_connect_frame(&self, nonce: &str) -> GatewayMessage {
        let connect_id = format!("connect-{}", uuid::Uuid::new_v4());
        let role = "operator";
        let scopes = vec!["operator.admin"];
        let signed_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let client_id = "gateway-client";
        let client_mode = "ui";

        // Try simple token auth first (fallback approach)
        // If we have a token, use it directly in auth
        if let Some(token) = &self.gateway_token {
            tracing::info!("Using token-based authentication (no device signature)");

            return GatewayMessage::Req {
                id: connect_id,
                method: "connect".to_string(),
                params: Some(serde_json::json!({
                    "minProtocol": 3,
                    "maxProtocol": 3,
                    "client": {
                        "id": client_id,
                        "displayName": "ClawX",
                        "version": env!("CARGO_PKG_VERSION"),
                        "platform": self.platform,
                        "mode": client_mode,
                    },
                    "auth": {
                        "token": token,
                    },
                    "caps": [],
                    "role": role,
                    "scopes": scopes,
                })),
            };
        }

        // Build device authentication if no token
        let device = self.device_identity.as_ref().map(|identity| {
            let payload = build_device_auth_payload(&BuildDeviceAuthParams {
                device_id: &identity.device_id,
                client_id,
                client_mode,
                role,
                scopes: &scopes,
                signed_at_ms,
                token: None,
                nonce: Some(nonce),
                version: "v2",
            });

            tracing::debug!("Device auth payload: {}", payload);

            let signature = identity.sign_payload(&payload);
            let public_key = URL_SAFE_NO_PAD.encode(identity.verifying_key().as_bytes());

            tracing::debug!("Device signature: {}", signature);

            serde_json::json!({
                "id": identity.device_id,
                "publicKey": public_key,
                "signature": signature,
                "signedAt": signed_at_ms,
                "nonce": nonce,
            })
        });

        GatewayMessage::Req {
            id: connect_id,
            method: "connect".to_string(),
            params: Some(serde_json::json!({
                "minProtocol": 3,
                "maxProtocol": 3,
                "client": {
                    "id": client_id,
                    "displayName": "ClawX",
                    "version": env!("CARGO_PKG_VERSION"),
                    "platform": self.platform,
                    "mode": client_mode,
                },
                "auth": {},
                "caps": [],
                "role": role,
                "scopes": scopes,
                "device": device,
            })),
        }
    }

    /// Handle incoming message
    async fn handle_message(pending: &Arc<RwLock<HashMap<String, PendingRequest>>>, message: GatewayMessage) {
        match message {
            GatewayMessage::Res { id, ok, payload, error } => {
                let mut pending_guard = pending.write().await;
                if let Some(req) = pending_guard.remove(&id) {
                    let result = if ok {
                        Ok(payload.unwrap_or(Value::Null))
                    } else {
                        let err_msg = error
                            .map(|e| e.message)
                            .unwrap_or_else(|| "Unknown error".to_string());
                        Err(anyhow::anyhow!("{}", err_msg))
                    };
                    let _ = req.tx.send(result);
                }
            }
            GatewayMessage::Event { event, payload } => {
                tracing::debug!("Gateway event: {} {:?}", event, payload);
                // TODO: Emit event to frontend
            }
            _ => {}
        }
    }

    /// Send an RPC request and wait for response
    pub async fn rpc(&self, method: &str, params: Option<Value>, timeout_ms: u64) -> Result<Value> {
        let tx = self.tx.as_ref().context("WebSocket not connected")?;

        let id = uuid::Uuid::new_v4().to_string();
        let (resp_tx, resp_rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(
                id.clone(),
                PendingRequest {
                    tx: resp_tx,
                    timeout: Instant::now() + Duration::from_millis(timeout_ms),
                },
            );
        }

        // Send request
        let message = GatewayMessage::Req {
            id: id.clone(),
            method: method.to_string(),
            params,
        };
        tx.send(message).await.context("Failed to send RPC request")?;

        // Wait for response with timeout
        let result = timeout(Duration::from_millis(timeout_ms), resp_rx)
            .await
            .context("RPC timeout")?
            .context("RPC channel closed")??;

        Ok(result)
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.tx.is_some()
    }

    /// Get the gateway token
    pub fn get_token(&self) -> &Option<String> {
        &self.gateway_token
    }

    /// Close the connection
    pub async fn close(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        self.tx = None;
    }
}

/// Parameters for building device auth payload
struct BuildDeviceAuthParams<'a> {
    device_id: &'a str,
    client_id: &'a str,
    client_mode: &'a str,
    role: &'a str,
    scopes: &'a [&'a str],
    signed_at_ms: i64,
    token: Option<&'a str>,
    nonce: Option<&'a str>,
    version: &'a str,
}

/// Build the canonical device auth payload string
fn build_device_auth_payload(params: &BuildDeviceAuthParams<'_>) -> String {
    let scopes = params.scopes.join(",");
    let token = params.token.unwrap_or("");
    let signed_at = params.signed_at_ms.to_string();
    let mut parts = vec![
        params.version,
        params.device_id,
        params.client_id,
        params.client_mode,
        params.role,
        &scopes,
        &signed_at,
        token,
    ];

    if params.version == "v2" {
        parts.push(params.nonce.unwrap_or(""));
    }

    parts.join("|")
}

/// Probe if Gateway is ready to accept connections
pub async fn probe_gateway_ready(port: u16, timeout_ms: u64) -> bool {
    let url = gateway_ws_url(port);

    match timeout(Duration::from_millis(timeout_ms), async {
        let (ws_stream, _) = connect_async(&url).await?;

        let (write, mut read) = ws_stream.split();

        // Wait for connect.challenge
        while let Some(msg) = read.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Ok(json) = serde_json::from_str::<Value>(&text) {
                        if json.get("type") == Some(&Value::String("event".to_string()))
                            && json.get("event") == Some(&Value::String("connect.challenge".to_string()))
                        {
                            return Ok::<_, anyhow::Error>(true);
                        }
                    }
                }
                Ok(WsMessage::Close(_)) | Err(_) => break,
                _ => {}
            }
        }

        Ok(false)
    })
    .await
    {
        Ok(Ok(true)) => true,
        _ => false,
    }
}

/// Wait for Gateway to become ready
pub async fn wait_for_gateway_ready(port: u16, max_retries: u32, interval_ms: u64) -> Result<()> {
    for i in 0..max_retries {
        if probe_gateway_ready(port, 1500).await {
            tracing::debug!("Gateway ready after {} attempts", i + 1);
            return Ok(());
        }

        if i > 0 && i % 10 == 0 {
            tracing::debug!("Still waiting for Gateway... (attempt {}/{})", i + 1, max_retries);
        }

        sleep(Duration::from_millis(interval_ms)).await;
    }

    anyhow::bail!("Gateway failed to become ready after {} attempts on port {}", max_retries, port)
}