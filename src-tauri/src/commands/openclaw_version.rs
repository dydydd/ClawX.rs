//! OpenClaw version check commands

use serde::{Deserialize, Serialize};

/// OpenClaw version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawVersionInfo {
    /// Current installed version
    pub current_version: String,
    /// Latest available version (if checked)
    pub latest_version: Option<String>,
    /// Whether an update is available
    pub update_available: bool,
}

/// Get OpenClaw version (compiled from package.json at build time)
#[tauri::command]
pub async fn get_openclaw_version() -> Result<OpenClawVersionInfo, String> {
    // Version is embedded at compile time from package.json
    let current_version = env!("OPENCLAW_VERSION").to_string();

    Ok(OpenClawVersionInfo {
        current_version,
        latest_version: None,
        update_available: false,
    })
}

/// Check for OpenClaw updates
#[tauri::command]
pub async fn check_openclaw_updates() -> Result<OpenClawVersionInfo, String> {
    // Get current version (embedded at compile time)
    let current_version = env!("OPENCLAW_VERSION").to_string();

    // Check latest version from npm
    let latest_version = check_latest_npm_version().await?;

    let update_available = current_version != latest_version && latest_version != "unknown";

    Ok(OpenClawVersionInfo {
        current_version,
        latest_version: Some(latest_version.clone()),
        update_available,
    })
}

/// Fetch latest version from npm registry
async fn check_latest_npm_version() -> Result<String, String> {
    use reqwest::Client;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get("https://registry.npmjs.org/openclaw/latest")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch from npm: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("npm registry returned status: {}", response.status()));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse npm response: {}", e))?;

    Ok(json["version"]
        .as_str()
        .unwrap_or("unknown")
        .to_string())
}