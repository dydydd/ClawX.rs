//! Node.js runtime check command handlers

use serde::{Deserialize, Serialize};
use std::process::Command;

/// Node.js version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeJsInfo {
    /// Whether Node.js is installed
    pub installed: bool,
    /// Node.js version (e.g., "v20.10.0")
    pub version: Option<String>,
    /// Path to node executable
    pub path: Option<String>,
    /// Error message if check failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Check if Node.js is installed and get version info
#[tauri::command]
pub async fn check_nodejs() -> Result<NodeJsInfo, String> {
    // Determine the Node.js executable name based on OS
    let node_exe = if cfg!(target_os = "windows") {
        "node.exe"
    } else {
        "node"
    };

    // Try to run `node --version`
    let output = Command::new(node_exe)
        .arg("--version")
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                // Parse version from stdout
                let version = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();

                // Try to get the path to node
                let path = get_node_path();

                Ok(NodeJsInfo {
                    installed: true,
                    version: Some(version),
                    path,
                    error: None,
                })
            } else {
                // Node.js exists but --version failed
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                Ok(NodeJsInfo {
                    installed: false,
                    version: None,
                    path: None,
                    error: Some(format!("Node.js check failed: {}", stderr)),
                })
            }
        }
        Err(e) => {
            // Node.js not found or cannot be executed
            let error_msg = if std::io::ErrorKind::NotFound == e.kind() {
                "Node.js is not installed or not in PATH".to_string()
            } else {
                format!("Failed to check Node.js: {}", e)
            };

            Ok(NodeJsInfo {
                installed: false,
                version: None,
                path: None,
                error: Some(error_msg),
            })
        }
    }
}

/// Try to get the path to the Node.js executable
fn get_node_path() -> Option<String> {
    // Try `which node` on Unix or `where node` on Windows
    #[cfg(unix)]
    {
        Command::new("which")
            .arg("node")
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
                } else {
                    None
                }
            })
    }

    #[cfg(windows)]
    {
        Command::new("where")
            .arg("node")
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    // `where` returns multiple lines, take the first one
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
    }

    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Check if Node.js version meets minimum requirement
#[tauri::command]
pub async fn check_nodejs_version(min_version: String) -> Result<bool, String> {
    let info = check_nodejs().await?;

    if !info.installed {
        return Ok(false);
    }

    let version = info.version.ok_or("Node.js version not available")?;

    // Parse version (remove 'v' prefix if present)
    let version_str = version.trim_start_matches('v');

    // Compare versions (simple string comparison for now)
    // This works for major.minor.patch format
    Ok(compare_versions(version_str, &min_version))
}

/// Compare two version strings (returns true if current >= minimum)
fn compare_versions(current: &str, minimum: &str) -> bool {
    let current_parts: Vec<u32> = current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    let minimum_parts: Vec<u32> = minimum
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    // Compare each part
    for i in 0..minimum_parts.len().max(current_parts.len()) {
        let current_part = current_parts.get(i).unwrap_or(&0);
        let minimum_part = minimum_parts.get(i).unwrap_or(&0);

        if current_part > minimum_part {
            return true;
        }
        if current_part < minimum_part {
            return false;
        }
    }

    // Versions are equal
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert!(compare_versions("18.0.0", "16.0.0"));
        assert!(compare_versions("20.10.0", "18.0.0"));
        assert!(compare_versions("18.0.0", "18.0.0"));
        assert!(!compare_versions("16.0.0", "18.0.0"));
        assert!(compare_versions("18.1.0", "18.0.0"));
        assert!(compare_versions("18.0.1", "18.0.0"));
    }
}