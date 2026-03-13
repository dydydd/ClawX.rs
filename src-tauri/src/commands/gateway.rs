//! Gateway IPC command handlers

use std::sync::Arc;
use serde::Serialize;
use tauri::State;
use crate::core::AppState;

/// Get the current gateway status
#[tauri::command]
pub async fn gateway_get_status(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::core::gateway::GatewayStatus, String> {
    Ok(state.gateway.get_status().await)
}

/// Start the gateway process
#[tauri::command]
pub async fn gateway_start(
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Get gateway token from settings
    let token = {
        let settings = state.settings.read().await;
        settings.get("gatewayToken")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default()
    };

    state.gateway.start(token).await.map_err(|e| e.to_string())
}

/// Stop the gateway process
#[tauri::command]
pub async fn gateway_stop(
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.gateway.stop().await.map_err(|e| e.to_string())
}

/// Send an RPC command to the gateway
#[tauri::command]
pub async fn gateway_rpc(
    method: String,
    params: Option<serde_json::Value>,
    timeout_ms: Option<u64>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let timeout = timeout_ms.unwrap_or(30000);
    state.gateway
        .rpc(&method, params, timeout)
        .await
        .map_err(|e| e.to_string())
}