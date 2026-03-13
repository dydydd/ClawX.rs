//! Channel configuration utilities
//!
//! Manages channel configuration in OpenClaw config files.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

/// Default account ID
const DEFAULT_ACCOUNT_ID: &str = "default";

/// Plugin-based channels (config goes under plugins.entries, not channels)
const PLUGIN_CHANNELS: &[&str] = &["whatsapp"];

/// Channel configuration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfigData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Plugins configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<HashMap<String, ChannelConfigData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// OpenClaw configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenClawConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<HashMap<String, ChannelConfigData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<HashMap<String, serde_json::Value>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Get OpenClaw directory path
fn get_openclaw_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Failed to get home directory")
        .join(".openclaw")
}

/// Get OpenClaw config file path
fn get_config_path() -> PathBuf {
    get_openclaw_dir().join("openclaw.json")
}

/// Ensure OpenClaw directory exists
async fn ensure_config_dir() -> Result<()> {
    let dir = get_openclaw_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("Failed to create OpenClaw directory: {}", dir.display()))?;
    }
    Ok(())
}

/// Read OpenClaw configuration
pub async fn read_openclaw_config() -> Result<OpenClawConfig> {
    ensure_config_dir().await?;

    let config_path = get_config_path();
    if !config_path.exists() {
        return Ok(OpenClawConfig::default());
    }

    let content = fs::read_to_string(&config_path)
        .await
        .context("Failed to read OpenClaw config file")?;

    let config: OpenClawConfig = serde_json::from_str(&content)
        .context("Failed to parse OpenClaw config")?;

    Ok(config)
}

/// Write OpenClaw configuration
pub async fn write_openclaw_config(config: &mut OpenClawConfig) -> Result<()> {
    ensure_config_dir().await?;

    // Enable graceful in-process reload authorization for SIGUSR1 flows
    let commands = config.commands.get_or_insert_with(HashMap::new);
    commands.insert("restart".to_string(), serde_json::json!(true));

    let config_path = get_config_path();
    let content = serde_json::to_string_pretty(&config)
        .context("Failed to serialize OpenClaw config")?;

    fs::write(&config_path, content)
        .await
        .context("Failed to write OpenClaw config file")?;

    Ok(())
}

/// Ensure plugin allowlist is configured for specific channel types
fn ensure_plugin_allowlist(config: &mut OpenClawConfig, channel_type: &str) {
    match channel_type {
        "feishu" | "wecom" | "dingtalk" | "qqbot" | "whatsapp" => {
            if config.plugins.is_none() {
                config.plugins = Some(PluginsConfig::default());
            }

            if let Some(ref mut plugins) = config.plugins {
                plugins.enabled = Some(true);

                // Initialize allow list if not present
                if plugins.allow.is_none() {
                    plugins.allow = Some(Vec::new());
                }

                let allow = plugins.allow.as_mut().unwrap();

                // Add appropriate plugin to allowlist
                let plugin_id = match channel_type {
                    "feishu" => "feishu-openclaw-plugin",
                    "wecom" => "wecom-openclaw-plugin",
                    other => other,
                };

                if !allow.contains(&plugin_id.to_string()) {
                    allow.push(plugin_id.to_string());
                }

                // Initialize entries if not present
                if plugins.entries.is_none() {
                    plugins.entries = Some(HashMap::new());
                }

                // Add entry for the plugin
                if let Some(ref mut entries) = plugins.entries {
                    if !entries.contains_key(channel_type) {
                        entries.insert(
                            channel_type.to_string(),
                            ChannelConfigData {
                                enabled: Some(true),
                                extra: HashMap::new(),
                            },
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

/// List configured channels
pub async fn list_configured_channels() -> Result<Vec<String>> {
    let config = read_openclaw_config().await?;
    let mut channels = Vec::new();

    if let Some(ref channels_map) = config.channels {
        for (channel_type, section) in channels_map {
            if section.enabled == Some(false) {
                continue;
            }

            // Check if channel has accounts configured
            if let Some(accounts) = section.extra.get("accounts") {
                if let Some(accounts_obj) = accounts.as_object() {
                    if !accounts_obj.is_empty() {
                        channels.push(channel_type.clone());
                        continue;
                    }
                }
            }

            // Check if channel has other config
            if !section.extra.is_empty() {
                channels.push(channel_type.clone());
            }
        }
    }

    // Check for plugin-based channels
    if let Some(ref plugins) = config.plugins {
        if let Some(ref entries) = plugins.entries {
            for (plugin_name, config) in entries {
                if config.enabled != Some(false) {
                    if !channels.contains(plugin_name) {
                        channels.push(plugin_name.clone());
                    }
                }
            }
        }
    }

    Ok(channels)
}

/// Get channel configuration
pub async fn get_channel_config(
    channel_type: &str,
    account_id: Option<&str>,
) -> Result<Option<ChannelConfigData>> {
    let config = read_openclaw_config().await?;

    // Plugin-based channels
    if PLUGIN_CHANNELS.contains(&channel_type) {
        if let Some(ref plugins) = config.plugins {
            if let Some(ref entries) = plugins.entries {
                return Ok(entries.get(channel_type).cloned());
            }
        }
        return Ok(None);
    }

    // Regular channels
    if let Some(ref channels) = config.channels {
        if let Some(section) = channels.get(channel_type) {
            let resolved_account_id = account_id.unwrap_or(DEFAULT_ACCOUNT_ID);

            // Try to get account-specific config
            if let Some(accounts) = section.extra.get("accounts") {
                if let Some(accounts_obj) = accounts.as_object() {
                    if let Some(account_config) = accounts_obj.get(resolved_account_id) {
                        if let Ok(config) = serde_json::from_value::<ChannelConfigData>(account_config.clone()) {
                            return Ok(Some(config));
                        }
                    }
                }
            }

            // Return top-level config (legacy format)
            return Ok(Some(section.clone()));
        }
    }

    Ok(None)
}

/// Save channel configuration
pub async fn save_channel_config(
    channel_type: &str,
    config: ChannelConfigData,
    account_id: Option<&str>,
) -> Result<()> {
    let mut openclaw_config = read_openclaw_config().await?;
    let resolved_account_id = account_id.unwrap_or(DEFAULT_ACCOUNT_ID);

    ensure_plugin_allowlist(&mut openclaw_config, channel_type);

    // Plugin-based channels
    if PLUGIN_CHANNELS.contains(&channel_type) {
        if openclaw_config.plugins.is_none() {
            openclaw_config.plugins = Some(PluginsConfig::default());
        }

        if let Some(ref mut plugins) = openclaw_config.plugins {
            if plugins.entries.is_none() {
                plugins.entries = Some(HashMap::new());
            }

            if let Some(ref mut entries) = plugins.entries {
                entries.insert(
                    channel_type.to_string(),
                    ChannelConfigData {
                        enabled: config.enabled,
                        extra: config.extra,
                    },
                );
            }
        }

        write_openclaw_config(&mut openclaw_config).await?;
        tracing::info!("Saved plugin channel config for {}", channel_type);
        return Ok(());
    }

    // Regular channels
    if openclaw_config.channels.is_none() {
        openclaw_config.channels = Some(HashMap::new());
    }

    if let Some(ref mut channels) = openclaw_config.channels {
        if !channels.contains_key(channel_type) {
            channels.insert(
                channel_type.to_string(),
                ChannelConfigData {
                    enabled: None,
                    extra: HashMap::new(),
                },
            );
        }

        if let Some(section) = channels.get_mut(channel_type) {
            // Initialize accounts if not present
            if !section.extra.contains_key("accounts") {
                section.extra.insert(
                    "accounts".to_string(),
                    serde_json::json!({}),
                );
            }

            // Set default account
            if !section.extra.contains_key("defaultAccount") {
                section.extra.insert(
                    "defaultAccount".to_string(),
                    serde_json::json!(DEFAULT_ACCOUNT_ID),
                );
            }

            // Update account-specific config
            if let Some(accounts) = section.extra.get_mut("accounts") {
                if let Some(accounts_obj) = accounts.as_object_mut() {
                    accounts_obj.insert(
                        resolved_account_id.to_string(),
                        serde_json::to_value(&config)?,
                    );
                }
            }

            // For default account, also mirror to top level (for backward compat)
            if resolved_account_id == DEFAULT_ACCOUNT_ID {
                for (key, value) in &config.extra {
                    section.extra.insert(key.clone(), value.clone());
                }
                if let Some(enabled) = config.enabled {
                    section.enabled = Some(enabled);
                }
            }
        }
    }

    write_openclaw_config(&mut openclaw_config).await?;
    tracing::info!(
        "Saved channel config for {} (account: {})",
        channel_type,
        resolved_account_id
    );

    Ok(())
}

/// Delete channel configuration
pub async fn delete_channel_config(channel_type: &str) -> Result<()> {
    let mut config = read_openclaw_config().await?;

    // Try to delete from channels
    if let Some(ref mut channels) = config.channels {
        channels.remove(channel_type);
    }

    // Try to delete from plugins
    if let Some(ref mut plugins) = config.plugins {
        if let Some(ref mut entries) = plugins.entries {
            entries.remove(channel_type);
        }
    }

    write_openclaw_config(&mut config).await?;
    tracing::info!("Deleted channel config for {}", channel_type);

    Ok(())
}

/// Delete channel account configuration
pub async fn delete_channel_account_config(channel_type: &str, account_id: &str) -> Result<()> {
    let mut config = read_openclaw_config().await?;

    if let Some(ref mut channels) = config.channels {
        if let Some(section) = channels.get_mut(channel_type) {
            if let Some(accounts) = section.extra.get_mut("accounts") {
                if let Some(accounts_obj) = accounts.as_object_mut() {
                    accounts_obj.remove(account_id);

                    // Remove channel if no accounts left
                    if accounts_obj.is_empty() {
                        channels.remove(channel_type);
                    }
                }
            }
        }
    }

    write_openclaw_config(&mut config).await?;
    tracing::info!("Deleted channel account config for {}/{}", channel_type, account_id);

    Ok(())
}

/// Set channel enabled status
pub async fn set_channel_enabled(channel_type: &str, enabled: bool) -> Result<()> {
    let mut config = read_openclaw_config().await?;

    // Plugin-based channels
    if PLUGIN_CHANNELS.contains(&channel_type) {
        if config.plugins.is_none() {
            config.plugins = Some(PluginsConfig::default());
        }

        if let Some(ref mut plugins) = config.plugins {
            if plugins.entries.is_none() {
                plugins.entries = Some(HashMap::new());
            }

            if let Some(ref mut entries) = plugins.entries {
                if !entries.contains_key(channel_type) {
                    entries.insert(
                        channel_type.to_string(),
                        ChannelConfigData {
                            enabled: Some(enabled),
                            extra: HashMap::new(),
                        },
                    );
                } else {
                    entries.get_mut(channel_type).unwrap().enabled = Some(enabled);
                }
            }
        }

        write_openclaw_config(&mut config).await?;
        return Ok(());
    }

    // Regular channels
    if config.channels.is_none() {
        config.channels = Some(HashMap::new());
    }

    if let Some(ref mut channels) = config.channels {
        if !channels.contains_key(channel_type) {
            channels.insert(
                channel_type.to_string(),
                ChannelConfigData {
                    enabled: Some(enabled),
                    extra: HashMap::new(),
                },
            );
        } else {
            channels.get_mut(channel_type).unwrap().enabled = Some(enabled);
        }
    }

    write_openclaw_config(&mut config).await?;
    tracing::info!("Set channel {} enabled: {}", channel_type, enabled);

    Ok(())
}