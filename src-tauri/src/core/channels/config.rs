//! Channel configuration management
//!
//! Manages channel configurations in a dedicated JSON file (~/.openclaw/channels.json)
//! and provides methods for CRUD operations on channels.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::RwLock;

/// Channel status variants
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelStatus {
    /// Channel is disconnected/not configured
    Disconnected,
    /// Channel is connecting/authenticating
    Connecting,
    /// Channel is connected and ready
    Connected,
    /// Channel encountered an error
    Error,
}

impl Default for ChannelStatus {
    fn default() -> Self {
        ChannelStatus::Disconnected
    }
}

/// Channel configuration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel-specific configuration fields
    #[serde(flatten)]
    pub fields: HashMap<String, serde_json::Value>,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }
}

/// Channel data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Unique channel identifier (e.g., "whatsapp-default", "feishu-main")
    pub id: String,
    /// Channel type (e.g., "feishu", "whatsapp", "slack", "discord")
    #[serde(rename = "type")]
    pub channel_type: String,
    /// Whether the channel is enabled
    pub enabled: bool,
    /// Channel-specific configuration
    pub config: ChannelConfig,
    /// Current channel status
    #[serde(default)]
    pub status: ChannelStatus,
    /// Status message (e.g., error description)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    /// Last updated timestamp
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: DateTime<Utc>,
    /// Creation timestamp
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
}

impl Channel {
    /// Create a new channel
    pub fn new(id: String, channel_type: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            channel_type,
            enabled: true,
            config: ChannelConfig::default(),
            status: ChannelStatus::Disconnected,
            status_message: None,
            updated_at: now,
            created_at: now,
        }
    }

    /// Set the channel status
    pub fn set_status(&mut self, status: ChannelStatus, message: Option<String>) {
        self.status = status;
        self.status_message = message;
        self.updated_at = Utc::now();
    }

    /// Update configuration
    pub fn set_config(&mut self, config: ChannelConfig) {
        self.config = config;
        self.updated_at = Utc::now();
    }

    /// Enable the channel
    pub fn enable(&mut self) {
        self.enabled = true;
        self.updated_at = Utc::now();
    }

    /// Disable the channel
    pub fn disable(&mut self) {
        self.enabled = false;
        self.updated_at = Utc::now();
    }
}

/// Channels data file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ChannelsData {
    /// Map of channel ID to channel data
    channels: HashMap<String, Channel>,
    /// Schema version for migration support
    version: u32,
}

impl ChannelsData {
    fn new() -> Self {
        Self {
            channels: HashMap::new(),
            version: 1,
        }
    }
}

/// Channel manager for managing channel configurations
pub struct ChannelManager {
    /// Path to the channels.json file
    config_path: PathBuf,
    /// In-memory cache of channels data
    data: RwLock<ChannelsData>,
}

impl ChannelManager {
    /// Create a new channel manager
    pub async fn new() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        let data = if config_path.exists() {
            Self::load_config(&config_path).await?
        } else {
            ChannelsData::new()
        };

        Ok(Self {
            config_path,
            data: RwLock::new(data),
        })
    }

    /// Get the path to the channels configuration file
    fn get_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        let config_dir = home.join(".openclaw");
        Ok(config_dir.join("channels.json"))
    }

    /// Load configuration from disk
    async fn load_config(path: &PathBuf) -> Result<ChannelsData> {
        let content = fs::read_to_string(path)
            .await
            .context("Failed to read channels config file")?;

        let data: ChannelsData = serde_json::from_str(&content)
            .context("Failed to parse channels config")?;

        Ok(data)
    }

    /// Save configuration to disk
    async fn save_config(&self, data: &ChannelsData) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create config directory")?;
        }

        let content = serde_json::to_string_pretty(data)
            .context("Failed to serialize channels config")?;

        fs::write(&self.config_path, content)
            .await
            .context("Failed to write channels config file")?;

        tracing::info!("Saved channels config to {}", self.config_path.display());
        Ok(())
    }

    /// List all channels
    pub async fn list_channels(&self) -> Vec<Channel> {
        let data = self.data.read().await;
        data.channels.values().cloned().collect()
    }

    /// List channels filtered by type
    pub async fn list_channels_by_type(&self, channel_type: &str) -> Vec<Channel> {
        let data = self.data.read().await;
        data.channels
            .values()
            .filter(|c| c.channel_type == channel_type)
            .cloned()
            .collect()
    }

    /// Get a specific channel by ID
    pub async fn get_channel(&self, id: &str) -> Option<Channel> {
        let data = self.data.read().await;
        data.channels.get(id).cloned()
    }

    /// Get a channel by type and account (generates ID)
    pub async fn get_channel_by_account(&self, channel_type: &str, account_id: &str) -> Option<Channel> {
        let id = format!("{}-{}", channel_type, account_id);
        self.get_channel(&id).await
    }

    /// Save or update a channel
    pub async fn save_channel(&self, channel: Channel) -> Result<()> {
        let mut data = self.data.write().await;
        data.channels.insert(channel.id.clone(), channel.clone());
        self.save_config(&data).await?;
        tracing::info!("Saved channel: {}", channel.id);
        Ok(())
    }

    /// Delete a channel by ID
    pub async fn delete_channel(&self, id: &str) -> Result<()> {
        let mut data = self.data.write().await;
        if data.channels.remove(id).is_some() {
            self.save_config(&data).await?;
            tracing::info!("Deleted channel: {}", id);
        }
        Ok(())
    }

    /// Set channel enabled status
    pub async fn set_channel_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let mut data = self.data.write().await;

        if let Some(channel) = data.channels.get_mut(id) {
            channel.enabled = enabled;
            channel.updated_at = Utc::now();
            self.save_config(&data).await?;
            tracing::info!("Set channel {} enabled: {}", id, enabled);
            Ok(())
        } else {
            anyhow::bail!("Channel not found: {}", id)
        }
    }

    /// Update channel status
    pub async fn update_channel_status(
        &self,
        id: &str,
        status: ChannelStatus,
        message: Option<String>,
    ) -> Result<()> {
        let mut data = self.data.write().await;

        if let Some(channel) = data.channels.get_mut(id) {
            channel.status = status;
            channel.status_message = message;
            channel.updated_at = Utc::now();
            self.save_config(&data).await?;
            tracing::info!("Updated channel {} status to {:?}", id, status);
            Ok(())
        } else {
            anyhow::bail!("Channel not found: {}", id)
        }
    }

    /// Check if a channel exists
    pub async fn channel_exists(&self, id: &str) -> bool {
        let data = self.data.read().await;
        data.channels.contains_key(id)
    }

    /// Get enabled channels
    pub async fn list_enabled_channels(&self) -> Vec<Channel> {
        let data = self.data.read().await;
        data.channels
            .values()
            .filter(|c| c.enabled)
            .cloned()
            .collect()
    }

    /// Create or get a channel
    pub async fn get_or_create_channel(
        &self,
        id: String,
        channel_type: String,
    ) -> Result<Channel> {
        if let Some(channel) = self.get_channel(&id).await {
            return Ok(channel);
        }

        let channel = Channel::new(id, channel_type);
        self.save_channel(channel.clone()).await?;
        Ok(channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_channel_crud() {
        // This test would need to be adapted to use a temporary config path
        // For now, we just verify the types compile correctly
        let channel = Channel::new("test-1".to_string(), "whatsapp".to_string());
        assert_eq!(channel.id, "test-1");
        assert_eq!(channel.channel_type, "whatsapp");
        assert!(channel.enabled);
    }
}
