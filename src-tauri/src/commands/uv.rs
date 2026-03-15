//! UV package manager and Python setup commands
//!
//! Handles installation and management of uv and managed Python versions.

use serde::{Deserialize, Serialize};
use std::process::Command;

/// Result of the install-all operation
#[derive(Debug, Serialize, Deserialize)]
pub struct InstallAllResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Check if uv is available in PATH
fn find_uv_in_path() -> Option<String> {
    let uv_name = if cfg!(windows) { "uv.exe" } else { "uv" };

    // Try to find uv in PATH
    if let Ok(output) = Command::new("where")
        .arg(uv_name)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            return Some(path.trim().to_string());
        }
    }

    // Fallback for Unix
    if cfg!(not(windows)) {
        if let Ok(output) = Command::new("which")
            .arg("uv")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                return Some(path.trim().to_string());
            }
        }
    }

    None
}

/// Check if a managed Python 3.12 is ready
fn is_python_ready(uv_path: &str) -> bool {
    Command::new(uv_path)
        .args(["python", "find", "3.12"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Install Python 3.12 using uv
fn install_python(uv_path: &str) -> Result<(), String> {
    let output = Command::new(uv_path)
        .args(["python", "install", "3.12"])
        .output()
        .map_err(|e| format!("Failed to execute uv: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.is_empty() {
            stderr.to_string()
        } else if !stdout.is_empty() {
            stdout.to_string()
        } else {
            "Unknown error".to_string()
        };
        Err(format!("Python installation failed: {}", detail))
    }
}

/// Install uv and Python - main entry point for setup
///
/// This command:
/// 1. Checks if uv is available in PATH
/// 2. Installs Python 3.12 using uv
/// 3. Returns the result
#[tauri::command]
pub async fn uv_install_all() -> InstallAllResult {
    tracing::info!("Starting uv and Python installation process");

    // Step 1: Check for uv
    let uv_path = match find_uv_in_path() {
        Some(path) => {
            tracing::info!("Found uv at: {}", path);
            "uv".to_string() // Use PATH lookup
        }
        None => {
            tracing::error!("uv not found in system PATH");
            return InstallAllResult {
                success: false,
                error: Some("uv not found in system PATH. Please install uv first: https://docs.astral.sh/uv/".to_string()),
            };
        }
    };

    // Step 2: Check if Python 3.12 is already installed
    if is_python_ready(&uv_path) {
        tracing::info!("Python 3.12 is already installed and ready");
        return InstallAllResult {
            success: true,
            error: None,
        };
    }

    // Step 3: Install Python 3.12
    tracing::info!("Installing Python 3.12...");
    match install_python(&uv_path) {
        Ok(()) => {
            tracing::info!("Python 3.12 installed successfully");
            InstallAllResult {
                success: true,
                error: None,
            }
        }
        Err(e) => {
            tracing::error!("Failed to install Python: {}", e);
            InstallAllResult {
                success: false,
                error: Some(e),
            }
        }
    }
}

/// Check if uv is installed
#[tauri::command]
pub async fn uv_check_installed() -> bool {
    find_uv_in_path().is_some()
}

/// Check if Python 3.12 is ready
#[tauri::command]
pub async fn uv_check_python_ready() -> bool {
    if let Some(uv_path) = find_uv_in_path() {
        is_python_ready(&uv_path)
    } else {
        false
    }
}