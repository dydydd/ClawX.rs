//! Cron job management IPC command handlers

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub enabled: bool,
}

/// List all cron jobs
#[tauri::command]
pub async fn list_cron_jobs() -> Result<Vec<CronJob>, String> {
    // TODO: Read from cron store
    Err("Not implemented yet".to_string())
}

/// Create a cron job
#[tauri::command]
pub async fn create_cron_job(job: CronJob) -> Result<(), String> {
    // TODO: Create cron job
    Err("Not implemented yet".to_string())
}

/// Delete a cron job
#[tauri::command]
pub async fn delete_cron_job(id: String) -> Result<(), String> {
    // TODO: Delete cron job
    Err("Not implemented yet".to_string())
}