//! Secret storage using OS keychain

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Secret types that can be stored
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SecretType {
    ApiKey { key: String },
    OAuthToken { access_token: String, refresh_token: Option<String> },
    Password { password: String },
}

/// Secret store using OS keychain
pub struct SecretStore {
    app_name: String,
}

impl SecretStore {
    /// Create a new secret store
    pub fn new(app_name: &str) -> Self {
        Self {
            app_name: app_name.to_string(),
        }
    }

    /// Store a secret for an account
    pub fn set(&self, account_id: &str, secret: &SecretType) -> Result<()> {
        let service = format!("{}-{}", self.app_name, account_id);
        let secret_json = serde_json::to_string(secret)?;

        keyring::Entry::new(&service, account_id)?
            .set_password(&secret_json)?;

        Ok(())
    }

    /// Retrieve a secret for an account
    pub fn get(&self, account_id: &str) -> Result<Option<SecretType>> {
        let service = format!("{}-{}", self.app_name, account_id);

        match keyring::Entry::new(&service, account_id)?.get_password() {
            Ok(secret_json) => {
                let secret = serde_json::from_str(&secret_json)?;
                Ok(Some(secret))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a secret for an account
    pub fn delete(&self, account_id: &str) -> Result<()> {
        let service = format!("{}-{}", self.app_name, account_id);

        match keyring::Entry::new(&service, account_id)?.delete_password() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
            Err(e) => Err(e.into()),
        }
    }
}