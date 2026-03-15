//! OpenClaw status command handlers

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// OpenClaw package status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawStatus {
    /// Whether the package exists
    pub package_exists: bool,
    /// Whether the package is built (has dist folder)
    pub is_built: bool,
    /// Path to the OpenClaw directory
    pub dir: String,
    /// Version from package.json
    pub version: Option<String>,
}

/// OpenClaw CLI command result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawCliCommand {
    /// Whether the command was successfully generated
    pub success: bool,
    /// The CLI command string
    pub command: Option<String>,
    /// Error message if not successful
    pub error: Option<String>,
}

/// Get OpenClaw package status
#[tauri::command]
pub async fn openclaw_status() -> Result<OpenClawStatus, String> {
    // Find OpenClaw directory
    let candidates: Vec<PathBuf> = {
        let mut paths = Vec::new();

        // 1. Current directory / node_modules / openclaw
        if let Ok(cwd) = std::env::current_dir() {
            paths.push(cwd.join("node_modules").join("openclaw"));
        }

        // 2. Parent directory / node_modules / openclaw (in case we're in src-tauri)
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(parent) = cwd.parent() {
                paths.push(parent.join("node_modules").join("openclaw"));
            }
        }

        // 3. Executable directory / resources / openclaw (packaged mode)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                paths.push(exe_dir.join("resources").join("openclaw"));
                // Also try two levels up for dev mode
                if let Some(two_up) = exe_dir.parent().and_then(|p| p.parent()) {
                    paths.push(two_up.join("node_modules").join("openclaw"));
                }
            }
        }

        paths
    };

    // Find the first existing OpenClaw directory
    for path in &candidates {
        if path.exists() && path.join("openclaw.mjs").exists() {
            let package_exists = true;
            let is_built = path.join("dist").exists();

            // Try to read version from package.json
            let version = std::fs::read_to_string(path.join("package.json"))
                .ok()
                .and_then(|content| {
                    serde_json::from_str::<serde_json::Value>(&content).ok()
                })
                .and_then(|json| {
                    json.get("version")?.as_str().map(|s| s.to_string())
                });

            return Ok(OpenClawStatus {
                package_exists,
                is_built,
                dir: path.display().to_string(),
                version,
            });
        }
    }

    // Not found - return first candidate as the expected location
    let default_path = candidates.first()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("node_modules/openclaw"));

    Ok(OpenClawStatus {
        package_exists: false,
        is_built: false,
        dir: default_path.display().to_string(),
        version: None,
    })
}

/// Get OpenClaw skills directory (~/.openclaw/skills)
#[tauri::command]
pub async fn openclaw_get_skills_dir() -> Result<String, String> {
    let skills_dir = dirs::home_dir()
        .ok_or("Could not determine home directory")?
        .join(".openclaw")
        .join("skills");

    // Ensure directory exists
    if !skills_dir.exists() {
        std::fs::create_dir_all(&skills_dir)
            .map_err(|e| format!("Failed to create skills directory: {}", e))?;
    }

    Ok(skills_dir.display().to_string())
}

/// Get OpenClaw CLI command
#[tauri::command]
pub async fn openclaw_get_cli_command() -> Result<OpenClawCliCommand, String> {
    // Check if OpenClaw package exists
    let status = openclaw_status().await?;

    if !status.package_exists {
        return Ok(OpenClawCliCommand {
            success: false,
            command: None,
            error: Some(format!("OpenClaw package not found at: {}", status.dir)),
        });
    }

    // Build the command string
    let entry_path = PathBuf::from(&status.dir).join("openclaw.mjs");
    if !entry_path.exists() {
        return Ok(OpenClawCliCommand {
            success: false,
            command: None,
            error: Some(format!("OpenClaw entry script not found at: {}", entry_path.display())),
        });
    }

    // Simple command: node <entry_path>
    let command = format!("node \"{}\"", entry_path.display());

    Ok(OpenClawCliCommand {
        success: true,
        command: Some(command),
        error: None,
    })
}