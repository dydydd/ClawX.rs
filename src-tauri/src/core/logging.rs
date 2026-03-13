//! Logging utilities
//!
//! Centralized logging with file output, log rotation, and retrieval for UI.

use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl Default for LogLevel {
    fn default() -> Self {
        #[cfg(debug_assertions)]
        {
            LogLevel::Debug
        }
        #[cfg(not(debug_assertions))]
        {
            LogLevel::Info
        }
    }
}

/// Log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
}

/// Log file info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: String,
}

/// Logger instance
pub struct Logger {
    /// Log directory
    log_dir: PathBuf,
    /// Current log file path
    current_file: Arc<RwLock<PathBuf>>,
    /// In-memory ring buffer
    ring_buffer: Arc<Mutex<Vec<String>>>,
    /// Ring buffer size
    ring_buffer_size: usize,
    /// Current log level
    level: Arc<RwLock<LogLevel>>,
}

impl Logger {
    /// Create a new logger
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        // Ensure log directory exists
        if !log_dir.exists() {
            fs::create_dir_all(&log_dir)
                .with_context(|| format!("Failed to create log directory: {}", log_dir.display()))?;
        }

        // Determine current log file
        let today = Local::now().format("%Y-%m-%d").to_string();
        let current_file = log_dir.join(format!("clawx-{}.log", today));

        // Write session header
        let header = format!(
            "\n{}\n[{}] === ClawX Session Start ===\n{}\n",
            "=".repeat(80),
            Utc::now().to_rfc3339(),
            "=".repeat(80)
        );

        {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&current_file)
                .context("Failed to open log file")?;
            file.write_all(header.as_bytes())
                .context("Failed to write session header")?;
        }

        Ok(Self {
            log_dir,
            current_file: Arc::new(RwLock::new(current_file)),
            ring_buffer: Arc::new(Mutex::new(Vec::with_capacity(500))),
            ring_buffer_size: 500,
            level: Arc::new(RwLock::new(LogLevel::default())),
        })
    }

    /// Get log directory
    pub fn get_log_dir(&self) -> &Path {
        &self.log_dir
    }

    /// Get current log file path
    pub fn get_current_file_path(&self) -> PathBuf {
        self.current_file.try_read().map(|f| f.clone()).unwrap_or_else(|_| self.log_dir.join("clawx.log"))
    }

    /// Set log level
    pub async fn set_level(&self, level: LogLevel) {
        *self.level.write().await = level;
    }

    /// Get current log level
    pub async fn get_level(&self) -> LogLevel {
        *self.level.read().await
    }

    /// Format a log message
    fn format_message(level: &str, message: &str) -> String {
        let timestamp = Utc::now().to_rfc3339();
        format!("[{}] [{}] {}", timestamp, level.pad_to_width(5), message)
    }

    /// Write log to buffer and file
    async fn write_log(&self, formatted: String) {
        // Add to ring buffer
        {
            let mut buffer = self.ring_buffer.lock().unwrap();
            buffer.push(formatted.clone());
            if buffer.len() > self.ring_buffer_size {
                buffer.remove(0);
            }
        }

        // Write to file
        let current_file = self.current_file.read().await.clone();
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current_file)
        {
            let _ = writeln!(file, "{}", formatted);
        }
    }

    /// Log a debug message
    pub async fn debug(&self, message: &str) {
        if *self.level.read().await <= LogLevel::Debug {
            let formatted = Self::format_message("DEBUG", message);
            tracing::debug!("{}", message);
            self.write_log(formatted).await;
        }
    }

    /// Log an info message
    pub async fn info(&self, message: &str) {
        if *self.level.read().await <= LogLevel::Info {
            let formatted = Self::format_message("INFO", message);
            tracing::info!("{}", message);
            self.write_log(formatted).await;
        }
    }

    /// Log a warning message
    pub async fn warn(&self, message: &str) {
        if *self.level.read().await <= LogLevel::Warn {
            let formatted = Self::format_message("WARN", message);
            tracing::warn!("{}", message);
            self.write_log(formatted).await;
        }
    }

    /// Log an error message
    pub async fn error(&self, message: &str) {
        if *self.level.read().await <= LogLevel::Error {
            let formatted = Self::format_message("ERROR", message);
            tracing::error!("{}", message);
            self.write_log(formatted).await;
        }
    }

    /// Get recent logs from ring buffer
    pub fn get_recent_logs(&self, count: Option<usize>, min_level: Option<LogLevel>) -> Vec<String> {
        let buffer = self.ring_buffer.lock().unwrap();

        let filtered: Vec<String> = if let Some(level) = min_level {
            buffer
                .iter()
                .filter(|line| {
                    match level {
                        LogLevel::Debug => true,
                        LogLevel::Info => !line.contains("] [DEBUG"),
                        LogLevel::Warn => line.contains("] [WARN") || line.contains("] [ERROR"),
                        LogLevel::Error => line.contains("] [ERROR"),
                    }
                })
                .cloned()
                .collect()
        } else {
            buffer.clone()
        };

        if let Some(n) = count {
            filtered.into_iter().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect()
        } else {
            filtered
        }
    }

    /// Read log file content
    pub async fn read_log_file(&self, tail_lines: usize) -> Result<String> {
        let current_file = self.current_file.read().await.clone();

        if !current_file.exists() {
            return Ok("(No log file found)".to_string());
        }

        let content = tokio::fs::read_to_string(&current_file)
            .await
            .context("Failed to read log file")?;

        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= tail_lines {
            Ok(content)
        } else {
            Ok(lines
                .into_iter()
                .rev()
                .take(tail_lines)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }

    /// List available log files
    pub async fn list_log_files(&self) -> Result<Vec<LogFileInfo>> {
        let mut results = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.log_dir)
            .await
            .context("Failed to read log directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "log").unwrap_or(false) {
                let metadata = entry.metadata().await?;
                let modified = metadata.modified()?;
                let modified_str: String = chrono::DateTime::<Utc>::from(modified).to_rfc3339();

                results.push(LogFileInfo {
                    name: path.file_name().unwrap().to_string_lossy().to_string(),
                    path: path.display().to_string(),
                    size: metadata.len(),
                    modified: modified_str,
                });
            }
        }

        // Sort by modified date descending
        results.sort_by(|a, b| b.modified.cmp(&a.modified));

        Ok(results)
    }

    /// Check if today's log file needs to be rotated (new day)
    pub async fn rotate_if_needed(&self) -> Result<()> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let expected_file = self.log_dir.join(format!("clawx-{}.log", today));

        let mut current_file = self.current_file.write().await;

        if *current_file != expected_file {
            *current_file = expected_file;

            // Write session header
            let header = format!(
                "\n{}\n[{}] === ClawX Session Start ===\n{}\n",
                "=".repeat(80),
                Utc::now().to_rfc3339(),
                "=".repeat(80)
            );

            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&*current_file)
            {
                let _ = file.write_all(header.as_bytes());
            }
        }

        Ok(())
    }
}

/// Initialize logger with default log directory
pub fn init_logger() -> Result<Arc<Logger>> {
    // Get platform data directory
    let data_dir = dirs::data_local_dir()
        .context("Failed to get data directory")?
        .join("ClawX");

    let log_dir = data_dir.join("logs");

    let logger = Logger::new(log_dir)?;

    // Configure tracing_subscriber to write to the log file
    let log_file = logger.get_current_file_path();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .context("Failed to open log file for tracing")?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .init();

    Ok(Arc::new(logger))
}

trait PadToWidth {
    fn pad_to_width(&self, width: usize) -> String;
}

impl PadToWidth for str {
    fn pad_to_width(&self, width: usize) -> String {
        format!("{:width$}", self, width = width)
    }
}