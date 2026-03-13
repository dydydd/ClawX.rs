//! Skill management IPC command handlers

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::skills::hub::{ClawHubClient, InstalledSkill as HubInstalledSkill, SearchParams};
use crate::core::skills::config::{SkillConfigManager, SkillEntry};

/// Skill representation for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub enabled: bool,
    pub icon: String,
    pub config: Option<HashMap<String, serde_json::Value>>,
    pub is_core: bool,
    pub is_bundled: bool,
}

/// Search request body
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Skill install request
#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    pub slug: String,
    pub version: Option<String>,
    #[serde(default)]
    pub force: bool,
}

/// Skill uninstall request
#[derive(Debug, Deserialize)]
pub struct UninstallRequest {
    pub slug: String,
}

/// Skill config update request
#[derive(Debug, Deserialize)]
pub struct UpdateSkillConfigRequest {
    pub skill_key: String,
    pub api_key: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

/// ClawHub search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawHubSkillResult {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub downloads: Option<u64>,
    pub stars: Option<u32>,
}

/// API Response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn ok(results: T) -> Self {
        Self {
            success: true,
            results: Some(results),
            error: None,
        }
    }

    fn err(error: impl ToString) -> Self {
        Self {
            success: false,
            results: None,
            error: Some(error.to_string()),
        }
    }
}

/// List all installed skills
#[tauri::command]
pub async fn list_skills() -> Result<ApiResponse<Vec<Skill>>, String> {
    let hub_client = ClawHubClient::new().map_err(|e| e.to_string())?;
    let config_manager = SkillConfigManager::new();

    // Get installed skills from ClawHub
    let installed = match hub_client.list_installed().await {
        Ok(skills) => skills,
        Err(e) => {
            tracing::error!("Failed to list installed skills: {}", e);
            vec![]
        }
    };

    // Get all configs
    let configs = match config_manager.get_all_skill_configs().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to get skill configs: {}", e);
            HashMap::new()
        }
    };

    let mut skills = Vec::new();

    for installed_skill in installed {
        let config = configs.get(&installed_skill.slug).cloned().unwrap_or_default();

        // Try to get name and description from SKILL.md
        let name = config_manager
            .extract_skill_name(&installed_skill.slug)
            .await
            .unwrap_or_else(|| installed_skill.slug.clone());

        let description = config_manager
            .extract_skill_description(&installed_skill.slug)
            .await
            .unwrap_or_default();

        skills.push(Skill {
            id: installed_skill.slug.clone(),
            slug: installed_skill.slug.clone(),
            name,
            description,
            version: installed_skill.version,
            author: None,
            enabled: config.enabled.unwrap_or(false),
            icon: "📦".to_string(),
            config: config.env.map(|e| {
                e.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::String(v)))
                    .collect()
            }),
            is_core: false,
            is_bundled: false,
        });
    }

    Ok(ApiResponse::ok(skills))
}

/// Search skills on ClawHub
#[tauri::command]
pub async fn search_skills(request: SearchRequest) -> Result<ApiResponse<Vec<ClawHubSkillResult>>, String> {
    let hub_client = ClawHubClient::new().map_err(|e| e.to_string())?;

    let params = SearchParams {
        query: request.query,
        limit: request.limit,
    };

    match hub_client.search(params).await {
        Ok(results) => {
            let mapped: Vec<ClawHubSkillResult> = results
                .into_iter()
                .map(|r| ClawHubSkillResult {
                    slug: r.slug,
                    name: r.name,
                    description: r.description,
                    version: r.version,
                    author: r.author,
                    downloads: r.downloads,
                    stars: r.stars,
                })
                .collect();
            Ok(ApiResponse::ok(mapped))
        }
        Err(e) => Ok(ApiResponse::err(e)),
    }
}

/// Install a skill
#[tauri::command]
pub async fn install_skill(request: InstallRequest) -> Result<ApiResponse<()>, String> {
    let hub_client = ClawHubClient::new().map_err(|e| e.to_string())?;

    let params = crate::core::skills::hub::InstallParams {
        slug: request.slug,
        version: request.version,
        force: request.force,
    };

    match hub_client.install(params).await {
        Ok(_) => Ok(ApiResponse::ok(())),
        Err(e) => Ok(ApiResponse::err(e)),
    }
}

/// Uninstall a skill
#[tauri::command]
pub async fn uninstall_skill(request: UninstallRequest) -> Result<ApiResponse<()>, String> {
    let hub_client = ClawHubClient::new().map_err(|e| e.to_string())?;

    let params = crate::core::skills::hub::UninstallParams {
        slug: request.slug,
    };

    match hub_client.uninstall(params).await {
        Ok(_) => Ok(ApiResponse::ok(())),
        Err(e) => Ok(ApiResponse::err(e)),
    }
}

/// Get skill config
#[tauri::command]
pub async fn get_skill_config(skill_key: String) -> Result<ApiResponse<SkillEntry>, String> {
    let config_manager = SkillConfigManager::new();

    match config_manager.get_skill_config(&skill_key).await {
        Some(config) => Ok(ApiResponse::ok(config)),
        None => Ok(ApiResponse::ok(SkillEntry::default())),
    }
}

/// Update skill config (apiKey and env)
#[tauri::command]
pub async fn update_skill_config(request: UpdateSkillConfigRequest) -> Result<ApiResponse<()>, String> {
    let config_manager = SkillConfigManager::new();

    match config_manager
        .update_skill_config(&request.skill_key, request.api_key, request.env)
        .await
    {
        Ok(_) => Ok(ApiResponse::ok(())),
        Err(e) => Ok(ApiResponse::err(e)),
    }
}

/// Get all skill configs
#[tauri::command]
pub async fn get_all_skill_configs() -> Result<ApiResponse<HashMap<String, SkillEntry>>, String> {
    let config_manager = SkillConfigManager::new();

    match config_manager.get_all_skill_configs().await {
        Ok(configs) => Ok(ApiResponse::ok(configs)),
        Err(e) => Ok(ApiResponse::err(e)),
    }
}

/// List installed skills from ClawHub (for direct ClawHub API access)
#[tauri::command]
pub async fn clawhub_list_installed() -> Result<ApiResponse<Vec<HubInstalledSkill>>, String> {
    let hub_client = ClawHubClient::new().map_err(|e| e.to_string())?;

    match hub_client.list_installed().await {
        Ok(skills) => Ok(ApiResponse::ok(skills)),
        Err(e) => Ok(ApiResponse::err(e)),
    }
}

/// Open skill readme
#[tauri::command]
pub async fn open_skill_readme(slug: String, fallback_slug: Option<String>) -> Result<ApiResponse<()>, String> {
    let hub_client = ClawHubClient::new().map_err(|e| e.to_string())?;

    match hub_client.open_skill_readme(&slug, fallback_slug.as_deref()).await {
        Ok(_) => Ok(ApiResponse::ok(())),
        Err(e) => Ok(ApiResponse::err(e)),
    }
}
