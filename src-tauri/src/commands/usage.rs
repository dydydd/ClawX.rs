//! Token usage tracking commands
//!
//! Commands for retrieving token usage history

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncBufReadExt;
use walkdir::WalkDir;

/// Token usage history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageHistoryEntry {
    pub timestamp: String,
    pub session_id: String,
    pub agent_id: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub content: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
}

/// Get recent token usage history
#[tauri::command]
pub async fn get_recent_token_usage(limit: Option<u64>) -> Result<Vec<TokenUsageHistoryEntry>, String> {
    tracing::info!("get_recent_token_usage called, limit={:?}", limit);

    let openclaw_dir = get_openclaw_config_dir()
        .map_err(|e| {
            tracing::error!("Failed to get openclaw config dir: {}", e);
            e.to_string()
        })?;

    tracing::info!("OpenClaw config dir: {:?}", openclaw_dir);

    let agents_dir = openclaw_dir.join("agents");

    if !agents_dir.exists() {
        tracing::warn!("Agents directory does not exist: {:?}", agents_dir);
        return Ok(Vec::new());
    }

    let mut results: Vec<TokenUsageHistoryEntry> = Vec::new();
    let max_entries = limit.unwrap_or(u64::MAX);

    tracing::info!("Scanning for usage entries, max_entries={}", max_entries);

    // Walk through all agent directories
    for entry in WalkDir::new(&agents_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }

        let agent_id = entry.file_name().to_string_lossy().to_string();
        let sessions_dir = entry.path().join("sessions");

        tracing::debug!("Checking agent: {}, sessions_dir: {:?}", agent_id, sessions_dir);

        if !sessions_dir.exists() {
            tracing::debug!("Sessions directory does not exist for agent: {}", agent_id);
            continue;
        }

        // List all files in sessions directory for debugging
        match std::fs::read_dir(&sessions_dir) {
            Ok(entries) => {
                let files: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                tracing::debug!("Files in sessions dir: {:?}", files);
            }
            Err(e) => {
                tracing::debug!("Failed to read sessions dir: {}", e);
            }
        }

        // Process session transcript files
        for session_entry in WalkDir::new(&sessions_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if results.len() as u64 >= max_entries {
                break;
            }

            let file_name = session_entry.file_name().to_string_lossy().to_string();
            tracing::debug!("Found file in sessions dir: {}", file_name);

            // Look for transcript JSONL files (including .reset. and .deleted variants)
            // Match files like: uuid.jsonl, uuid.jsonl.reset.xxx, uuid.deleted.jsonl
            let is_transcript = file_name.contains(".jsonl") &&
                (file_name.ends_with(".jsonl") ||
                 file_name.contains(".jsonl.reset.") ||
                 file_name.ends_with(".deleted.jsonl"));

            if !is_transcript {
                tracing::debug!("Skipping non-transcript file: {}", file_name);
                continue;
            }

            tracing::debug!("Processing transcript file: {}", file_name);

            // Extract session ID from filename (handle .reset. and .deleted variants)
            let session_id = {
                let name = file_name.as_str();
                // Remove .deleted.jsonl suffix
                let name = if name.ends_with(".deleted.jsonl") {
                    &name[..name.len() - ".deleted.jsonl".len()]
                } else {
                    name
                };
                // Remove .jsonl.reset.xxx suffix
                let name = if let Some(idx) = name.find(".jsonl.reset.") {
                    &name[..idx]
                } else {
                    name
                };
                // Remove .jsonl suffix
                let name = name.trim_end_matches(".jsonl");
                name.to_string()
            };

            tracing::debug!("Extracted session_id: {} from file: {}", session_id, file_name);

            tracing::debug!("Processing transcript file: {:?}, session_id: {}", session_entry.path(), session_id);

            // Parse usage entries from file
            match parse_usage_entries_from_file(
                session_entry.path(),
                &session_id,
                &agent_id,
            ).await {
                Ok(entries) => {
                    tracing::debug!("Found {} entries in file", entries.len());
                    results.extend(entries);
                }
                Err(e) => {
                    tracing::debug!("Failed to parse file {:?}: {}", session_entry.path(), e);
                }
            }
        }
    }

    tracing::info!("Found {} total usage entries", results.len());

    // Sort by timestamp (newest first)
    results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Limit results
    if let Some(limit) = limit {
        results.truncate(limit as usize);
    }

    tracing::info!("Returning {} usage entries", results.len());

    Ok(results)
}

/// Parse usage entries from a transcript JSONL file
async fn parse_usage_entries_from_file(
    file_path: &std::path::Path,
    session_id: &str,
    agent_id: &str,
) -> Result<Vec<TokenUsageHistoryEntry>> {
    let file = fs::File::open(file_path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut all_lines = Vec::new();

    // Read all lines
    while let Some(line) = lines.next_line().await? {
        all_lines.push(line);
    }

    let mut entries = Vec::new();

    // Process lines in reverse order (newest first, like Electron version)
    for line in all_lines.iter().rev() {
        if let Ok(entry) = parse_usage_entry_from_line(line, session_id, agent_id) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Parse a single usage entry from a JSONL line
fn parse_usage_entry_from_line(
    line: &str,
    session_id: &str,
    agent_id: &str,
) -> Result<TokenUsageHistoryEntry> {
    let json: serde_json::Value = serde_json::from_str(line)?;

    // Get message and timestamp
    let message = json.get("message")
        .and_then(|m| m.as_object())
        .ok_or_else(|| anyhow::anyhow!("No message field"))?;

    let timestamp = json.get("timestamp")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow::anyhow!("No timestamp field"))?
        .to_string();

    let role = message.get("role")
        .and_then(|r| r.as_str())
        .unwrap_or("");

    // Parse usage based on message type
    let (usage, model, provider, content) = if role == "assistant" {
        // Assistant message with usage field
        let usage_obj = message.get("usage")
            .and_then(|u| u.as_object())
            .ok_or_else(|| anyhow::anyhow!("No usage field in assistant message"))?;

        let model = message.get("model")
            .and_then(|m| m.as_str())
            .or_else(|| message.get("modelRef").and_then(|m| m.as_str()))
            .map(|s| s.to_string());

        let provider = message.get("provider")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());

        let content = extract_content(message);

        (usage_obj, model, provider, content)
    } else if role == "toolResult" {
        // Tool result message with details.usage
        let details = message.get("details")
            .and_then(|d| d.as_object())
            .ok_or_else(|| anyhow::anyhow!("No details in toolResult"))?;

        let usage_obj = details.get("usage")
            .and_then(|u| u.as_object())
            .ok_or_else(|| anyhow::anyhow!("No usage in details"))?;

        let model = details.get("model")
            .and_then(|m| m.as_str())
            .or_else(|| message.get("model").and_then(|m| m.as_str()))
            .or_else(|| message.get("modelRef").and_then(|m| m.as_str()))
            .map(|s| s.to_string());

        let provider = details.get("provider")
            .and_then(|p| p.as_str())
            .or_else(|| details.get("externalContent").and_then(|e| e.get("provider")).and_then(|p| p.as_str()))
            .or_else(|| message.get("provider").and_then(|p| p.as_str()))
            .map(|s| s.to_string());

        let content = extract_content(details)
            .or_else(|| extract_content(message));

        if model.is_none() && provider.is_none() {
            return Err(anyhow::anyhow!("No model or provider in toolResult"));
        }

        (usage_obj, model, provider, content)
    } else {
        return Err(anyhow::anyhow!("Unsupported message role: {}", role));
    };

    // Extract token counts from usage object
    let input_tokens = usage.get("input")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("promptTokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);

    let output_tokens = usage.get("output")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("completionTokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);

    let cache_read_tokens = usage.get("cacheRead")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let cache_write_tokens = usage.get("cacheWrite")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let total_tokens = usage.get("total")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("totalTokens").and_then(|v| v.as_u64()))
        .unwrap_or_else(|| input_tokens + output_tokens + cache_read_tokens + cache_write_tokens);

    // Skip if no tokens
    if total_tokens == 0 {
        return Err(anyhow::anyhow!("No tokens in usage"));
    }

    // Get cost
    let cost_usd = usage.get("cost")
        .and_then(|c| c.get("total"))
        .and_then(|v| v.as_f64());

    Ok(TokenUsageHistoryEntry {
        timestamp,
        session_id: session_id.to_string(),
        agent_id: agent_id.to_string(),
        model,
        provider,
        content,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        total_tokens,
        cost_usd,
    })
}

/// Extract content from message object
fn extract_content(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let content = obj.get("content")?;

    if let Some(text) = content.as_str() {
        let trimmed = text.trim();
        return if trimmed.len() > 0 {
            Some(trimmed.to_string())
        } else {
            None
        };
    }

    if let Some(arr) = content.as_array() {
        let chunks: Vec<String> = arr.iter()
            .filter_map(|item| {
                if let Some(text) = item.as_str() {
                    let trimmed = text.trim();
                    return if trimmed.len() > 0 {
                        Some(trimmed.to_string())
                    } else {
                        None
                    };
                }
                if let Some(obj) = item.as_object() {
                    if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                        let trimmed = text.trim();
                        return if trimmed.len() > 0 {
                            Some(trimmed.to_string())
                        } else {
                            None
                        };
                    }
                }
                None
            })
            .collect();
        if chunks.len() > 0 {
            return Some(chunks.join("\n\n"));
        }
    }

    if let Some(obj) = content.as_object() {
        if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
            let trimmed = text.trim();
            return if trimmed.len() > 0 {
                Some(trimmed.to_string())
            } else {
                None
            };
        }
        if let Some(thinking) = obj.get("thinking").and_then(|t| t.as_str()) {
            let trimmed = thinking.trim();
            return if trimmed.len() > 0 {
                Some(trimmed.to_string())
            } else {
                None
            };
        }
    }

    None
}

/// Get OpenClaw config directory
fn get_openclaw_config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    Ok(home.join(".openclaw"))
}