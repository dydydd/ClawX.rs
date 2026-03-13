//! Skill Configuration Manager
//!
//! Direct read/write access to skill configuration in ~/.openclaw/openclaw.json
//! This bypasses the Gateway RPC for faster and more reliable config updates.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info};

/// Skill entry in config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillEntry {
    pub enabled: Option<bool>,
    pub api_key: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

/// OpenClaw config structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenClawConfig {
    pub skills: Option<SkillsConfig>,
}

/// Skills config section
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillsConfig {
    pub entries: Option<HashMap<String, SkillEntry>>,
}

/// Pre-installed skill specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreinstalledSkillSpec {
    pub slug: String,
    pub version: Option<String>,
    #[serde(default)]
    pub auto_enable: bool,
}

/// Pre-installed manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreinstalledManifest {
    pub skills: Option<Vec<PreinstalledSkillSpec>>,
}

/// Pre-installed marker structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreinstalledMarker {
    pub source: String,
    pub slug: String,
    pub version: String,
    pub installed_at: String,
}

/// Installed skill info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSkill {
    pub slug: String,
    pub version: String,
    pub path: PathBuf,
    pub enabled: bool,
}

/// Skill configuration manager
pub struct SkillConfigManager {
    config_path: PathBuf,
    skills_dir: PathBuf,
}

impl SkillConfigManager {
    /// Create a new skill config manager
    pub fn new() -> Self {
        let config_dir = Self::get_openclaw_config_dir();
        let config_path = config_dir.join("openclaw.json");
        let skills_dir = config_dir.join("skills");

        Self {
            config_path,
            skills_dir,
        }
    }

    /// Get the OpenClaw config directory (~/.openclaw)
    fn get_openclaw_config_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".openclaw"))
            .unwrap_or_else(|| PathBuf::from(".openclaw"))
    }

    /// Read the current OpenClaw config
    pub async fn read_config(&self) -> Result<OpenClawConfig> {
        if !self.config_path.exists() {
            return Ok(OpenClawConfig::default());
        }

        let mut file = fs::File::open(&self.config_path).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;

        match serde_json::from_str(&content) {
            Ok(config) => Ok(config),
            Err(e) => {
                error!("Failed to parse openclaw config: {}", e);
                Ok(OpenClawConfig::default())
            }
        }
    }

    /// Write the OpenClaw config
    pub async fn write_config(&self, config: &OpenClawConfig) -> Result<()> {
        let json = serde_json::to_string_pretty(config)?;

        // Ensure directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::File::create(&self.config_path).await?;
        file.write_all(json.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    /// Get skill config by key
    pub async fn get_skill_config(&self, skill_key: &str) -> Option<SkillEntry> {
        let config = self.read_config().await.ok()?;
        config
            .skills?
            .entries?
            .get(skill_key)
            .cloned()
    }

    /// Update skill config (apiKey and env)
    pub async fn update_skill_config(
        &self,
        skill_key: &str,
        api_key: Option<String>,
        env: Option<HashMap<String, String>>,
    ) -> Result<()> {
        let mut config = self.read_config().await?;

        // Ensure skills.entries exists
        if config.skills.is_none() {
            config.skills = Some(SkillsConfig::default());
        }
        let skills = config.skills.as_mut().unwrap();
        if skills.entries.is_none() {
            skills.entries = Some(HashMap::new());
        }
        let entries = skills.entries.as_mut().unwrap();

        // Get or create entry
        let entry = entries.entry(skill_key.to_string()).or_default();

        // Update apiKey
        if let Some(key) = api_key {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                entry.api_key = None;
            } else {
                entry.api_key = Some(trimmed.to_string());
            }
        }

        // Update env
        if let Some(new_env) = env {
            let mut filtered_env: HashMap<String, String> = HashMap::new();
            for (key, value) in new_env {
                let trimmed_key = key.trim();
                let trimmed_val = value.trim();
                if !trimmed_key.is_empty() && !trimmed_val.is_empty() {
                    filtered_env.insert(trimmed_key.to_string(), trimmed_val.to_string());
                }
            }

            if filtered_env.is_empty() {
                entry.env = None;
            } else {
                entry.env = Some(filtered_env);
            }
        }

        self.write_config(&config).await?;
        info!("Updated skill config for: {}", skill_key);
        Ok(())
    }

    /// Get all skill configs
    pub async fn get_all_skill_configs(&self) -> Result<HashMap<String, SkillEntry>> {
        let config = self.read_config().await?;
        Ok(config
            .skills
            .and_then(|s| s.entries)
            .unwrap_or_default())
    }

    /// List all installed skills
    pub async fn list_installed_skills(&self) -> Result<Vec<InstalledSkill>> {
        let mut skills = Vec::new();

        if !self.skills_dir.exists() {
            return Ok(skills);
        }

        let config = self.read_config().await?;
        let entries = config
            .skills
            .and_then(|s| s.entries)
            .unwrap_or_default();

        let mut read_dir = fs::read_dir(&self.skills_dir).await?;

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if !file_type.is_dir() {
                continue;
            }

            let slug = entry.file_name().to_string_lossy().to_string();
            let skill_dir = entry.path();

            // Check if SKILL.md exists
            let skill_md = skill_dir.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            // Get version from lock file if available
            let version = self.get_skill_version(&slug).await.unwrap_or_else(|| "unknown".to_string());

            // Get enabled status from config
            let enabled = entries
                .get(&slug)
                .and_then(|e| e.enabled)
                .unwrap_or(false);

            skills.push(InstalledSkill {
                slug,
                version,
                path: skill_dir,
                enabled,
            });
        }

        Ok(skills)
    }

    /// Check if a skill is installed
    pub async fn is_skill_installed(&self, id: &str) -> bool {
        let skill_dir = self.skills_dir.join(id);
        let skill_md = skill_dir.join("SKILL.md");
        skill_md.exists()
    }

    /// Get skill version from lock file
    async fn get_skill_version(&self, slug: &str) -> Option<String> {
        let lock_file = self.skills_dir.parent().unwrap().join(".clawhub").join("lock.json");
        if !lock_file.exists() {
            return None;
        }

        let content = match fs::read_to_string(&lock_file).await {
            Ok(c) => c,
            Err(_) => return None,
        };

        let lock_data: serde_json::Value = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(_) => return None,
        };

        lock_data
            .get("skills")?
            .get(slug)?
            .get("version")?
            .as_str()
            .map(|s| s.to_string())
    }

    /// Set skills enabled status
    pub async fn set_skills_enabled(&self, skill_keys: Vec<String>, enabled: bool) -> Result<()> {
        if skill_keys.is_empty() {
            return Ok(());
        }

        let mut config = self.read_config().await?;

        if config.skills.is_none() {
            config.skills = Some(SkillsConfig::default());
        }
        let skills = config.skills.as_mut().unwrap();
        if skills.entries.is_none() {
            skills.entries = Some(HashMap::new());
        }
        let entries = skills.entries.as_mut().unwrap();

        for skill_key in skill_keys {
            let entry = entries.entry(skill_key).or_default();
            entry.enabled = Some(enabled);
        }

        self.write_config(&config).await?;
        Ok(())
    }

    /// Get skill path
    pub fn get_skill_path(&self, slug: &str) -> PathBuf {
        self.skills_dir.join(slug)
    }

    /// Read SKILL.md content
    pub async fn read_skill_manifest(&self, slug: &str) -> Result<String> {
        let skill_dir = self.skills_dir.join(slug);
        let skill_md = skill_dir.join("SKILL.md");

        if !skill_md.exists() {
            return Err(anyhow::anyhow!("SKILL.md not found for skill: {}", slug));
        }

        fs::read_to_string(&skill_md)
            .await
            .context("Failed to read SKILL.md")
    }

    /// Extract name from SKILL.md frontmatter
    pub async fn extract_skill_name(&self, slug: &str) -> Option<String> {
        let content = self.read_skill_manifest(slug).await.ok()?;

        // Match the first frontmatter block and read `name: ...`
        let frontmatter = content.split("---").nth(1)?;
        for line in frontmatter.lines() {
            // Parse name: value or name: "value" or name: 'value'
            let line = line.trim();
            if line.starts_with("name:") {
                let value = line[5..].trim();
                // Remove quotes if present
                let name = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                    value[1..value.len()-1].trim()
                } else if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
                    value[1..value.len()-1].trim()
                } else {
                    value
                };
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        None
    }

    /// Extract description from SKILL.md
    pub async fn extract_skill_description(&self, slug: &str) -> Option<String> {
        let content = self.read_skill_manifest(slug).await.ok()?;

        // Find content after frontmatter
        let parts: Vec<&str> = content.split("---").collect();
        if parts.len() >= 3 {
            // Content is after the second ---
            let content_part = parts[2..].join("---");
            let trimmed = content_part.trim();
            // Take first paragraph or line
            trimmed.lines().next().map(|s| s.trim().to_string())
        } else {
            None
        }
    }
}

impl Default for SkillConfigManager {
    fn default() -> Self {
        Self::new()
    }
}
