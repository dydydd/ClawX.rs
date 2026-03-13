//! ClawHub Client
//!
//! Manages interactions with the ClawHub CLI for skills management.
//! Similar to the Electron ClawHubService but implemented in Rust.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{error, info};

/// Skill representation from ClawHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub downloads: Option<u64>,
    pub stars: Option<u32>,
}

/// Search result from ClawHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSearchResult {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub downloads: Option<u64>,
    pub stars: Option<u32>,
}

/// Search parameters for ClawHub
#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    pub query: String,
    pub limit: Option<u32>,
}

/// Install parameters for ClawHub
#[derive(Debug, Clone)]
pub struct InstallParams {
    pub slug: String,
    pub version: Option<String>,
    pub force: bool,
}

/// Uninstall parameters for ClawHub
#[derive(Debug, Clone)]
pub struct UninstallParams {
    pub slug: String,
}

/// ClawHub API Client
pub struct ClawHubClient {
    work_dir: PathBuf,
    cli_path: PathBuf,
    cli_entry_path: PathBuf,
    use_node_runner: bool,
}

impl ClawHubClient {
    /// Create a new ClawHub client
    pub fn new() -> Result<Self> {
        let work_dir = Self::get_openclaw_config_dir();
        std::fs::create_dir_all(&work_dir)?;

        let cli_entry_path = Self::get_clawhub_cli_entry_path();
        let cli_path = Self::get_clawhub_cli_bin_path();

        let use_node_runner = if cfg!(debug_assertions) {
            !cli_path.exists()
        } else {
            false
        };

        let cli_path = if use_node_runner {
            std::env::current_exe().context("Failed to get current executable path")?
        } else {
            cli_path
        };

        Ok(Self {
            work_dir,
            cli_path,
            cli_entry_path,
            use_node_runner,
        })
    }

    /// Get the OpenClaw config directory (~/.openclaw)
    fn get_openclaw_config_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".openclaw"))
            .unwrap_or_else(|| PathBuf::from(".openclaw"))
    }

    /// Get ClawHub CLI binary path (node_modules/.bin)
    fn get_clawhub_cli_bin_path() -> PathBuf {
        let bin_name = if std::env::consts::OS == "windows" {
            "clawhub.cmd"
        } else {
            "clawhub"
        };
        PathBuf::from("node_modules")
            .join(".bin")
            .join(bin_name)
    }

    /// Get ClawHub CLI entry script path
    fn get_clawhub_cli_entry_path() -> PathBuf {
        PathBuf::from("node_modules")
            .join("clawhub")
            .join("bin")
            .join("clawdhub.js")
    }

    /// Strip ANSI escape sequences from output
    fn strip_ansi(input: &str) -> String {
        let re = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b\[[\(\)][0-9;]*[a-zA-Z]").unwrap_or_else(|_| {
            // Fallback: simple pattern for common ANSI codes
            regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap()
        });
        re.replace_all(input, "").trim().to_string()
    }

    /// Run a ClawHub CLI command
    async fn run_command(&self, args: &[&str]) -> Result<String> {
        if self.use_node_runner && !self.cli_entry_path.exists() {
            return Err(anyhow::anyhow!(
                "ClawHub CLI entry not found at: {}",
                self.cli_entry_path.display()
            ));
        }

        if !self.use_node_runner && !self.cli_path.exists() {
            return Err(anyhow::anyhow!(
                "ClawHub CLI not found at: {}",
                self.cli_path.display()
            ));
        }

        let (command, command_args): (String, Vec<String>) = if self.use_node_runner {
            let exec_path = self
                .cli_path
                .to_str()
                .context("Invalid CLI path")?
                .to_string();
            let entry_path = self
                .cli_entry_path
                .to_str()
                .context("Invalid CLI entry path")?
                .to_string();
            let args: Vec<String> = std::iter::once(entry_path)
                .chain(args.iter().map(|&s| s.to_string()))
                .collect();
            (exec_path, args)
        } else {
            let path = self
                .cli_path
                .to_str()
                .context("Invalid CLI path")?
                .to_string();
            let args: Vec<String> = args.iter().map(|&s| s.to_string()).collect();
            (path, args)
        };

        let display_command = format!("{} {}", command, command_args.join(" "));
        info!("Running ClawHub command: {}", display_command);

        let mut cmd = Command::new(&command);
        cmd.args(&command_args)
            .current_dir(&self.work_dir)
            .env("CI", "true")
            .env("FORCE_COLOR", "0")
            .env("CLAWHUB_WORKDIR", &self.work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if self.use_node_runner {
            cmd.env("ELECTRON_RUN_AS_NODE", "1");
        }

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let output = cmd.output().await.context("Failed to run ClawHub command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            error!("ClawHub command failed with code: {:?}", output.status.code());
            error!("Stderr: {}", stderr);
            return Err(anyhow::anyhow!(
                "Command failed: {}",
                if stderr.is_empty() { &stdout } else { &stderr }
            ));
        }

        Ok(stdout.trim().to_string())
    }

    /// Search for skills on ClawHub
    pub async fn search(&self, params: SearchParams) -> Result<Vec<SkillSearchResult>> {
        if params.query.is_empty() {
            return self.explore(params.limit).await;
        }

        let output = if let Some(limit) = params.limit {
            let limit_str = limit.to_string();
            let args: Vec<&str> = vec!["search", &params.query, "--limit", &limit_str];
            self.run_command(&args).await?
        } else {
            let args: Vec<&str> = vec!["search", &params.query];
            self.run_command(&args).await?
        };

        if output.is_empty() || output.contains("No skills found") {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for line in output.lines() {
            let clean_line = Self::strip_ansi(line);
            if clean_line.is_empty() {
                continue;
            }

            // Try format: slug vversion description (score)
            let parsed = if let Some(caps) = regex::Regex::new(r"^(\S+)\s+v?(\d+\.\S+)\s+(.+)$")
                .ok()
                .and_then(|re| re.captures(&clean_line))
            {
                let slug = caps[1].to_string();
                let version = caps[2].to_string();
                let description = regex::Regex::new(r"\(\d+\.\d+\)$")
                    .unwrap()
                    .replace(&caps[3], "")
                    .trim()
                    .to_string();

                Some(SkillSearchResult {
                    slug,
                    name: caps[1].to_string(),
                    version,
                    description,
                    author: None,
                    downloads: None,
                    stars: None,
                })
            }
            // Fallback: slug  name/description  (score)
            else if let Some(caps) = regex::Regex::new(r"^(\S+)\s+(.+)$")
                .ok()
                .and_then(|re| re.captures(&clean_line))
            {
                let slug = caps[1].to_string();
                let description = regex::Regex::new(r"\(\d+\.\d+\)$")
                    .unwrap()
                    .replace(&caps[2], "")
                    .trim()
                    .to_string();

                Some(SkillSearchResult {
                    slug,
                    name: caps[1].to_string(),
                    version: "latest".to_string(),
                    description,
                    author: None,
                    downloads: None,
                    stars: None,
                })
            } else {
                None
            };

            if let Some(skill) = parsed {
                results.push(skill);
            }
        }

        Ok(results)
    }

    /// Explore trending skills
    pub async fn explore(&self, limit: Option<u32>) -> Result<Vec<SkillSearchResult>> {
        let output = if let Some(limit) = limit {
            let limit_str = limit.to_string();
            let args: Vec<&str> = vec!["explore", "--limit", &limit_str];
            self.run_command(&args).await?
        } else {
            let args: Vec<&str> = vec!["explore"];
            self.run_command(&args).await?
        };

        if output.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for line in output.lines() {
            let clean_line = Self::strip_ansi(line);
            if clean_line.is_empty() {
                continue;
            }

            // Format: slug vversion time description
            // Example: my-skill v1.0.0 2 hours ago A great skill
            if let Some(caps) = regex::Regex::new(
                r"^(\S+)\s+v?(\d+\.\S+)\s+(.+?\s+(?:ago|just now|yesterday))\s+(.+)$",
            )
            .ok()
            .and_then(|re| re.captures(&clean_line))
            {
                results.push(SkillSearchResult {
                    slug: caps[1].to_string(),
                    name: caps[1].to_string(),
                    version: caps[2].to_string(),
                    description: caps[4].to_string(),
                    author: None,
                    downloads: None,
                    stars: None,
                });
            }
        }

        Ok(results)
    }

    /// Get a specific skill by ID (slug)
    pub async fn get_skill(&self, id: &str) -> Option<Skill> {
        // Try to find in installed skills first
        if let Ok(installed) = self.list_installed().await {
            if let Some(skill) = installed.iter().find(|s| s.slug == id) {
                return Some(Skill {
                    slug: skill.slug.clone(),
                    name: skill.slug.clone(),
                    description: String::new(),
                    version: skill.version.clone(),
                    author: None,
                    downloads: None,
                    stars: None,
                });
            }
        }

        // Otherwise search for it
        match self.search(SearchParams {
            query: id.to_string(),
            limit: Some(10),
        }).await {
            Ok(results) => results.into_iter().find(|s| s.slug == id).map(|s| Skill {
                slug: s.slug,
                name: s.name,
                description: s.description,
                version: s.version,
                author: s.author,
                downloads: s.downloads,
                stars: s.stars,
            }),
            Err(_) => None,
        }
    }

    /// Install a skill
    pub async fn install(&self, params: InstallParams) -> Result<()> {
        let mut args = vec!["install", &params.slug];

        if let Some(version) = &params.version {
            args.push("--version");
            args.push(version);
        }

        if params.force {
            args.push("--force");
        }

        self.run_command(&args).await?;
        info!("Successfully installed skill: {}", params.slug);
        Ok(())
    }

    /// Uninstall a skill
    pub async fn uninstall(&self, params: UninstallParams) -> Result<()> {
        let skills_dir = self.work_dir.join("skills");
        let skill_dir = skills_dir.join(&params.slug);

        // 1. Delete the skill directory
        if skill_dir.exists() {
            info!("Deleting skill directory: {}", skill_dir.display());
            tokio::fs::remove_dir_all(&skill_dir)
                .await
                .context("Failed to delete skill directory")?;
        }

        // 2. Remove from lock.json
        let lock_file = self.work_dir.join(".clawhub").join("lock.json");
        if lock_file.exists() {
            let content = tokio::fs::read_to_string(&lock_file).await?;
            let mut lock_data: serde_json::Value = serde_json::from_str(&content)
                .context("Failed to parse lock.json")?;

            if let Some(skills) = lock_data.get_mut("skills") {
                if let Some(obj) = skills.as_object_mut() {
                    if obj.remove(&params.slug).is_some() {
                        info!("Removing {} from lock.json", params.slug);
                        let new_content = serde_json::to_string_pretty(&lock_data)?;
                        tokio::fs::write(&lock_file, new_content).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// List installed skills
    pub async fn list_installed(&self) -> Result<Vec<InstalledSkill>> {
        let output = self.run_command(&["list"]).await?;

        if output.is_empty() || output.contains("No installed skills") {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for line in output.lines() {
            let clean_line = Self::strip_ansi(line);
            if let Some(caps) =
                regex::Regex::new(r"^(\S+)\s+v?(\d+\.\S+)").ok().and_then(|re| re.captures(&clean_line))
            {
                results.push(InstalledSkill {
                    slug: caps[1].to_string(),
                    version: caps[2].to_string(),
                });
            }
        }

        Ok(results)
    }

    /// Open skill readme in default application
    pub async fn open_skill_readme(&self, skill_key_or_slug: &str, fallback_slug: Option<&str>) -> Result<()> {
        let candidates: Vec<String> = [Some(skill_key_or_slug), fallback_slug]
            .into_iter()
            .flatten()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if candidates.is_empty() {
            return Err(anyhow::anyhow!("No valid skill key or slug provided"));
        }

        let skills_root = self.work_dir.join("skills");

        // Try direct path first
        let mut skill_dir: Option<PathBuf> = candidates
            .iter()
            .map(|id| skills_root.join(id))
            .find(|dir| dir.exists());

        // Try resolving by manifest name
        if skill_dir.is_none() {
            skill_dir = self.resolve_skill_dir_by_manifest_name(&candidates).await;
        }

        // Find documentation file
        let possible_files = ["SKILL.md", "README.md", "skill.md", "readme.md"];
        let mut target_file: Option<PathBuf> = None;

        if let Some(ref dir) = skill_dir {
            for file in &possible_files {
                let file_path = dir.join(file);
                if file_path.exists() {
                    target_file = Some(file_path);
                    break;
                }
            }
        }

        // Fall back to directory if no md file
        let target = match target_file {
            Some(file) => file,
            None => match skill_dir {
                Some(dir) => dir,
                None => return Err(anyhow::anyhow!("Skill directory not found")),
            },
        };

        // Open with default application
        open::that(&target).context("Failed to open skill readme")?;
        Ok(())
    }

    /// Resolve skill directory by reading SKILL.md frontmatter
    async fn resolve_skill_dir_by_manifest_name(&self, candidates: &[String]) -> Option<PathBuf> {
        let skills_root = self.work_dir.join("skills");
        if !skills_root.exists() {
            return None;
        }

        let wanted: std::collections::HashSet<String> = candidates
            .iter()
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty())
            .collect();

        let mut entries = match tokio::fs::read_dir(&skills_root).await {
            Ok(e) => e,
            Err(_) => return None,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if !entry.file_type().await.map(|ft| ft.is_dir()).unwrap_or(false) {
                continue;
            }

            let skill_dir = entry.path();
            let skill_manifest = skill_dir.join("SKILL.md");
            if !skill_manifest.exists() {
                continue;
            }

            let frontmatter_name = match self.extract_frontmatter_name(&skill_manifest).await {
                Some(name) => name,
                None => continue,
            };

            if wanted.contains(&frontmatter_name.to_lowercase()) {
                return Some(skill_dir);
            }
        }

        None
    }

    /// Extract name from SKILL.md frontmatter
    async fn extract_frontmatter_name(&self, path: &Path) -> Option<String> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => return None,
        };

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
}

/// Installed skill info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSkill {
    pub slug: String,
    pub version: String,
}
