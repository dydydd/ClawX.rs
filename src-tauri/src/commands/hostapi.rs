//! Host API HTTP proxy command handlers
//!
//! Provides HTTP fetch capability through the backend, allowing the frontend
//! to make HTTP requests via IPC (useful for proxy support and CORS bypass).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default Gateway port
const DEFAULT_GATEWAY_PORT: u16 = 18789;

/// HTTP request options (legacy format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchOptions {
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// HTTP response (legacy format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub ok: bool,
}

/// Host API fetch request (new format matching frontend)
#[derive(Debug, Deserialize)]
pub struct HostApiFetchRequest {
    pub path: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

/// Host API fetch response (new format matching frontend)
#[derive(Debug, Serialize)]
pub struct HostApiFetchResponse {
    pub ok: bool,
    pub data: Option<HostApiFetchData>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HostApiFetchData {
    pub status: u16,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Fetch from Gateway HTTP API (new format)
#[tauri::command]
pub async fn hostapi_fetch(
    request: HostApiFetchRequest,
    state: tauri::State<'_, std::sync::Arc<crate::core::AppState>>,
) -> Result<HostApiFetchResponse, String> {
    let path = request.path;

    if !path.starts_with('/') {
        return Ok(HostApiFetchResponse {
            ok: false,
            data: None,
            error: Some(format!("Invalid path: {}", path)),
        });
    }

    let method = if request.method.is_empty() {
        "GET"
    } else {
        &request.method
    };

    let url = format!("http://127.0.0.1:{}{}", DEFAULT_GATEWAY_PORT, path);

    tracing::debug!("Host API proxy: {} {}", method, url);

    // Get gateway token from settings
    let gateway_token = {
        let settings = state.settings.read().await;
        settings.get("gatewayToken")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default()
    };

    // Build client
    let client = reqwest::Client::new();
    let mut req = match method {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        "HEAD" => client.head(&url),
        _ => client.request(reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET), &url),
    };

    // Add headers
    for (key, value) in request.headers.iter() {
        req = req.header(key, value);
    }

    // Add authorization header if we have a token
    if !gateway_token.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", gateway_token));
    }

    // Add body
    if let Some(body) = request.body {
        req = req.body(body);
    }

    // Send request
    let response = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(HostApiFetchResponse {
                ok: false,
                data: None,
                error: Some(format!("Request failed: {}", e)),
            });
        }
    };

    let status = response.status().as_u16();
    let ok = response.status().is_success();

    tracing::debug!("Host API response: status={}, ok={}", status, ok);

    // Parse response
    let (json, text) = if status != 204 {
        let content_type = response.headers().get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        tracing::debug!("Response content-type: {}", content_type);

        if content_type.contains("application/json") {
            match response.json::<serde_json::Value>().await {
                Ok(j) => {
                    tracing::debug!("Parsed JSON response: {:?}", j);
                    (Some(j), None)
                }
                Err(e) => {
                    tracing::warn!("Failed to parse JSON: {}", e);
                    (None, None)
                }
            }
        } else {
            match response.text().await {
                Ok(t) => {
                    tracing::debug!("Got text response (len={})", t.len());
                    (None, Some(t))
                }
                Err(e) => {
                    tracing::warn!("Failed to read text: {}", e);
                    (None, None)
                }
            }
        }
    } else {
        (None, None)
    };

    Ok(HostApiFetchResponse {
        ok: true,
        data: Some(HostApiFetchData {
            status,
            ok,
            json,
            text,
        }),
        error: None,
    })
}