//! Settings IPC command handlers

use std::sync::Arc;
use serde_json::Value;
use std::collections::HashMap;
use tauri::{State, AppHandle, Emitter};
use crate::core::AppState;
use crate::services::tray::update_tray_language;

/// Settings changed event name
pub const SETTINGS_CHANGED_EVENT: &str = "settings:changed";

/// Get a specific setting value
#[tauri::command]
pub async fn get_setting(
    key: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<Value>, String> {
    let settings = state.settings.read().await;
    Ok(settings.get(&key))
}

/// Set a setting value
#[tauri::command]
pub async fn set_setting(
    key: String,
    value: Value,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<(), String> {
    let mut settings = state.settings.write().await;
    settings.set(key.clone(), value.clone());
    settings.persist().await.map_err(|e| e.to_string())?;

    // If language changed, update tray menu
    if key == "language" {
        if let Some(lang) = value.as_str() {
            if let Err(e) = update_tray_language(&app, lang).await {
                tracing::warn!("Failed to update tray language: {}", e);
            }
        }
    }

    // Emit event to notify frontend of settings change
    app.emit(SETTINGS_CHANGED_EVENT, serde_json::json!({
        "key": key,
        "value": value,
    })).map_err(|e| e.to_string())?;

    Ok(())
}

/// Set multiple settings at once (batch update)
#[tauri::command]
pub async fn set_many_settings(
    patch: HashMap<String, Value>,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<(), String> {
    let mut settings = state.settings.write().await;

    for (key, value) in &patch {
        settings.set(key.clone(), value.clone());
    }

    settings.persist().await.map_err(|e| e.to_string())?;

    // If language changed in batch, update tray menu
    if let Some(lang_value) = patch.get("language") {
        if let Some(lang) = lang_value.as_str() {
            if let Err(e) = update_tray_language(&app, lang).await {
                tracing::warn!("Failed to update tray language: {}", e);
            }
        }
    }

    // Emit event with all changed keys
    app.emit(SETTINGS_CHANGED_EVENT, serde_json::json!({
        "keys": patch.keys().collect::<Vec<_>>(),
        "batch": true,
    })).map_err(|e| e.to_string())?;

    Ok(())
}

/// Get all settings
#[tauri::command]
pub async fn get_all_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<HashMap<String, Value>, String> {
    let settings = state.settings.read().await;
    Ok(settings.get_all())
}

/// Reset all settings to defaults
#[tauri::command]
pub async fn reset_settings(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<HashMap<String, Value>, String> {
    let mut settings = state.settings.write().await;
    settings.reset().await.map_err(|e| e.to_string())?;

    // Get the reset settings
    let all_settings = settings.get_all();

    // Emit reset event
    app.emit(SETTINGS_CHANGED_EVENT, serde_json::json!({
        "reset": true,
        "settings": all_settings,
    })).map_err(|e| e.to_string())?;

    Ok(all_settings)
}

/// Export settings to JSON string
#[tauri::command]
pub async fn export_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let settings = state.settings.read().await;
    settings.export().map_err(|e| e.to_string())
}

/// Import settings from JSON string
#[tauri::command]
pub async fn import_settings(
    json: String,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<(), String> {
    let mut settings = state.settings.write().await;
    settings.import(&json).await.map_err(|e| e.to_string())?;

    // Update tray language if it changed
    let language = settings.get("language")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "en".to_string());
    if let Err(e) = update_tray_language(&app, &language).await {
        tracing::warn!("Failed to update tray language: {}", e);
    }

    // Emit event with imported settings
    let all_settings = settings.get_all();
    app.emit(SETTINGS_CHANGED_EVENT, serde_json::json!({
        "imported": true,
        "settings": all_settings,
    })).map_err(|e| e.to_string())?;

    Ok(())
}