//! OAuth IPC command handlers

use std::sync::Arc;
use tauri::State;
use crate::core::auth::{DeviceOAuthManager, BrowserOAuthManager, OAuthStartOptions, OAuthProvider, OAuthRegion};

/// Start OAuth flow
#[tauri::command]
pub async fn oauth_start(
    provider: String,
    region: Option<String>,
    account_id: Option<String>,
    label: Option<String>,
    device_oauth: State<'_, Arc<DeviceOAuthManager>>,
    browser_oauth: State<'_, Arc<BrowserOAuthManager>>,
) -> Result<(), String> {
    let provider_enum = match provider.as_str() {
        "anthropic" => OAuthProvider::Anthropic,
        "google" => OAuthProvider::Google,
        "openai" => OAuthProvider::OpenAI,
        _ => return Err(format!("Unknown OAuth provider: {}", provider)),
    };

    let region_enum = region.and_then(|r| match r.as_str() {
        "global" => Some(OAuthRegion::Global),
        "cn" => Some(OAuthRegion::Cn),
        _ => None,
    });

    let is_device_flow = matches!(provider_enum, OAuthProvider::Anthropic);

    let options = OAuthStartOptions {
        provider: provider_enum.clone(),
        region: region_enum.clone(),
        account_id,
        label,
    };

    if is_device_flow {
        device_oauth
            .start_flow(provider_enum, region_enum, options)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        browser_oauth
            .start_flow(provider_enum, options)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Cancel OAuth flow
#[tauri::command]
pub async fn oauth_cancel(
    device_oauth: State<'_, Arc<DeviceOAuthManager>>,
    browser_oauth: State<'_, Arc<BrowserOAuthManager>>,
) -> Result<(), String> {
    device_oauth.stop_flow().await;
    browser_oauth.stop_flow().await;
    Ok(())
}

/// Submit OAuth code manually
#[tauri::command]
pub async fn oauth_submit_code(
    code: String,
    browser_oauth: State<'_, Arc<BrowserOAuthManager>>,
) -> Result<bool, String> {
    browser_oauth
        .submit_manual_code(code)
        .await
        .map_err(|e| e.to_string())
}

/// Get OAuth status
#[tauri::command]
pub async fn oauth_get_status(
    device_oauth: State<'_, Arc<DeviceOAuthManager>>,
    browser_oauth: State<'_, Arc<BrowserOAuthManager>>,
) -> Result<crate::core::auth::OAuthStatus, String> {
    let device_status = device_oauth.get_status().await;
    let browser_status = browser_oauth.get_status().await;

    // Return whichever status is not Idle
    match (&device_status, &browser_status) {
        (crate::core::auth::OAuthStatus::Idle, _) => Ok(browser_status),
        _ => Ok(device_status),
    }
}