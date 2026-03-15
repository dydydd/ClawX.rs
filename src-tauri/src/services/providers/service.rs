//! Provider service - manages provider accounts
//!
//! This module mirrors the Electron provider service from:
//! electron/services/providers/provider-service.ts

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

use crate::core::providers::{
    ProviderAuthMode, ProviderBackendConfig, ProviderDefinition,
    ProviderProtocol, ProviderTypeInfo, ProviderVendorCategory, get_provider_definition,
    PROVIDER_VENDORS,
};

/// Provider account metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccountMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_models: Option<Vec<String>>,
}

/// Provider account - user-created account for a provider vendor
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccount {
    pub id: String,
    pub vendor_id: String,
    pub label: String,
    pub auth_mode: ProviderAuthMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_protocol: Option<ProviderProtocol>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_account_ids: Option<Vec<String>>,
    pub enabled: bool,
    pub is_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ProviderAccountMetadata>,
    pub created_at: String,
    pub updated_at: String,
}

/// Provider account updates (partial update structure)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccountUpdates {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_mode: Option<ProviderAuthMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_protocol: Option<Option<ProviderProtocol>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_models: Option<Option<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_account_ids: Option<Option<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Option<ProviderAccountMetadata>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Provider vendor info for frontend (matches ProviderTypeInfo)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderVendorInfo {
    #[serde(flatten)]
    pub type_info: ProviderTypeInfo,
    pub category: ProviderVendorCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_config: Option<ProviderBackendConfig>,
    pub supported_auth_modes: Vec<ProviderAuthMode>,
    pub default_auth_mode: ProviderAuthMode,
    pub supports_multiple_accounts: bool,
}

impl From<&ProviderDefinition> for ProviderVendorInfo {
    fn from(def: &ProviderDefinition) -> Self {
        Self {
            type_info: def.type_info.clone(),
            category: def.category.clone(),
            env_var: def.env_var.clone(),
            provider_config: def.provider_config.clone(),
            supported_auth_modes: def.supported_auth_modes.clone(),
            default_auth_mode: def.default_auth_mode.clone(),
            supports_multiple_accounts: def.supports_multiple_accounts,
        }
    }
}

/// Stored provider account with file format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccount {
    #[serde(flatten)]
    account: ProviderAccount,
}

/// Provider accounts storage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderAccountsData {
    version: u32,
    #[serde(rename = "defaultAccountId")]
    default_account_id: Option<String>,
    accounts: Vec<StoredAccount>,
}

impl Default for ProviderAccountsData {
    fn default() -> Self {
        Self {
            version: 1,
            default_account_id: None,
            accounts: Vec::new(),
        }
    }
}

/// Provider service manages provider accounts
pub struct ProviderService {
    data_path: PathBuf,
    data: ProviderAccountsData,
    accounts: HashMap<String, ProviderAccount>,
}

impl ProviderService {
    /// Create a new provider service
    pub async fn new(data_path: PathBuf) -> Result<Self> {
        let mut service = Self {
            data_path,
            data: ProviderAccountsData::default(),
            accounts: HashMap::new(),
        };

        service.load().await?;

        Ok(service)
    }

    /// Load provider accounts from disk
    async fn load(&mut self) -> Result<()> {
        if self.data_path.exists() {
            info!("Loading provider accounts from {:?}", self.data_path);
            let content = fs::read_to_string(&self.data_path).await?;
            self.data = serde_json::from_str(&content)?;

            // Populate the HashMap for fast lookups
            for stored in &self.data.accounts {
                self.accounts.insert(stored.account.id.clone(), stored.account.clone());
            }

            info!("Loaded {} provider accounts", self.accounts.len());
        } else {
            info!("No provider accounts file found, starting fresh");
        }

        Ok(())
    }

    /// Save provider accounts to disk
    async fn persist(&self) -> Result<()> {
        // Update the data structure from the HashMap
        let data = ProviderAccountsData {
            version: 1,
            default_account_id: self.data.default_account_id.clone(),
            accounts: self.accounts.values().map(|a| StoredAccount { account: a.clone() }).collect(),
        };

        // Ensure parent directory exists
        if let Some(parent) = self.data_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Write to file
        let content = serde_json::to_string_pretty(&data)?;
        let mut file = fs::File::create(&self.data_path).await?;
        file.write_all(content.as_bytes()).await?;

        debug!("Saved {} provider accounts to disk", self.accounts.len());
        Ok(())
    }

    /// List all provider vendors
    pub fn list_vendors(&self) -> Vec<ProviderVendorInfo> {
        PROVIDER_VENDORS.iter().map(ProviderVendorInfo::from).collect()
    }

    /// List all provider accounts
    pub fn list_accounts(&self) -> Vec<&ProviderAccount> {
        self.accounts.values().collect()
    }

    /// Get a provider account by ID
    pub fn get_account(&self, account_id: &str) -> Option<&ProviderAccount> {
        self.accounts.get(account_id)
    }

    /// Get the default provider account ID
    pub fn get_default_account_id(&self) -> Option<&str> {
        self.data.default_account_id.as_deref()
    }

    /// Create a new provider account
    pub async fn create_account(
        &mut self,
        mut account: ProviderAccount,
        api_key: Option<String>,
    ) -> Result<ProviderAccount> {
        // Validate the vendor exists
        let vendor = get_provider_definition(&account.vendor_id)
            .with_context(|| format!("Invalid vendor_id: {}", account.vendor_id))?;

        // Validate auth mode is supported
        if !vendor.supported_auth_modes.contains(&account.auth_mode) {
            anyhow::bail!(
                "Auth mode {:?} not supported for vendor {}",
                account.auth_mode,
                account.vendor_id
            );
        }

        // Set timestamps if not provided
        let now = chrono::Utc::now().to_rfc3339();
        if account.created_at.is_empty() {
            account.created_at = now.clone();
        }
        if account.updated_at.is_empty() {
            account.updated_at = now;
        }

        // Store the API key if provided
        if let Some(key) = &api_key {
            let key_service = crate::services::providers::ProviderApiKeyService::new();
            key_service.set(&account.id, key)?;
            info!("Stored API key for account {}", account.id);
        }

        // Store the account
        self.accounts.insert(account.id.clone(), account.clone());

        // If this is the first account or it should be default
        if self.data.default_account_id.is_none() || account.is_default {
            self.data.default_account_id = Some(account.id.clone());
            // Clear is_default on other accounts
            for (id, acc) in self.accounts.iter_mut() {
                if id != &account.id {
                    acc.is_default = false;
                }
            }
        }

        self.persist().await?;

        info!("Created provider account: {} ({}/{:?})",
            account.id,
            account.label,
            account.auth_mode
        );

        // Sync auth profiles to OpenClaw
        let accounts_vec: Vec<_> = self.accounts.values().cloned().collect();
        if let Err(e) = sync_auth_to_openclaw(&accounts_vec).await {
            warn!("Failed to sync auth profiles to OpenClaw: {}", e);
        }

        Ok(account)
    }

    /// Update a provider account
    pub async fn update_account(
        &mut self,
        account_id: &str,
        updates: ProviderAccountUpdates,
    ) -> Result<ProviderAccount> {
        let existing = self.accounts
            .get_mut(account_id)
            .with_context(|| format!("Provider account not found: {}", account_id))?;

        // Apply updates
        if let Some(label) = updates.label {
            existing.label = label;
        }
        if let Some(auth_mode) = updates.auth_mode {
            // Validate the new auth mode is supported by the vendor
            let vendor = get_provider_definition(&existing.vendor_id)
                .with_context(|| format!("Vendor definition not found: {}", existing.vendor_id))?;
            if !vendor.supported_auth_modes.contains(&auth_mode) {
                anyhow::bail!(
                    "Auth mode {:?} not supported for vendor {}",
                    auth_mode,
                    existing.vendor_id
                );
            }
            existing.auth_mode = auth_mode;
        }
        if let Some(base_url) = updates.base_url {
            existing.base_url = base_url;
        }
        if let Some(api_protocol) = updates.api_protocol {
            existing.api_protocol = api_protocol;
        }
        if let Some(model) = updates.model {
            existing.model = model;
        }
        if let Some(fallback_models) = updates.fallback_models {
            existing.fallback_models = fallback_models;
        }
        if let Some(fallback_account_ids) = updates.fallback_account_ids {
            existing.fallback_account_ids = fallback_account_ids;
        }
        if let Some(enabled) = updates.enabled {
            existing.enabled = enabled;
        }
        if let Some(is_default) = updates.is_default {
            existing.is_default = is_default;
            if is_default {
                self.data.default_account_id = Some(account_id.to_string());
            }
        }
        if let Some(metadata) = updates.metadata {
            existing.metadata = metadata;
        }

        // Update the timestamp
        existing.updated_at = updates.updated_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        let account = existing.clone();
        self.persist().await?;

        info!("Updated provider account: {}", account_id);

        // Sync auth profiles to OpenClaw
        let accounts_vec: Vec<_> = self.accounts.values().cloned().collect();
        if let Err(e) = sync_auth_to_openclaw(&accounts_vec).await {
            warn!("Failed to sync auth profiles to OpenClaw: {}", e);
        }

        Ok(account)
    }

    /// Delete a provider account
    pub async fn delete_account(&mut self, account_id: &str) -> Result<()> {
        let existed = self.accounts.remove(account_id).is_some();

        if existed {
            // If this was the default account, clear it
            if self.data.default_account_id.as_deref() == Some(account_id) {
                self.data.default_account_id = self.accounts.values().next().map(|a| a.id.clone());
                // Set the new default if there are remaining accounts
                if let Some(new_default_id) = &self.data.default_account_id {
                    if let Some(acc) = self.accounts.get_mut(new_default_id) {
                        acc.is_default = true;
                    }
                }
            }

            self.persist().await?;
            info!("Deleted provider account: {}", account_id);

            // Sync auth profiles to OpenClaw
            let accounts_vec: Vec<_> = self.accounts.values().cloned().collect();
            if let Err(e) = sync_auth_to_openclaw(&accounts_vec).await {
                warn!("Failed to sync auth profiles to OpenClaw: {}", e);
            }
        } else {
            warn!("Attempted to delete non-existent provider account: {}", account_id);
        }

        Ok(())
    }

    /// Set the default provider account
    pub async fn set_default_account(&mut self, account_id: &str) -> Result<()> {
        if !self.accounts.contains_key(account_id) {
            anyhow::bail!("Provider account not found: {}", account_id);
        }

        self.data.default_account_id = Some(account_id.to_string());

        // Update is_default flags
        for (id, acc) in self.accounts.iter_mut() {
            acc.is_default = id == account_id;
        }

        self.persist().await?;
        info!("Set default provider account: {}", account_id);

        Ok(())
    }

    /// Get vendor definition by ID
    pub fn get_vendor_definition(&self, vendor_id: &str) -> Option<&'static ProviderDefinition> {
        get_provider_definition(vendor_id)
    }

    /// Get provider environment variable
    pub fn get_provider_env_var(&self, vendor_id: &str) -> Option<&'static str> {
        crate::core::providers::get_provider_env_var(vendor_id)
    }

    /// Get keyable provider types (those with env vars)
    pub fn get_keyable_provider_types(&self) -> Vec<&'static str> {
        crate::core::providers::get_keyable_provider_types()
    }
}

/// Sync provider auth profiles to OpenClaw agent format
///
/// Creates auth-profiles.json in the OpenClaw agent directory
/// with API keys from the secure storage.
pub async fn sync_auth_to_openclaw(
    accounts: &[ProviderAccount],
) -> Result<()> {
    use crate::services::providers::ProviderApiKeyService;

    // Get OpenClaw data directory (uses home directory on all platforms)
    // OpenClaw stores data in ~/.openclaw (or %USERPROFILE%\.openclaw on Windows)
    let home_dir = dirs::home_dir()
        .context("Failed to get home directory")?;
    let openclaw_dir = home_dir.join(".openclaw");

    // Use main agent directory
    let agent_dir = openclaw_dir.join("agents").join("main").join("agent");

    // Ensure agent directory exists
    fs::create_dir_all(&agent_dir).await?;

    let auth_profiles_path = agent_dir.join("auth-profiles.json");

    info!("Syncing auth profiles to {}", auth_profiles_path.display());

    // Build profiles from accounts with API keys
    let mut profiles = HashMap::new();
    let api_key_service = ProviderApiKeyService::new();

    for account in accounts {
        if !account.enabled {
            continue;
        }

        // Try to get API key from secure storage
        match api_key_service.get(&account.id) {
            Ok(Some(api_key)) => {
                // Determine the profile key (vendor_id or account id)
                let profile_key = if accounts.iter().filter(|a| a.vendor_id == account.vendor_id).count() == 1 {
                    // Single account for this vendor, use vendor_id
                    account.vendor_id.clone()
                } else {
                    // Multiple accounts, use account id
                    account.id.clone()
                };

                profiles.insert(profile_key, serde_json::json!({
                    "type": "api_key",
                    "key": api_key,
                    "label": account.label,
                }));

                info!("Synced auth profile for {} ({})", account.label, account.vendor_id);
            }
            Ok(None) => {
                debug!("No API key found for account {}", account.id);
            }
            Err(e) => {
                warn!("Failed to get API key for account {}: {}", account.id, e);
            }
        }
    }

    // Fallback: If no "anthropic" profile exists but we have a default account with a key,
    // add it as "anthropic" so OpenClaw can find it
    if !profiles.contains_key("anthropic") {
        // Find the default account
        if let Some(default_id) = accounts.iter().find(|a| a.is_default).map(|a| a.id.clone()) {
            if let Some(default_account) = accounts.iter().find(|a| a.id == default_id) {
                if let Ok(Some(api_key)) = api_key_service.get(&default_id) {
                    profiles.insert("anthropic".to_string(), serde_json::json!({
                        "type": "api_key",
                        "key": api_key,
                        "label": format!("{} (default)", default_account.label),
                    }));
                    info!("Added anthropic profile from default account {}", default_id);
                }
            }
        }
    }

    // Also add "openai" profile from default account for OpenAI-compatible providers
    if !profiles.contains_key("openai") {
        if let Some(default_id) = accounts.iter().find(|a| a.is_default).map(|a| a.id.clone()) {
            if let Some(default_account) = accounts.iter().find(|a| a.id == default_id) {
                if let Ok(Some(api_key)) = api_key_service.get(&default_id) {
                    profiles.insert("openai".to_string(), serde_json::json!({
                        "type": "api_key",
                        "key": api_key,
                        "label": format!("{} (default)", default_account.label),
                    }));
                    info!("Added openai profile from default account {}", default_id);
                }
            }
        }
    }

    // Build the auth-profiles structure
    let auth_profiles = serde_json::json!({
        "version": 1,
        "profiles": profiles
    });

    // Write to file
    let content = serde_json::to_string_pretty(&auth_profiles)?;
    let mut file = fs::File::create(&auth_profiles_path).await?;
    file.write_all(content.as_bytes()).await?;

    info!("Synced {} auth profiles to {}", profiles.len(), auth_profiles_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_provider_service_crud() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("providers.json");

        let mut service = ProviderService::new(path).await.unwrap();

        // List vendors
        let vendors = service.list_vendors();
        assert!(!vendors.is_empty());
        assert!(vendors.iter().any(|v| v.type_info.id == "anthropic"));

        // Create account
        let account = ProviderAccount {
            id: "test-1".to_string(),
            vendor_id: "anthropic".to_string(),
            label: "Test Account".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
            base_url: None,
            api_protocol: None,
            model: Some("claude-3-opus".to_string()),
            fallback_models: None,
            fallback_account_ids: None,
            enabled: true,
            is_default: false,
            metadata: None,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let created = service.create_account(account, None).await.unwrap();
        assert_eq!(created.id, "test-1");
        assert_eq!(created.label, "Test Account");

        // List accounts
        let accounts = service.list_accounts();
        assert_eq!(accounts.len(), 1);

        // Get account
        let retrieved = service.get_account("test-1");
        assert!(retrieved.is_some());

        // Update account
        let updates = ProviderAccountUpdates {
            label: Some("Updated Account".to_string()),
            ..Default::default()
        };
        let updated = service.update_account("test-1", updates).await.unwrap();
        assert_eq!(updated.label, "Updated Account");

        // Set default
        service.set_default_account("test-1").await.unwrap();
        assert_eq!(service.get_default_account_id(), Some("test-1"));

        // Delete account
        service.delete_account("test-1").await.unwrap();
        assert!(service.get_account("test-1").is_none());
    }

    #[tokio::test]
    async fn test_provider_service_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("providers.json");

        // Create service and add account
        {
            let mut service = ProviderService::new(path.clone()).await.unwrap();
            let account = ProviderAccount {
                id: "persist-1".to_string(),
                vendor_id: "openai".to_string(),
                label: "Persist Test".to_string(),
                auth_mode: ProviderAuthMode::ApiKey,
                base_url: None,
                api_protocol: None,
                model: None,
                fallback_models: None,
                fallback_account_ids: None,
                enabled: true,
                is_default: false,
                metadata: None,
                created_at: String::new(),
                updated_at: String::new(),
            };
            service.create_account(account, None).await.unwrap();
            service.set_default_account("persist-1").await.unwrap();
        }

        // Create new service instance and verify data persisted
        {
            let service = ProviderService::new(path).await.unwrap();
            let account = service.get_account("persist-1");
            assert!(account.is_some());
            assert_eq!(account.unwrap().label, "Persist Test");
            assert_eq!(service.get_default_account_id(), Some("persist-1"));
        }
    }
}
