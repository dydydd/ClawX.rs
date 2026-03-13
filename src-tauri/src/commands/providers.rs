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
#[tauri::command]
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
/// Stores the API key securely in the OS keyring
#[tauri::command]
pub async fn set_provider_api_key(
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let service = ProviderApiKeyService::new();
    service
        .set(&provider_id, &api_key)
        .map_err(|e| format!("Failed to store API key: {}", e))
}

/// Get an API key for a provider
///
/// Returns the raw API key if it exists
#[tauri::command]
pub async fn get_provider_api_key(
    provider_id: String,
) -> Result<Option<String>, String> {
    let service = ProviderApiKeyService::new();
    service
        .get(&provider_id)
        .map_err(|e| format!("Failed to retrieve API key: {}", e))
}

/// Check if a provider has an API key stored
#[tauri::command]
pub async fn has_provider_api_key(
    provider_id: String,
) -> Result<bool, String> {
    let service = ProviderApiKeyService::new();
    service
        .has(&provider_id)
        .map_err(|e| format!("Failed to check API key: {}", e))
}

/// Delete an API key for a provider
#[tauri::command]
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
#[tauri::command]
pub async fn get_provider_api_key_masked(
    provider_id: String,
) -> Result<Option<String>, String> {
    let service = ProviderApiKeyService::new();
    service
        .get_masked(&provider_id)
        .map_err(|e| format!("Failed to retrieve masked API key: {}", e))
}