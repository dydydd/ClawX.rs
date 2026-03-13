//! Provider API Key management service
//!
//! This module provides a service for managing provider API keys using
//! the OS keyring. It handles naming conventions for different provider types.

use anyhow::Result;
use crate::core::storage::SecureStorage;

/// Service name for provider API keys in the keyring
const PROVIDER_KEY_SERVICE: &str = "ClawX";

/// Prefix for provider API key entries in the keyring
const PROVIDER_KEY_PREFIX: &str = "provider-api-key";

/// Provider API Key service
pub struct ProviderApiKeyService {
    storage: SecureStorage,
}

impl ProviderApiKeyService {
    /// Create a new provider API key service
    pub fn new() -> Self {
        Self {
            storage: SecureStorage::new(PROVIDER_KEY_SERVICE),
        }
    }

    /// Build the account ID for a provider's API key
    ///
    /// The format is: `provider-api-key:{provider_id}`
    fn build_account_id(&self, provider_id: &str) -> String {
        format!("{}:{}", PROVIDER_KEY_PREFIX, provider_id)
    }

    /// Store an API key for a provider
    ///
    /// # Arguments
    /// * `provider_id` - The unique provider identifier (e.g., "openai", "anthropic")
    /// * `api_key` - The API key to store
    pub fn set(&self, provider_id: &str, api_key: &str) -> Result<()> {
        let account_id = self.build_account_id(provider_id);
        self.storage.set(&account_id, api_key)
    }

    /// Retrieve an API key for a provider
    ///
    /// # Arguments
    /// * `provider_id` - The unique provider identifier
    ///
    /// # Returns
    /// * `Ok(Some(api_key))` - If the API key exists
    /// * `Ok(None)` - If no API key is stored for this provider
    pub fn get(&self, provider_id: &str) -> Result<Option<String>> {
        let account_id = self.build_account_id(provider_id);
        self.storage.get(&account_id)
    }

    /// Check if a provider has an API key stored
    ///
    /// # Arguments
    /// * `provider_id` - The unique provider identifier
    pub fn has(&self, provider_id: &str) -> Result<bool> {
        let account_id = self.build_account_id(provider_id);
        self.storage.has(&account_id)
    }

    /// Delete an API key for a provider
    ///
    /// # Arguments
    /// * `provider_id` - The unique provider identifier
    pub fn delete(&self, provider_id: &str) -> Result<()> {
        let account_id = self.build_account_id(provider_id);
        self.storage.delete(&account_id)
    }

    /// Get a masked version of the API key for display
    ///
    /// Shows first 4 and last 4 characters, with asterisks in between.
    /// For short keys, shows all asterisks.
    ///
    /// # Arguments
    /// * `provider_id` - The unique provider identifier
    ///
    /// # Returns
    /// * `Ok(Some(masked_key))` - If the API key exists
    /// * `Ok(None)` - If no API key is stored
    pub fn get_masked(&self, provider_id: &str) -> Result<Option<String>> {
        match self.get(provider_id)? {
            Some(api_key) => {
                let masked = if api_key.len() > 12 {
                    format!(
                        "{}{}{}",
                        &api_key[..4],
                        "*".repeat(api_key.len() - 8),
                        &api_key[api_key.len() - 4..]
                    )
                } else {
                    "*".repeat(api_key.len())
                };
                Ok(Some(masked))
            }
            None => Ok(None),
        }
    }
}

impl Default for ProviderApiKeyService {
    fn default() -> Self {
        Self::new()
    }
}

/// Create the default provider API key service instance
pub fn create_provider_api_key_service() -> ProviderApiKeyService {
    ProviderApiKeyService::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_id_format() {
        let service = ProviderApiKeyService::new();
        assert_eq!(
            service.build_account_id("openai"),
            "provider-api-key:openai"
        );
        assert_eq!(
            service.build_account_id("anthropic-claude"),
            "provider-api-key:anthropic-claude"
        );
    }

    #[test]
    fn test_api_key_lifecycle() {
        let service = ProviderApiKeyService::new();
        let provider_id = "test-provider-lifecycle";
        let api_key = "sk-test1234567890abcdef";

        // Clean up
        let _ = service.delete(provider_id);

        // Initially no key
        assert!(!service.has(provider_id).unwrap());
        assert_eq!(service.get(provider_id).unwrap(), None);

        // Set key
        service.set(provider_id, api_key).unwrap();

        // Key should exist
        assert!(service.has(provider_id).unwrap());
        assert_eq!(service.get(provider_id).unwrap(), Some(api_key.to_string()));

        // Check masking
        let masked = service.get_masked(provider_id).unwrap().unwrap();
        assert!(masked.starts_with("sk-t"));
        assert!(masked.ends_with("cdef"));
        assert!(masked.contains("***"));

        // Delete key
        service.delete(provider_id).unwrap();

        // Key should be gone
        assert!(!service.has(provider_id).unwrap());
        assert_eq!(service.get(provider_id).unwrap(), None);
    }

    #[test]
    fn test_mask_short_key() {
        let service = ProviderApiKeyService::new();
        let provider_id = "test-short-key";
        let short_key = "abc123";

        // Clean up
        let _ = service.delete(provider_id);

        service.set(provider_id, short_key).unwrap();
        let masked = service.get_masked(provider_id).unwrap().unwrap();
        assert_eq!(masked, "******");

        // Clean up
        let _ = service.delete(provider_id);
    }

    #[test]
    fn test_mask_long_key() {
        let service = ProviderApiKeyService::new();
        let provider_id = "test-long-key";
        let long_key = "sk-abcdefghijklmnopqrstuvwxyz1234567890";

        // Clean up
        let _ = service.delete(provider_id);

        service.set(provider_id, long_key).unwrap();
        let masked = service.get_masked(provider_id).unwrap().unwrap();

        // Should be: sk-a + ****... + 7890
        assert!(masked.starts_with("sk-a"));
        assert!(masked.ends_with("7890"));
        // Total length should be same as original
        assert_eq!(masked.len(), long_key.len());

        // Clean up
        let _ = service.delete(provider_id);
    }
}
