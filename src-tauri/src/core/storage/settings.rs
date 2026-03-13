//! Settings storage using file-based JSON
//!
//! This module provides persistent key-value storage for application settings,
//! backed by a JSON file. It matches the functionality of the Electron store.ts
//! implementation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Get the OpenClaw config directory path
fn get_openclaw_config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Failed to get home directory")
        .join(".openclaw")
}

/// Get the OpenClaw config file path
fn get_openclaw_config_path() -> PathBuf {
    get_openclaw_config_dir().join("openclaw.json")
}

/// Read gateway token from OpenClaw config file
fn read_gateway_token_from_openclaw_config() -> Option<String> {
    let config_path = get_openclaw_config_path();
    if !config_path.exists() {
        return None;
    }

    std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| {
            serde_json::from_str::<Value>(&content).ok()
        })
        .and_then(|json| {
            // Navigate to gateway.auth.token
            json.get("gateway")?
                .get("auth")?
                .get("token")?
                .as_str()
                .map(|s| s.to_string())
        })
}

/// Application settings with strongly-typed defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // General
    pub theme: String,
    pub language: String,
    pub start_minimized: bool,
    pub launch_at_startup: bool,
    pub telemetry_enabled: bool,
    pub machine_id: String,
    pub has_reported_install: bool,

    // Gateway
    pub gateway_auto_start: bool,
    pub gateway_port: u16,
    pub gateway_token: String,
    pub proxy_enabled: bool,
    pub proxy_server: String,
    pub proxy_http_server: String,
    pub proxy_https_server: String,
    pub proxy_all_server: String,
    pub proxy_bypass_rules: String,

    // Update
    pub update_channel: String,
    pub auto_check_update: bool,
    pub auto_download_update: bool,
    pub skipped_versions: Vec<String>,

    // UI State
    pub sidebar_collapsed: bool,
    pub dev_mode_unlocked: bool,

    // Presets
    pub selected_bundles: Vec<String>,
    pub enabled_skills: Vec<String>,
    pub disabled_skills: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // General
            theme: "system".to_string(),
            language: "en".to_string(),
            start_minimized: false,
            launch_at_startup: false,
            telemetry_enabled: true,
            machine_id: String::new(),
            has_reported_install: false,

            // Gateway
            gateway_auto_start: true,
            gateway_port: 18789,
            gateway_token: generate_token(),
            proxy_enabled: false,
            proxy_server: String::new(),
            proxy_http_server: String::new(),
            proxy_https_server: String::new(),
            proxy_all_server: String::new(),
            proxy_bypass_rules: "<local>;localhost;127.0.0.1;::1".to_string(),

            // Update
            update_channel: "stable".to_string(),
            auto_check_update: true,
            auto_download_update: false,
            skipped_versions: Vec::new(),

            // UI State
            sidebar_collapsed: false,
            dev_mode_unlocked: false,

            // Presets
            selected_bundles: vec!["productivity".to_string(), "developer".to_string()],
            enabled_skills: Vec::new(),
            disabled_skills: Vec::new(),
        }
    }
}

/// Generate a random token for gateway authentication
fn generate_token() -> String {
    use rand::distributions::Alphanumeric;
    use rand::Rng;

    let mut rng = rand::thread_rng();
    format!(
        "clawx-{}",
        std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(32)
            .collect::<String>()
    )
}

/// Settings store backed by a JSON file
///
/// This store provides persistent key-value storage with:
/// - Lazy loading from disk
/// - Automatic save on modification
/// - Default value support
/// - Type-safe access via serde_json::Value
#[derive(Debug)]
pub struct SettingsStore {
    /// Path to the settings file
    path: PathBuf,
    /// In-memory store of settings
    data: HashMap<String, Value>,
    /// Default values for settings
    defaults: HashMap<String, Value>,
}

impl SettingsStore {
    /// Create a new settings store with the given path
    ///
    /// If the file doesn't exist, it will be created with default values.
    /// If the file exists but is missing some keys, defaults will be applied.
    pub async fn new(path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Initialize with defaults
        let defaults = Self::create_defaults();

        // Load existing data or start empty
        let mut data = if path.exists() {
            let content = fs::read_to_string(&path).await?;
            serde_json::from_str(&content)?
        } else {
            HashMap::new()
        };

        // Apply defaults for missing keys
        let mut needs_save = false;
        for (key, value) in &defaults {
            if !data.contains_key(key) {
                data.insert(key.clone(), value.clone());
                needs_save = true;
            }
        }

        // Special handling for gatewayToken
        // Priority: 1. Existing in settings, 2. From OpenClaw config, 3. Generate new
        let needs_token_save = if !data.contains_key("gatewayToken") {
            // Try to read from OpenClaw config first
            let token = read_gateway_token_from_openclaw_config()
                .unwrap_or_else(|| {
                    tracing::info!("Generating new gatewayToken (not found in OpenClaw config)");
                    generate_token()
                });
            tracing::info!("Using gatewayToken: {}...", &token[..8.min(token.len())]);
            data.insert("gatewayToken".to_string(), Value::String(token));
            true
        } else {
            false
        };

        needs_save = needs_save || needs_token_save;

        let store = Self { path, data, defaults };

        // Save if we set new defaults or token
        if needs_save {
            tracing::info!("Saving initial settings with defaults");
            store.persist().await?;
        }

        Ok(store)
    }

    /// Create default settings as a HashMap
    fn create_defaults() -> HashMap<String, Value> {
        let settings = AppSettings::default();
        let mut map = HashMap::new();

        // General settings (camelCase to match TypeScript convention)
        map.insert("theme".to_string(), Value::String(settings.theme));
        map.insert("language".to_string(), Value::String(settings.language));
        map.insert("startMinimized".to_string(), Value::Bool(settings.start_minimized));
        map.insert("launchAtStartup".to_string(), Value::Bool(settings.launch_at_startup));
        map.insert("telemetryEnabled".to_string(), Value::Bool(settings.telemetry_enabled));
        map.insert("machineId".to_string(), Value::String(settings.machine_id));
        map.insert("hasReportedInstall".to_string(), Value::Bool(settings.has_reported_install));

        // Gateway settings
        map.insert("gatewayAutoStart".to_string(), Value::Bool(settings.gateway_auto_start));
        map.insert("gatewayPort".to_string(), Value::Number(settings.gateway_port.into()));
        map.insert("gatewayToken".to_string(), Value::String(settings.gateway_token));
        map.insert("proxyEnabled".to_string(), Value::Bool(settings.proxy_enabled));
        map.insert("proxyServer".to_string(), Value::String(settings.proxy_server));
        map.insert("proxyHttpServer".to_string(), Value::String(settings.proxy_http_server));
        map.insert("proxyHttpsServer".to_string(), Value::String(settings.proxy_https_server));
        map.insert("proxyAllServer".to_string(), Value::String(settings.proxy_all_server));
        map.insert("proxyBypassRules".to_string(), Value::String(settings.proxy_bypass_rules));

        // Update settings
        map.insert("updateChannel".to_string(), Value::String(settings.update_channel));
        map.insert("autoCheckUpdate".to_string(), Value::Bool(settings.auto_check_update));
        map.insert("autoDownloadUpdate".to_string(), Value::Bool(settings.auto_download_update));
        map.insert(
            "skippedVersions".to_string(),
            Value::Array(settings.skipped_versions.into_iter().map(Value::String).collect()),
        );

        // UI State settings
        map.insert("sidebarCollapsed".to_string(), Value::Bool(settings.sidebar_collapsed));
        map.insert("devModeUnlocked".to_string(), Value::Bool(settings.dev_mode_unlocked));

        // Presets settings
        map.insert(
            "selectedBundles".to_string(),
            Value::Array(settings.selected_bundles.into_iter().map(Value::String).collect()),
        );
        map.insert(
            "enabledSkills".to_string(),
            Value::Array(settings.enabled_skills.into_iter().map(Value::String).collect()),
        );
        map.insert(
            "disabledSkills".to_string(),
            Value::Array(settings.disabled_skills.into_iter().map(Value::String).collect()),
        );

        map
    }

    /// Get a setting value by key
    ///
    /// Returns `None` if the key doesn't exist and has no default.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.data.get(key).cloned()
    }

    /// Get a setting value by key, returning the default if not set
    pub fn get_or_default(&self, key: &str) -> Value {
        self.data
            .get(key)
            .cloned()
            .or_else(|| self.defaults.get(key).cloned())
            .unwrap_or(Value::Null)
    }

    /// Set a setting value
    ///
    /// The value is stored in memory immediately but not persisted to disk
    /// until `persist()` is called.
    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        self.data.insert(key.into(), value);
    }

    /// Get all settings as a HashMap
    ///
    /// Returns a clone of all current settings including defaults.
    pub fn get_all(&self) -> HashMap<String, Value> {
        let mut result = self.defaults.clone();
        result.extend(self.data.clone());
        result
    }

    /// Reset all settings to their default values
    ///
    /// This clears all custom settings and resets to defaults.
    /// The changes are persisted to disk immediately.
    pub async fn reset(&mut self) -> Result<()> {
        self.data = self.defaults.clone();
        // Generate a new gateway token on reset
        let token = generate_token();
        self.data.insert("gatewayToken".to_string(), Value::String(token));
        self.persist().await?;
        Ok(())
    }

    /// Save settings to disk (alias for persist)
    ///
    /// This is kept for backwards compatibility.
    pub async fn save(&self) -> Result<()> {
        self.persist().await
    }

    /// Persist settings to disk
    ///
    /// Writes the current in-memory settings to the JSON file.
    pub async fn persist(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.data)
            .context("Failed to serialize settings")?;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Write atomically using a temp file
        let temp_path = self.path.with_extension("json.tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(content.as_bytes()).await?;
        file.sync_all().await?;

        // Rename temp file to actual file (atomic on most filesystems)
        fs::rename(&temp_path, &self.path).await?;

        tracing::debug!("Settings persisted to {:?}", self.path);
        Ok(())
    }

    /// Import settings from a JSON string
    ///
    /// Replaces all current settings with the imported values.
    pub async fn import(&mut self, json: &str) -> Result<()> {
        let imported: HashMap<String, Value> = serde_json::from_str(json)
            .context("Invalid settings JSON")?;
        self.data = imported;
        self.persist().await?;
        Ok(())
    }

    /// Export settings to a JSON string
    pub fn export(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.get_all())
            .context("Failed to export settings")
    }

    /// Check if a setting exists
    pub fn has(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Remove a setting
    ///
    /// Removes the setting from the custom data. If it's a setting with a default,
    /// the default value will be returned on next `get_all()` call.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }

    /// Get the file path for this store
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_settings_store_basic() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_settings.json");

        let mut store = SettingsStore::new(path).await.unwrap();

        // Test get/set
        store.set("test_key", Value::String("test_value".to_string()));
        assert_eq!(
            store.get("test_key"),
            Some(Value::String("test_value".to_string()))
        );

        // Test persist and reload
        store.persist().await.unwrap();

        let store2 = SettingsStore::new(store.path().clone()).await.unwrap();
        assert_eq!(
            store2.get("test_key"),
            Some(Value::String("test_value".to_string()))
        );
    }

    #[tokio::test]
    async fn test_settings_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_settings.json");

        let store = SettingsStore::new(path).await.unwrap();

        // Check default values
        assert_eq!(store.get("theme"), Some(Value::String("system".to_string())));
        assert_eq!(store.get("language"), Some(Value::String("en".to_string())));
        assert!(store.get("gatewayToken").is_some());
    }

    #[tokio::test]
    async fn test_settings_reset() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_settings.json");

        let mut store = SettingsStore::new(path).await.unwrap();

        // Change a default value
        store.set("theme", Value::String("dark".to_string()));
        store.persist().await.unwrap();

        // Reset
        store.reset().await.unwrap();

        // Check that default is restored
        assert_eq!(store.get("theme"), Some(Value::String("system".to_string())));
    }
}
