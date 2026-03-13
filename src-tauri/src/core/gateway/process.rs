//! Gateway process spawning and lifecycle management
//!
//! This module handles spawning the OpenClaw Gateway subprocess using Node.js.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Default Gateway port
pub const DEFAULT_GATEWAY_PORT: u16 = 18789;

/// Gateway process lifecycle state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayProcessState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error(String),
}

/// Configuration for launching the Gateway process
#[derive(Debug, Clone)]
pub struct GatewayLaunchConfig {
    /// Port for Gateway to listen on
    pub port: u16,
    /// Gateway authentication token
    pub token: String,
    /// Whether to skip channel startup
    pub skip_channels: bool,
    /// Additional environment variables
    pub env: Vec<(String, String)>,
}

impl Default for GatewayLaunchConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_GATEWAY_PORT,
            token: String::new(),
            skip_channels: false,
            env: Vec::new(),
        }
    }
}

/// Gateway process handle
pub struct GatewayProcess {
    /// Child process handle
    child: Option<Child>,
    /// Process state
    state: GatewayProcessState,
    /// PID of the running process
    pid: Option<u32>,
    /// Launch configuration
    config: GatewayLaunchConfig,
}

impl GatewayProcess {
    /// Create a new Gateway process manager
    pub fn new(config: GatewayLaunchConfig) -> Self {
        Self {
            child: None,
            state: GatewayProcessState::Stopped,
            pid: None,
            config,
        }
    }

    /// Get current process state
    pub fn state(&self) -> &GatewayProcessState {
        &self.state
    }

    /// Get process PID
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Start the Gateway process
    pub fn start(&mut self) -> Result<()> {
        if self.child.is_some() {
            tracing::warn!("Gateway process already running");
            return Ok(());
        }

        self.state = GatewayProcessState::Starting;

        let openclaw_dir = get_openclaw_dir()?;
        let entry_script = openclaw_dir.join("openclaw.mjs");

        if !entry_script.exists() {
            let err_msg = format!("OpenClaw entry script not found at: {}", entry_script.display());
            self.state = GatewayProcessState::Error(err_msg.clone());
            anyhow::bail!("{}", err_msg);
        }

        // Build arguments
        let mut args = vec![
            "gateway".to_string(),
            "--port".to_string(),
            self.config.port.to_string(),
            "--token".to_string(),
            self.config.token.clone(),
            "--allow-unconfigured".to_string(),
        ];

        if self.config.skip_channels {
            args.push("--skip-channels".to_string());
        }

        tracing::info!(
            "Starting Gateway process: node {} {} (cwd={})",
            entry_script.display(),
            args.join(" "),
            openclaw_dir.display()
        );

        // Find node executable
        let node_exe = if cfg!(target_os = "windows") {
            "node.exe"
        } else {
            "node"
        };

        // Build environment
        let mut cmd = Command::new(node_exe);
        cmd.arg(&entry_script)
            .args(&args)
            .current_dir(&openclaw_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        // Add required environment variables
        cmd.env("OPENCLAW_NO_RESPAWN", "1");
        cmd.env("OPENCLAW_GATEWAY_TOKEN", &self.config.token);

        if self.config.skip_channels {
            cmd.env("OPENCLAW_SKIP_CHANNELS", "1");
            cmd.env("CLAWDBOT_SKIP_CHANNELS", "1");
        }

        // Spawn the process
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Failed to spawn Gateway process: {}. Make sure Node.js is installed and in PATH.", e);
                self.state = GatewayProcessState::Error(err_msg.clone());
                anyhow::bail!("{}", err_msg);
            }
        };

        let pid = child.id();
        self.pid = Some(pid);
        tracing::info!("Gateway process started (pid={})", pid);

        // Read stderr in a background thread (not tokio task, to avoid async issues)
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            let level = classify_stderr_line(&l);
                            match level {
                                StderrLevel::Debug => tracing::debug!("[Gateway stderr] {}", l),
                                StderrLevel::Warn => tracing::warn!("[Gateway stderr] {}", l),
                                StderrLevel::Drop => {}
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Error reading Gateway stderr: {}", e);
                            break;
                        }
                    }
                }
            });
        }

        // Read stdout in a background thread
        let stdout = child.stdout.take();
        if let Some(stdout) = stdout {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(l) => tracing::debug!("[Gateway stdout] {}", l),
                        Err(e) => {
                            tracing::debug!("Error reading Gateway stdout: {}", e);
                            break;
                        }
                    }
                }
            });
        }

        self.child = Some(child);
        self.state = GatewayProcessState::Running;

        Ok(())
    }

    /// Stop the Gateway process
    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            self.state = GatewayProcessState::Stopping;
            tracing::info!("Stopping Gateway process (pid={})", child.id());

            // Try graceful shutdown first
            #[cfg(unix)]
            {
                use std::io::Write;
                let _ = child.stdin.as_mut().map(|stdin| stdin.write_all(b"shutdown\n"));
            }

            // Give it a moment to shutdown gracefully
            std::thread::sleep(Duration::from_millis(500));

            // Check if it's still running
            match child.try_wait() {
                Ok(Some(status)) => {
                    tracing::info!("Gateway process exited with status: {}", status);
                }
                Ok(None) => {
                    // Still running, force kill
                    tracing::warn!("Gateway process didn't exit gracefully, killing...");
                    child.kill().context("Failed to kill Gateway process")?;
                }
                Err(e) => {
                    tracing::warn!("Error checking Gateway process status: {}", e);
                    let _ = child.kill();
                }
            }
        }

        self.pid = None;
        self.state = GatewayProcessState::Stopped;
        Ok(())
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    tracing::info!("Gateway process exited with status: {}", status);
                    self.child = None;
                    self.pid = None;
                    self.state = GatewayProcessState::Stopped;
                    false
                }
                Ok(None) => true, // Still running
                Err(e) => {
                    tracing::warn!("Error checking Gateway process status: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Get the exit code if the process has exited
    pub fn exit_code(&mut self) -> Option<i32> {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => status.code(),
                _ => None,
            }
        } else {
            None
        }
    }
}

impl Drop for GatewayProcess {
    fn drop(&mut self) {
        if self.child.is_some() {
            let _ = self.stop();
        }
    }
}

/// Severity level for stderr lines
enum StderrLevel {
    Debug,
    Warn,
    Drop,
}

/// Classify a stderr line to determine how to log it
fn classify_stderr_line(line: &str) -> StderrLevel {
    let line_lower = line.to_lowercase();

    // Drop noisy debug messages
    if line_lower.contains("debug:") || line_lower.contains("[debug]") {
        return StderrLevel::Debug;
    }

    // Warn for errors and warnings
    if line_lower.contains("error") || line_lower.contains("warn") || line_lower.contains("fail") {
        return StderrLevel::Warn;
    }

    // Default to debug
    StderrLevel::Debug
}

/// Get the OpenClaw package directory
pub fn get_openclaw_dir() -> Result<PathBuf> {
    // Try multiple locations
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

    tracing::debug!("Searching for OpenClaw in {} locations", candidates.len());
    for path in &candidates {
        tracing::debug!("  Checking: {}", path.display());
        if path.exists() && path.join("openclaw.mjs").exists() {
            tracing::info!("Found OpenClaw at: {}", path.display());
            return Ok(path.clone());
        }
    }

    let searched = candidates.iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
    anyhow::bail!(
        "OpenClaw package not found. Searched locations:\n  {}\nPlease ensure openclaw is installed in node_modules.",
        searched
    );
}

/// Get the OpenClaw entry script path
pub fn get_openclaw_entry_path() -> Result<PathBuf> {
    Ok(get_openclaw_dir()?.join("openclaw.mjs"))
}

/// Check if OpenClaw package exists
pub fn is_openclaw_present() -> bool {
    get_openclaw_dir().map(|dir| dir.join("openclaw.mjs").exists()).unwrap_or(false)
}

/// Check if a Gateway process is already running on the given port
pub fn is_gateway_running_on_port(port: u16) -> bool {
    // Try to connect to the port to check if something is listening
    use std::net::TcpStream;

    let addr = format!("127.0.0.1:{}", port);
    TcpStream::connect(&addr).is_ok()
}

/// Find and kill any existing Gateway process on the given port
pub fn kill_gateway_on_port(port: u16) -> Result<bool> {
    if !is_gateway_running_on_port(port) {
        return Ok(false);
    }

    tracing::info!("Found existing Gateway on port {}, attempting to stop it...", port);

    // On Windows, use netstat to find the PID
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output()
            .context("Failed to run netstat")?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let port_str = format!(":{}", port);

        for line in output_str.lines() {
            if line.contains(&port_str) && line.contains("LISTENING") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pid_str) = parts.last() {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        tracing::info!("Killing Gateway process (pid={})", pid);

                        let kill_result = Command::new("taskkill")
                            .args(["/F", "/PID", &pid.to_string()])
                            .output();

                        match kill_result {
                            Ok(_) => {
                                tracing::info!("Successfully killed Gateway process (pid={})", pid);
                                // Wait a moment for the process to fully terminate
                                std::thread::sleep(Duration::from_millis(500));
                                return Ok(true);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to kill Gateway process (pid={}): {}", pid, e);
                            }
                        }
                    }
                }
            }
        }
    }

    // On Unix systems, use lsof
    #[cfg(unix)]
    {
        let output = Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output();

        if let Ok(output) = output {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid_str in pids.lines() {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    tracing::info!("Killing Gateway process (pid={})", pid);

                    let kill_result = Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output();

                    if kill_result.is_ok() {
                        tracing::info!("Successfully killed Gateway process (pid={})", pid);
                        std::thread::sleep(Duration::from_millis(500));
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_stderr_line() {
        assert!(matches!(classify_stderr_line("Error: something"), StderrLevel::Warn));
        assert!(matches!(classify_stderr_line("warn: test"), StderrLevel::Warn));
        assert!(matches!(classify_stderr_line("debug: test"), StderrLevel::Debug));
        assert!(matches!(classify_stderr_line("normal log"), StderrLevel::Debug));
    }
}