//! Secure keyring storage for API keys
//!
//! This module provides a simple abstraction over the OS keychain/keyring
//! for storing sensitive data like API keys. It uses the `keyring` crate
//! which supports Windows (Credential Manager), macOS (Keychain), and Linux (Secret Service).

use anyhow::{Context, Result};

/// Secure storage using OS keyring/keychain
pub struct SecureStorage {
    service_name: String,
}

impl SecureStorage {
    /// Create a new secure storage instance
    ///
    /// # Arguments
    /// * `service_name` - The service name used for the keyring entry (e.g., "ClawX")
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
        }
    }

    /// Store a secret in the OS keyring
    ///
    /// # Arguments
    /// * `account_id` - Unique identifier for the account/secret
    /// * `secret` - The secret value to store (e.g., API key)
    ///
    /// # Errors
    /// Returns an error if the keyring is inaccessible or the operation fails
    pub fn set(&self, account_id: &str, secret: &str) -> Result<()> {
        let entry = keyring::Entry::new(&self.service_name, account_id)
            .with_context(|| format!("Failed to create keyring entry for service: {}", self.service_name))?;

        entry
            .set_password(secret)
            .with_context(|| format!("Failed to store secret for account: {}", account_id))?;

        tracing::debug!("Stored secret in keyring for account: {}", account_id);
        Ok(())
    }

    /// Retrieve a secret from the OS keyring
    ///
    /// # Arguments
    /// * `account_id` - Unique identifier for the account/secret
    ///
    /// # Returns
    /// * `Ok(Some(secret))` - If the secret exists and was retrieved
    /// * `Ok(None)` - If no secret exists for this account
    /// * `Err(_)` - If the keyring is inaccessible
    pub fn get(&self, account_id: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(&self.service_name, account_id)
            .with_context(|| format!("Failed to create keyring entry for service: {}", self.service_name))?;

        match entry.get_password() {
            Ok(secret) => {
                tracing::debug!("Retrieved secret from keyring for account: {}", account_id);
                Ok(Some(secret))
            }
            Err(keyring::Error::NoEntry) => {
                tracing::debug!("No secret found in keyring for account: {}", account_id);
                Ok(None)
            }
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("Failed to retrieve secret for account: {}", account_id))),
        }
    }

    /// Check if a secret exists in the OS keyring
    ///
    /// # Arguments
    /// * `account_id` - Unique identifier for the account/secret
    ///
    /// # Returns
    /// * `Ok(true)` - If a secret exists for this account
    /// * `Ok(false)` - If no secret exists
    /// * `Err(_)` - If the keyring is inaccessible
    pub fn has(&self, account_id: &str) -> Result<bool> {
        match self.get(account_id) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Delete a secret from the OS keyring
    ///
    /// # Arguments
    /// * `account_id` - Unique identifier for the account/secret
    ///
    /// # Returns
    /// * `Ok(())` - If the secret was deleted or didn't exist
    /// * `Err(_)` - If the keyring is inaccessible or the operation fails
    pub fn delete(&self, account_id: &str) -> Result<()> {
        let entry = keyring::Entry::new(&self.service_name, account_id)
            .with_context(|| format!("Failed to create keyring entry for service: {}", self.service_name))?;

        match entry.delete_password() {
            Ok(()) => {
                tracing::debug!("Deleted secret from keyring for account: {}", account_id);
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                tracing::debug!("No secret to delete in keyring for account: {}", account_id);
                Ok(())
            }
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("Failed to delete secret for account: {}", account_id))),
        }
    }
}

impl Default for SecureStorage {
    fn default() -> Self {
        Self::new("ClawX")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_storage_lifecycle() {
        // Use a unique service name for testing to avoid conflicts
        let storage = SecureStorage::new("ClawX-Test");
        let account_id = "test-account";
        let secret = "test-api-key-12345";

        // Clean up any existing entry
        let _ = storage.delete(account_id);

        // Test has (should be false initially)
        assert!(!storage.has(account_id).unwrap());

        // Test set
        storage.set(account_id, secret).unwrap();

        // Test has (should be true now)
        assert!(storage.has(account_id).unwrap());

        // Test get
        let retrieved = storage.get(account_id).unwrap();
        assert_eq!(retrieved, Some(secret.to_string()));

        // Test delete
        storage.delete(account_id).unwrap();

        // Verify deletion
        assert!(!storage.has(account_id).unwrap());
        assert_eq!(storage.get(account_id).unwrap(), None);
    }

    #[test]
    fn test_get_nonexistent() {
        let storage = SecureStorage::new("ClawX-Test");
        let result = storage.get("nonexistent-account-xyz");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_delete_nonexistent() {
        let storage = SecureStorage::new("ClawX-Test");
        // Should not error when deleting non-existent entry
        let result = storage.delete("nonexistent-account-abc");
        assert!(result.is_ok());
    }
}
