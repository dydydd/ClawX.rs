//! Cron job management IPC command handlers
//!
//! Provides local cron job storage and management using a JSON file.
//! Includes a scheduler that automatically executes jobs when they're due.

use anyhow::Result;
use chrono::{Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

use crate::core::gateway::GatewayManager;

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
    pub target: Option<CronJobTarget>,
}

/// Input for updating a cron job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobUpdateInput {
    pub name: Option<String>,
    pub message: Option<String>,
    pub schedule: Option<String>,
    pub enabled: Option<bool>,
    pub target: Option<CronJobTarget>,
}

/// Calculate the next run time based on cron expression
/// Simple implementation for common patterns
fn calculate_next_run(cron_expr: &str) -> Option<String> {
    let now = chrono::Local::now();
    let next = now.clone();

    // Parse cron expression (minute hour day_of_month month day_of_week)
    let parts: Vec<&str> = cron_expr.split_whitespace().collect();
    if parts.len() != 5 {
        return None;
    }

    let minute = parts[0];
    let hour = parts[1];
    let _day_of_month = parts[2];
    let _month = parts[3];
    let day_of_week = parts[4];

    // Simple implementation for common patterns
    // Every minute: * * * * *
    if minute == "*" && hour == "*" {
        let next_time = now + chrono::Duration::minutes(1);
        return Some(next_time.format("%Y-%m-%dT%H:%M:00Z").to_string());
    }

    // Every N minutes: */N * * * *
    if minute.starts_with("*/") {
        if let Ok(interval) = minute[2..].parse::<u32>() {
            let current_minute = now.minute();
            let next_minute = ((current_minute / interval) + 1) * interval;
            let next_time = if next_minute >= 60 {
                now.with_minute(0)?.with_hour((now.hour() + 1) % 24)?
            } else {
                now.with_minute(next_minute)?
            };
            return Some(next_time.format("%Y-%m-%dT%H:%M:00Z").to_string());
        }
    }

    // Every hour: 0 * * * *
    if hour == "*" && minute == "0" {
        let next_time = now.with_minute(0)? + chrono::Duration::hours(1);
        return Some(next_time.format("%Y-%m-%dT%H:%M:00Z").to_string());
    }

    // Daily at specific time: M H * * *
    if hour != "*" && minute != "*" && _day_of_month == "*" && day_of_week == "*" {
        if let (Ok(h), Ok(m)) = (hour.parse::<u32>(), minute.parse::<u32>()) {
            let mut next_time = now.with_hour(h)?.with_minute(m)?;
            if next_time <= now {
                next_time = next_time + chrono::Duration::days(1);
            }
            return Some(next_time.format("%Y-%m-%dT%H:%M:00Z").to_string());
        }
    }

    // Weekly on specific day: M H * * DOW
    if day_of_week != "*" && hour != "*" && minute != "*" {
        if let (Ok(h), Ok(m), Ok(dow)) = (
            hour.parse::<u32>(),
            minute.parse::<u32>(),
            day_of_week.parse::<u32>(),
        ) {
            let current_dow = now.weekday().num_days_from_monday();
            let days_ahead = (dow + 7 - current_dow) % 7;
            let next_time = (now + chrono::Duration::days(days_ahead as i64))
                .with_hour(h)?
                .with_minute(m)?;
            return Some(next_time.format("%Y-%m-%dT%H:%M:00Z").to_string());
        }
    }

    // For other patterns, return None (not implemented)
    None
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
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let schedule: serde_json::Value = serde_json::json!({
            "kind": "cron",
            "expr": input.schedule
        });

        // Calculate next run time
        let next_run = calculate_next_run(&input.schedule);

        let job = CronJob {
            id: id.clone(),
            name: input.name,
            message: input.message,
            schedule,
            target: input.target,
            enabled: input.enabled.unwrap_or(true),
            created_at: now.clone(),
            updated_at: now,
            last_run: None,
            next_run,
        };

        // Insert job into map
        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(id, job.clone());
        }

        // Persist after releasing the lock
        if let Err(e) = self.persist().await {
            tracing::error!("Failed to persist cron jobs: {}", e);
        }

        job
    }

    pub async fn update(&self, id: &str, input: CronJobUpdateInput) -> Option<CronJob> {
        let updated = {
            let mut jobs = self.jobs.write().await;

            if let Some(job) = jobs.get_mut(id) {
                if let Some(name) = input.name {
                    job.name = name;
                }
                if let Some(message) = input.message {
                    job.message = message;
                }
                if let Some(schedule) = &input.schedule {
                    job.schedule = serde_json::json!({
                        "kind": "cron",
                        "expr": schedule
                    });
                    // Update next run time when schedule changes
                    job.next_run = calculate_next_run(schedule);
                }
                if let Some(enabled) = input.enabled {
                    job.enabled = enabled;
                    // Recalculate next run when enabled status changes
                    if enabled {
                        if let Some(expr) = job.schedule.get("expr").and_then(|e| e.as_str()) {
                            job.next_run = calculate_next_run(expr);
                        }
                    } else {
                        job.next_run = None;
                    }
                }
                if let Some(target) = input.target {
                    job.target = Some(target);
                }
                job.updated_at = Utc::now().to_rfc3339();

                Some(job.clone())
            } else {
                None
            }
        };

        // Persist after releasing the lock
        if updated.is_some() {
            if let Err(e) = self.persist().await {
                tracing::error!("Failed to persist cron jobs: {}", e);
            }
        }

        updated
    }

    pub async fn delete(&self, id: &str) -> bool {
        let removed = {
            let mut jobs = self.jobs.write().await;
            jobs.remove(id).is_some()
        };

        if removed {
            if let Err(e) = self.persist().await {
                tracing::error!("Failed to persist cron jobs: {}", e);
            }
        }

        removed
    }

    pub async fn toggle(&self, id: &str, enabled: bool) -> Option<CronJob> {
        let updated = {
            let mut jobs = self.jobs.write().await;

            if let Some(job) = jobs.get_mut(id) {
                job.enabled = enabled;
                job.updated_at = Utc::now().to_rfc3339();

                // Update next run time based on enabled status
                if enabled {
                    if let Some(expr) = job.schedule.get("expr").and_then(|e| e.as_str()) {
                        job.next_run = calculate_next_run(expr);
                    }
                } else {
                    job.next_run = None;
                }

                Some(job.clone())
            } else {
                None
            }
        };

        // Persist after releasing the lock
        if updated.is_some() {
            if let Err(e) = self.persist().await {
                tracing::error!("Failed to persist cron jobs: {}", e);
            }
        }

        updated
    }
}

// Global cron store instance
static CRON_STORE: std::sync::OnceLock<Arc<CronStore>> = std::sync::OnceLock::new();

pub async fn init_cron_store(data_dir: PathBuf) -> Result<Arc<CronStore>> {
    let store = CronStore::new(data_dir).await?;
    let arc_store = Arc::new(store);
    let _ = CRON_STORE.set(arc_store.clone());
    Ok(arc_store)
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
pub async fn cron_trigger(
    id: String,
    state: tauri::State<'_, Arc<crate::core::AppState>>,
) -> Result<(), String> {
    let store = get_cron_store()?;

    // Get job details first
    let job = {
        let jobs = store.jobs.read().await;
        jobs.get(&id).cloned()
    };

    let job = job.ok_or("Job not found")?;

    // Execute the job via Gateway RPC
    let start_time = std::time::Instant::now();
    let execution_result = if let Some(target) = &job.target {
        // Send to specific channel
        tracing::info!(
            "Executing cron job '{}' to channel {} ({})",
            job.name,
            target.channel_name,
            target.channel_type
        );

        state.gateway
            .rpc(
                "chat.send",
                Some(serde_json::json!({
                    "sessionKey": format!("agent:main:{}", target.channel_type),
                    "message": job.message,
                    "deliver": true,
                    "idempotencyKey": uuid::Uuid::new_v4().to_string(),
                })),
                30000,
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    } else {
        // Send to default session (main chat)
        tracing::info!(
            "Executing cron job '{}' to default session",
            job.name
        );

        state.gateway
            .rpc(
                "chat.send",
                Some(serde_json::json!({
                    "sessionKey": "agent:main:main",
                    "message": job.message,
                    "deliver": false,
                    "idempotencyKey": uuid::Uuid::new_v4().to_string(),
                })),
                30000,
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    };

    let duration = start_time.elapsed().as_millis() as u64;

    // Update job with execution result
    let updated = {
        let mut jobs = store.jobs.write().await;

        if let Some(job) = jobs.get_mut(&id) {
            job.last_run = Some(CronJobLastRun {
                time: Utc::now().to_rfc3339(),
                success: execution_result.is_ok(),
                error: execution_result.as_ref().err().cloned(),
                duration: Some(duration),
            });
            job.updated_at = Utc::now().to_rfc3339();

            // Update next run time
            if job.enabled {
                if let Some(expr) = job.schedule.get("expr").and_then(|e| e.as_str()) {
                    job.next_run = calculate_next_run(expr);
                }
            }

            tracing::info!(
                "Cron job '{}' executed in {}ms, success={}",
                job.name,
                duration,
                execution_result.is_ok()
            );

            Some(job.clone())
        } else {
            None
        }
    };

    // Persist after releasing the lock
    if updated.is_some() {
        if let Err(e) = store.persist().await {
            tracing::error!("Failed to persist cron jobs: {}", e);
        }
    }

    execution_result
}

/// Cron job scheduler that periodically checks and executes due jobs
pub struct CronScheduler {
    store: Arc<CronStore>,
    gateway: Arc<GatewayManager>,
    running: Arc<RwLock<bool>>,
}

impl CronScheduler {
    pub fn new(store: Arc<CronStore>, gateway: Arc<GatewayManager>) -> Self {
        Self {
            store,
            gateway,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the scheduler (runs in background)
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            tracing::warn!("Cron scheduler is already running");
            return;
        }
        *running = true;
        drop(running);

        let store = self.store.clone();
        let gateway = self.gateway.clone();
        let running_flag = self.running.clone();

        tokio::spawn(async move {
            tracing::info!("Cron scheduler started");

            // Check every 30 seconds
            let mut check_interval = interval(Duration::from_secs(30));

            loop {
                check_interval.tick().await;

                // Check if scheduler is still running
                {
                    let is_running = running_flag.read().await;
                    if !*is_running {
                        tracing::info!("Cron scheduler stopped");
                        break;
                    }
                }

                // Get current time
                let now = chrono::Local::now();
                let now_str = now.format("%Y-%m-%dT%H:%M:00Z").to_string();

                // Get all enabled jobs
                let jobs = store.list().await;
                let enabled_jobs: Vec<_> = jobs.into_iter().filter(|j| j.enabled).collect();

                for job in enabled_jobs {
                    // Check if job is due
                    if let Some(next_run) = &job.next_run {
                        if next_run <= &now_str {
                            tracing::info!(
                                "Executing scheduled cron job '{}' (id: {})",
                                job.name,
                                job.id
                            );

                            // Execute the job
                            let start_time = std::time::Instant::now();
                            let execution_result = if let Some(target) = &job.target {
                                // Send to specific channel
                                gateway
                                    .rpc(
                                        "chat.send",
                                        Some(serde_json::json!({
                                            "sessionKey": format!("agent:main:{}", target.channel_type),
                                            "message": job.message,
                                            "deliver": true,
                                            "idempotencyKey": uuid::Uuid::new_v4().to_string(),
                                        })),
                                        30000,
                                    )
                                    .await
                                    .map(|_| ())
                                    .map_err(|e| e.to_string())
                            } else {
                                // Send to default session (main chat)
                                gateway
                                    .rpc(
                                        "chat.send",
                                        Some(serde_json::json!({
                                            "sessionKey": "agent:main:main",
                                            "message": job.message,
                                            "deliver": false,
                                            "idempotencyKey": uuid::Uuid::new_v4().to_string(),
                                        })),
                                        30000,
                                    )
                                    .await
                                    .map(|_| ())
                                    .map_err(|e| e.to_string())
                            };

                            let duration = start_time.elapsed().as_millis() as u64;

                            // Update job with execution result
                            let updated = {
                                let mut jobs = store.jobs.write().await;

                                if let Some(job) = jobs.get_mut(&job.id) {
                                    job.last_run = Some(CronJobLastRun {
                                        time: Utc::now().to_rfc3339(),
                                        success: execution_result.is_ok(),
                                        error: execution_result.as_ref().err().cloned(),
                                        duration: Some(duration),
                                    });
                                    job.updated_at = Utc::now().to_rfc3339();

                                    // Update next run time
                                    if job.enabled {
                                        if let Some(expr) =
                                            job.schedule.get("expr").and_then(|e| e.as_str())
                                        {
                                            job.next_run = calculate_next_run(expr);
                                        }
                                    }

                                    tracing::info!(
                                        "Cron job '{}' executed in {}ms, success={}",
                                        job.name,
                                        duration,
                                        execution_result.is_ok()
                                    );

                                    if let Err(e) = &execution_result {
                                        tracing::error!(
                                            "Cron job '{}' execution failed: {}",
                                            job.name,
                                            e
                                        );
                                    }

                                    Some(job.clone())
                                } else {
                                    None
                                }
                            };

                            // Persist after releasing the lock
                            if updated.is_some() {
                                if let Err(e) = store.persist().await {
                                    tracing::error!("Failed to persist cron jobs: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    /// Stop the scheduler
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        tracing::info!("Cron scheduler stop signal sent");
    }
}

// Global scheduler instance
static CRON_SCHEDULER: std::sync::OnceLock<Arc<CronScheduler>> = std::sync::OnceLock::new();

/// Initialize and start the cron scheduler
pub async fn init_cron_scheduler(
    store: Arc<CronStore>,
    gateway: Arc<GatewayManager>,
) -> Result<()> {
    let scheduler = Arc::new(CronScheduler::new(store, gateway));
    scheduler.start().await;
    let _ = CRON_SCHEDULER.set(scheduler);
    Ok(())
}

/// Stop the cron scheduler
pub async fn stop_cron_scheduler() {
    if let Some(scheduler) = CRON_SCHEDULER.get() {
        scheduler.stop().await;
    }
}