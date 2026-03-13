//! Provider storage

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub config: serde_json::Value,
}

/// Provider store backed by a JSON file
pub struct ProviderStore {
    path: PathBuf,
    providers: HashMap<String, Provider>,
}

impl ProviderStore {
    /// Create a new provider store
    pub async fn new(path: PathBuf) -> Result<Self> {
        let providers = if path.exists() {
            let content = fs::read_to_string(&path).await?;
            let list: Vec<Provider> = serde_json::from_str(&content)?;
            list.into_iter().map(|p| (p.id.clone(), p)).collect()
        } else {
            HashMap::new()
        };

        Ok(Self { path, providers })
    }

    /// List all providers
    pub fn list(&self) -> Vec<&Provider> {
        self.providers.values().collect()
    }

    /// Get a provider by ID
    pub fn get(&self, id: &str) -> Option<&Provider> {
        self.providers.get(id)
    }

    /// Save a provider
    pub fn save(&mut self, provider: Provider) {
        self.providers.insert(provider.id.clone(), provider);
    }

    /// Delete a provider
    pub fn delete(&mut self, id: &str) {
        self.providers.remove(id);
    }

    /// Save providers to disk
    pub async fn persist(&self) -> Result<()> {
        let list: Vec<_> = self.providers.values().cloned().collect();
        let content = serde_json::to_string_pretty(&list)?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::File::create(&self.path).await?;
        file.write_all(content.as_bytes()).await?;

        Ok(())
    }
}