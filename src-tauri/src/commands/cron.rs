//! Cron job management IPC command handlers
//!
//! Provides local cron job storage and management using a JSON file.

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cron job definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub message: String,
    pub schedule: serde_json::Value,
    #[serde(default)]
    pub target: Option<CronJobTarget>,
    pub enabled: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "lastRun")]
    pub last_run: Option<CronJobLastRun>,
    #[serde(rename = "nextRun")]
    pub next_run: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobTarget {
    #[serde(rename = "channelType")]
    pub channel_type: String,
    #[serde(rename = "channelId")]
    pub channel_id: String,
    #[serde(rename = "channelName")]
    pub channel_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobLastRun {
    pub time: String,
    pub success: bool,
    pub error: Option<String>,
    pub duration: Option<u64>,
}

/// Input for creating a cron job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobCreateInput {
    pub name: String,
    pub message: String,
    pub schedule: String,
    pub enabled: Option<bool>,
}

/// Input for updating a cron job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobUpdateInput {
    pub name: Option<String>,
    pub message: Option<String>,
    pub schedule: Option<String>,
    pub enabled: Option<bool>,
}

/// Cron job store
pub struct CronStore {
    jobs: RwLock<HashMap<String, CronJob>>,
    path: PathBuf,
}

impl CronStore {
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        let path = data_dir.join("cron_jobs.json");
        let jobs = if path.exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            let jobs_vec: Vec<CronJob> = serde_json::from_str(&content).unwrap_or_default();
            jobs_vec.into_iter().map(|j| (j.id.clone(), j)).collect()
        } else {
            HashMap::new()
        };

        Ok(Self {
            jobs: RwLock::new(jobs),
            path,
        })
    }

    async fn persist(&self) -> Result<()> {
        let jobs = self.jobs.read().await;
        let jobs_vec: Vec<_> = jobs.values().cloned().collect();
        let content = serde_json::to_string_pretty(&jobs_vec)?;
        tokio::fs::write(&self.path, content).await?;
        Ok(())
    }

    pub async fn list(&self) -> Vec<CronJob> {
        let jobs = self.jobs.read().await;
        jobs.values().cloned().collect()
    }

    pub async fn get(&self, id: &str) -> Option<CronJob> {
        let jobs = self.jobs.read().await;
        jobs.get(id).cloned()
    }

    pub async fn create(&self, input: CronJobCreateInput) -> CronJob {
        let mut jobs = self.jobs.write().await;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let schedule: serde_json::Value = serde_json::json!({
            "kind": "cron",
            "expr": input.schedule
        });

        let job = CronJob {
            id: id.clone(),
            name: input.name,
            message: input.message,
            schedule,
            target: None,
            enabled: input.enabled.unwrap_or(true),
            created_at: now.clone(),
            updated_at: now,
            last_run: None,
            next_run: None,
        };

        jobs.insert(id, job.clone());

        if let Err(e) = self.persist().await {
            tracing::error!("Failed to persist cron jobs: {}", e);
        }

        job
    }

    pub async fn update(&self, id: &str, input: CronJobUpdateInput) -> Option<CronJob> {
        let mut jobs = self.jobs.write().await;

        if let Some(job) = jobs.get_mut(id) {
            if let Some(name) = input.name {
                job.name = name;
            }
            if let Some(message) = input.message {
                job.message = message;
            }
            if let Some(schedule) = input.schedule {
                job.schedule = serde_json::json!({
                    "kind": "cron",
                    "expr": schedule
                });
            }
            if let Some(enabled) = input.enabled {
                job.enabled = enabled;
            }
            job.updated_at = Utc::now().to_rfc3339();

            let updated = job.clone();

            if let Err(e) = self.persist().await {
                tracing::error!("Failed to persist cron jobs: {}", e);
            }

            Some(updated)
        } else {
            None
        }
    }

    pub async fn delete(&self, id: &str) -> bool {
        let mut jobs = self.jobs.write().await;
        let removed = jobs.remove(id).is_some();

        if removed {
            if let Err(e) = self.persist().await {
                tracing::error!("Failed to persist cron jobs: {}", e);
            }
        }

        removed
    }

    pub async fn toggle(&self, id: &str, enabled: bool) -> Option<CronJob> {
        let mut jobs = self.jobs.write().await;

        if let Some(job) = jobs.get_mut(id) {
            job.enabled = enabled;
            job.updated_at = Utc::now().to_rfc3339();

            let updated = job.clone();

            if let Err(e) = self.persist().await {
                tracing::error!("Failed to persist cron jobs: {}", e);
            }

            Some(updated)
        } else {
            None
        }
    }

    pub async fn trigger(&self, _id: &str) -> Result<(), String> {
        // TODO: Implement actual job execution
        // For now, just update lastRun
        Err("Job execution not implemented yet".to_string())
    }
}

// Global cron store instance
static CRON_STORE: std::sync::OnceLock<Arc<CronStore>> = std::sync::OnceLock::new();

pub async fn init_cron_store(data_dir: PathBuf) -> Result<()> {
    let store = CronStore::new(data_dir).await?;
    let _ = CRON_STORE.set(Arc::new(store));
    Ok(())
}

fn get_cron_store() -> Result<Arc<CronStore>, String> {
    CRON_STORE
        .get()
        .cloned()
        .ok_or_else(|| "Cron store not initialized".to_string())
}

/// List all cron jobs
#[tauri::command]
pub async fn cron_list() -> Result<Vec<CronJob>, String> {
    let store = get_cron_store()?;
    Ok(store.list().await)
}

/// Create a cron job
#[tauri::command]
pub async fn cron_create(input: CronJobCreateInput) -> Result<CronJob, String> {
    let store = get_cron_store()?;
    Ok(store.create(input).await)
}

/// Update a cron job
#[tauri::command]
pub async fn cron_update(id: String, input: CronJobUpdateInput) -> Result<CronJob, String> {
    let store = get_cron_store()?;
    store
        .update(&id, input)
        .await
        .ok_or_else(|| "Job not found".to_string())
}

/// Delete a cron job
#[tauri::command]
pub async fn cron_delete(id: String) -> Result<(), String> {
    let store = get_cron_store()?;
    if store.delete(&id).await {
        Ok(())
    } else {
        Err("Job not found".to_string())
    }
}

/// Toggle cron job enabled state
#[tauri::command]
pub async fn cron_toggle(id: String, enabled: bool) -> Result<CronJob, String> {
    let store = get_cron_store()?;
    store
        .toggle(&id, enabled)
        .await
        .ok_or_else(|| "Job not found".to_string())
}

/// Trigger a cron job manually
#[tauri::command]
pub async fn cron_trigger(id: String) -> Result<(), String> {
    let store = get_cron_store()?;
    store.trigger(&id).await
}