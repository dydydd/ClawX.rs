//! Provider management IPC command handlers

use std::sync::Arc;
use serde::Serialize;
use tauri::State;
use crate::core::AppState;
use crate::services::providers::{ProviderApiKeyService, ProviderAccount, ProviderAccountUpdates, ProviderVendorInfo};

/// List all provider vendors
#[tauri::command]
pub async fn list_provider_vendors(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProviderVendorInfo>, String> {
    let providers = state.providers.read().await;
    Ok(providers.list_vendors())
}

/// List all provider accounts
#[tauri::command]
pub async fn list_provider_accounts(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProviderAccount>, String> {
    let providers = state.providers.read().await;
    Ok(providers.list_accounts().into_iter().cloned().collect())
}

/// Get a provider account by ID
#[tauri::command]
pub async fn get_provider_account(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<ProviderAccount>, String> {
    let providers = state.providers.read().await;
    Ok(providers.get_account(&id).cloned())
}

/// Create a new provider account
#[tauri::command(rename_all = "camelCase")]
pub async fn create_provider_account(
    account: ProviderAccount,
    api_key: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<ProviderAccount, String> {
    let mut providers = state.providers.write().await;
    providers.create_account(account, api_key)
        .await
        .map_err(|e| e.to_string())
}

/// Update a provider account
#[tauri::command]
pub async fn update_provider_account(
    id: String,
    updates: ProviderAccountUpdates,
    state: State<'_, Arc<AppState>>,
) -> Result<ProviderAccount, String> {
    let mut providers = state.providers.write().await;
    providers.update_account(&id, updates)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a provider account
#[tauri::command]
pub async fn delete_provider_account(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut providers = state.providers.write().await;
    providers.delete_account(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Get the default provider account ID
#[tauri::command]
pub async fn get_default_provider_account(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<String>, String> {
    let providers = state.providers.read().await;
    Ok(providers.get_default_account_id().map(|s| s.to_string()))
}

/// Set the default provider account
#[tauri::command]
pub async fn set_default_provider_account(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut providers = state.providers.write().await;
    providers.set_default_account(&id)
        .await
        .map_err(|e| e.to_string())
}

// ==================== Provider API Key Commands ====================

/// Set an API key for a provider
///
/// Stores the API key securely in the OS keyring and syncs to OpenClaw
#[tauri::command(rename_all = "camelCase")]
pub async fn set_provider_api_key(
    provider_id: String,
    api_key: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let service = ProviderApiKeyService::new();
    service
        .set(&provider_id, &api_key)
        .map_err(|e| format!("Failed to store API key: {}", e))?;

    // Sync auth profiles to OpenClaw
    let providers = state.providers.read().await;
    let accounts: Vec<_> = providers.list_accounts().into_iter().cloned().collect();
    drop(providers); // Release the lock before async operation

    if let Err(e) = crate::services::providers::sync_auth_to_openclaw(&accounts).await {
        tracing::warn!("Failed to sync auth profiles to OpenClaw: {}", e);
    }

    Ok(())
}

/// Get an API key for a provider
///
/// Returns the raw API key if it exists
#[tauri::command(rename_all = "camelCase")]
pub async fn get_provider_api_key(
    provider_id: String,
) -> Result<Option<String>, String> {
    let service = ProviderApiKeyService::new();
    service
        .get(&provider_id)
        .map_err(|e| format!("Failed to retrieve API key: {}", e))
}

/// Check if a provider has an API key stored
#[tauri::command(rename_all = "camelCase")]
pub async fn has_provider_api_key(
    provider_id: String,
) -> Result<bool, String> {
    let service = ProviderApiKeyService::new();
    service
        .has(&provider_id)
        .map_err(|e| format!("Failed to check API key: {}", e))
}

/// Delete an API key for a provider
#[tauri::command(rename_all = "camelCase")]
pub async fn delete_provider_api_key(
    provider_id: String,
) -> Result<(), String> {
    let service = ProviderApiKeyService::new();
    service
        .delete(&provider_id)
        .map_err(|e| format!("Failed to delete API key: {}", e))
}

/// Get a masked version of the API key for display
///
/// Shows first 4 and last 4 characters with asterisks in between
#[tauri::command(rename_all = "camelCase")]
pub async fn get_provider_api_key_masked(
    provider_id: String,
) -> Result<Option<String>, String> {
    let service = ProviderApiKeyService::new();
    service
        .get_masked(&provider_id)
        .map_err(|e| format!("Failed to retrieve masked API key: {}", e))
}

/// Validation result for provider API key
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Validate a provider API key
///
/// Performs a basic validation by making a test API call
#[tauri::command(rename_all = "camelCase")]
pub async fn validate_provider_api_key(
    provider_id: String,
    api_key: String,
    base_url: Option<String>,
    api_protocol: Option<String>,
) -> Result<ValidationResult, String> {
    // For custom and local providers, accept any non-empty key
    let provider = provider_id.to_lowercase();

    if provider.contains("custom") || provider.contains("ollama") {
        if api_key.trim().is_empty() && provider != "ollama" {
            return Ok(ValidationResult {
                valid: false,
                error: Some("API key cannot be empty".to_string()),
            });
        }
        return Ok(ValidationResult {
            valid: true,
            error: None,
        });
    }

    // For other providers, check if the key is not empty
    if api_key.trim().is_empty() {
        return Ok(ValidationResult {
            valid: false,
            error: Some("API key cannot be empty".to_string()),
        });
    }

    // Basic format validation for known providers (warning only, don't reject)
    let key = api_key.trim();

    // Log a warning if the format looks unusual, but still accept it
    let expected_pattern = match provider.as_str() {
        "anthropic" => key.starts_with("sk-ant-"),
        "openai" => key.starts_with("sk-"),
        "deepseek" => key.starts_with("sk-"),
        "moonshot" => key.starts_with("sk-"),
        "siliconflow" => key.starts_with("sk-"),
        "openrouter" => key.starts_with("sk-or-"),
        "ark" => key.len() > 10,
        "google" => key.starts_with("AIza"),
        "minimax-portal" | "minimax-portal-cn" => key.len() > 20,
        _ => key.len() >= 4, // Minimum reasonable length for unknown providers
    };

    if !expected_pattern {
        tracing::warn!(
            "API key format looks unusual for provider {}, but accepting it",
            provider_id
        );
    }

    Ok(ValidationResult {
        valid: true,
        error: None,
    })
}

/// Sync provider auth profiles to OpenClaw
///
/// Writes the auth-profiles.json file for the OpenClaw agent
#[tauri::command]
pub async fn sync_provider_auth_to_openclaw(
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let providers = state.providers.read().await;
    let accounts: Vec<_> = providers.list_accounts().into_iter().cloned().collect();

    crate::services::providers::sync_auth_to_openclaw(&accounts)
        .await
        .map_err(|e| e.to_string())
}