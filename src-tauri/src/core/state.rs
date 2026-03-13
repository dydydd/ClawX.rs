//! Application state management
//!
//! This module provides centralized state management for the Tauri application.
//! All stores and managers are accessed through `AppState`.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::auth::{DeviceOAuthManager, BrowserOAuthManager};
use crate::core::channels::{ChannelManager, WhatsAppManager};
use crate::core::gateway::GatewayManager;
use crate::core::logging::Logger;
use crate::core::storage::{get_settings_path, SecretStore, SettingsStore};
use crate::services::providers::ProviderService;

/// Get the provider accounts data file path
fn get_provider_accounts_path() -> std::path::PathBuf {
    let data_dir = dirs::data_local_dir().expect("Failed to get data directory");
    data_dir.join("ClawX").join("provider_accounts.json")
}

/// Central application state shared across all commands
pub struct AppState {
    /// Settings storage (JSON file)
    pub settings: Arc<RwLock<SettingsStore>>,
    /// Provider service (manages provider accounts)
    pub providers: Arc<RwLock<ProviderService>>,
    /// Secret storage (OS keychain)
    pub secrets: Arc<SecretStore>,
    /// Gateway manager
    pub gateway: Arc<GatewayManager>,
    /// Logger
    pub logger: Arc<Logger>,
    /// Channel manager
    pub channels: Arc<ChannelManager>,
    /// WhatsApp manager
    pub whatsapp: Arc<WhatsAppManager>,
    /// Device OAuth manager
    pub device_oauth: Arc<DeviceOAuthManager>,
    /// Browser OAuth manager
    pub browser_oauth: Arc<BrowserOAuthManager>,
}

impl AppState {
    /// Create and initialize the application state
    pub async fn new() -> anyhow::Result<Self> {
        // Initialize logger
        let logger = crate::core::logging::init_logger()?;
        tracing::info!("Logger initialized");

        let settings = SettingsStore::new(get_settings_path()).await?;
        tracing::info!("Settings store initialized");

        let providers = ProviderService::new(get_provider_accounts_path()).await?;
        tracing::info!("Provider service initialized");

        let secrets = SecretStore::new("ClawX");
        tracing::info!("Secret store initialized");

        let gateway = GatewayManager::new();
        tracing::info!("Gateway manager initialized");

        let channels = Arc::new(ChannelManager::new().await?);
        tracing::info!("Channel manager initialized");

        let whatsapp = WhatsAppManager::new(Arc::clone(&channels)).await?;
        tracing::info!("WhatsApp manager initialized");

        let device_oauth = Arc::new(DeviceOAuthManager::new());
        let browser_oauth = Arc::new(BrowserOAuthManager::new());
        tracing::info!("OAuth managers initialized");

        Ok(Self {
            settings: Arc::new(RwLock::new(settings)),
            providers: Arc::new(RwLock::new(providers)),
            secrets: Arc::new(secrets),
            gateway: Arc::new(gateway),
            logger,
            channels,
            whatsapp: Arc::new(whatsapp),
            device_oauth,
            browser_oauth,
        })
    }
}