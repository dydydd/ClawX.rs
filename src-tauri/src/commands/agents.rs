//! Agent management IPC command handlers
//!
//! Provides commands for managing agents by calling Gateway RPC methods.
//! Gateway WebSocket RPC methods: agents.list, agents.create, agents.update, agents.delete

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::core::AppState;

/// Agent identity information from Gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub name: Option<String>,
    pub theme: Option<String>,
    pub emoji: Option<String>,
    pub avatar: Option<String>,
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
}

/// Agent information from Gateway agents.list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayAgent {
    pub id: String,
    pub name: Option<String>,
    pub identity: Option<AgentIdentity>,
}

/// Gateway agents.list raw response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayAgentsResponse {
    #[serde(rename = "defaultId")]
    pub default_id: String,
    #[serde(rename = "mainKey")]
    pub main_key: String,
    pub scope: String,
    pub agents: Vec<GatewayAgent>,
}

/// Agent summary information (frontend-facing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
    #[serde(rename = "modelDisplay")]
    pub model_display: String,
    #[serde(rename = "inheritedModel")]
    pub inherited_model: bool,
    pub workspace: String,
    #[serde(rename = "agentDir")]
    pub agent_dir: String,
    #[serde(rename = "mainSessionKey")]
    pub main_session_key: String,
    #[serde(rename = "channelTypes")]
    pub channel_types: Vec<String>,
}

/// Agents snapshot response (frontend-facing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsSnapshot {
    pub agents: Vec<AgentSummary>,
    #[serde(rename = "defaultAgentId")]
    pub default_agent_id: String,
    #[serde(rename = "configuredChannelTypes")]
    pub configured_channel_types: Vec<String>,
    #[serde(rename = "channelOwners")]
    pub channel_owners: std::collections::HashMap<String, String>,
}

/// Convert Gateway agent response to frontend AgentSummary
fn convert_gateway_agent(agent: &GatewayAgent, default_id: &str) -> AgentSummary {
    // Get name from agent.name or fall back to identity.name, then to agent id
    let name = agent.name.clone()
        .or_else(|| agent.identity.as_ref().and_then(|i| i.name.clone()))
        .unwrap_or_else(|| agent.id.clone());

    AgentSummary {
        id: agent.id.clone(),
        name,
        is_default: agent.id == default_id,
        model_display: String::new(), // Not provided by Gateway, will be empty
        inherited_model: true,        // Default to inherited
        workspace: String::new(),     // Not provided by Gateway
        agent_dir: String::new(),     // Not provided by Gateway
        main_session_key: format!("agent:{}:main", agent.id),
        channel_types: Vec::new(),    // Not provided by Gateway, will be empty
    }
}

/// Convert Gateway response to frontend AgentsSnapshot
fn convert_gateway_response(response: GatewayAgentsResponse) -> AgentsSnapshot {
    let agents: Vec<AgentSummary> = response.agents
        .iter()
        .map(|a| convert_gateway_agent(a, &response.default_id))
        .collect();

    AgentsSnapshot {
        agents,
        default_agent_id: response.default_id,
        configured_channel_types: Vec::new(), // Not provided by Gateway
        channel_owners: std::collections::HashMap::new(), // Not provided by Gateway
    }
}

/// Input for creating an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentInput {
    pub name: String,
}

/// Input for updating an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentInput {
    pub name: String,
}

/// Helper function to fetch agents list from Gateway and convert to snapshot
async fn fetch_agents_snapshot(state: &State<'_, Arc<AppState>>) -> Result<AgentsSnapshot, String> {
    let result = state
        .gateway
        .rpc("agents.list", None, 30000)
        .await
        .map_err(|e| e.to_string())?;

    let gateway_response: GatewayAgentsResponse = serde_json::from_value(result)
        .map_err(|e| format!("Failed to parse agents list: {}", e))?;

    Ok(convert_gateway_response(gateway_response))
}

/// List all agents
#[tauri::command]
pub async fn list_agents(
    state: State<'_, Arc<AppState>>,
) -> Result<AgentsSnapshot, String> {
    let result = state
        .gateway
        .rpc("agents.list", None, 30000)
        .await
        .map_err(|e| e.to_string())?;

    // Parse Gateway response and convert to frontend format
    let gateway_response: GatewayAgentsResponse = serde_json::from_value(result)
        .map_err(|e| format!("Failed to parse agents list: {}", e))?;

    let snapshot = convert_gateway_response(gateway_response);
    Ok(snapshot)
}

/// Create a new agent
#[tauri::command]
pub async fn create_agent(
    state: State<'_, Arc<AppState>>,
    input: CreateAgentInput,
) -> Result<AgentsSnapshot, String> {
    // Get the OpenClaw workspace directory (~/.openclaw)
    let workspace = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory".to_string())?
        .join(".openclaw")
        .display()
        .to_string();

    let params = serde_json::json!({
        "name": input.name,
        "workspace": workspace,
    });

    let result = state
        .gateway
        .rpc("agents.create", Some(params), 30000)
        .await
        .map_err(|e| e.to_string())?;

    // Verify the operation succeeded (result should have ok: true)
    let success = result.get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        let error_msg = result.get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Failed to create agent: {}", error_msg));
    }

    // Fetch updated agents list
    fetch_agents_snapshot(&state).await
}

/// Update an agent
#[tauri::command(rename_all = "camelCase")]
pub async fn update_agent(
    state: State<'_, Arc<AppState>>,
    agent_id: String,
    input: UpdateAgentInput,
) -> Result<AgentsSnapshot, String> {
    let params = serde_json::json!({
        "agentId": agent_id,
        "name": input.name,
    });

    let result = state
        .gateway
        .rpc("agents.update", Some(params), 30000)
        .await
        .map_err(|e| e.to_string())?;

    // Verify the operation succeeded
    let success = result.get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        let error_msg = result.get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Failed to update agent: {}", error_msg));
    }

    // Fetch updated agents list
    fetch_agents_snapshot(&state).await
}

/// Delete an agent
#[tauri::command(rename_all = "camelCase")]
pub async fn delete_agent(
    state: State<'_, Arc<AppState>>,
    agent_id: String,
) -> Result<AgentsSnapshot, String> {
    let params = serde_json::json!({
        "agentId": agent_id,
    });

    let result = state
        .gateway
        .rpc("agents.delete", Some(params.clone()), 30000)
        .await
        .map_err(|e| e.to_string())?;

    // Verify the operation succeeded
    let success = result.get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        let error_msg = result.get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Failed to delete agent: {}", error_msg));
    }

    // Fetch updated agents list
    fetch_agents_snapshot(&state).await
}

/// Assign a channel to an agent
#[tauri::command(rename_all = "camelCase")]
pub async fn agent_assign_channel(
    state: State<'_, Arc<AppState>>,
    agent_id: String,
    channel_type: String,
) -> Result<AgentsSnapshot, String> {
    // First get the current agents list
    let snapshot = fetch_agents_snapshot(&state).await?;

    let agent = snapshot
        .agents
        .iter()
        .find(|a| a.id == agent_id)
        .cloned()
        .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

    // Add the channel type to the agent's channel types if not already present
    let mut channel_types = agent.channel_types.clone();
    if !channel_types.contains(&channel_type) {
        channel_types.push(channel_type.clone());
    }

    let params = serde_json::json!({
        "agentId": agent_id,
        "channelTypes": channel_types,
    });

    let result = state
        .gateway
        .rpc("agents.update", Some(params), 30000)
        .await
        .map_err(|e| e.to_string())?;

    // Verify the operation succeeded
    let success = result.get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        let error_msg = result.get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Failed to assign channel: {}", error_msg));
    }

    // Fetch updated agents list
    fetch_agents_snapshot(&state).await
}

/// Remove a channel from an agent
#[tauri::command(rename_all = "camelCase")]
pub async fn agent_remove_channel(
    state: State<'_, Arc<AppState>>,
    agent_id: String,
    channel_type: String,
) -> Result<AgentsSnapshot, String> {
    // First get the current agents list
    let snapshot = fetch_agents_snapshot(&state).await?;

    let agent = snapshot
        .agents
        .iter()
        .find(|a| a.id == agent_id)
        .cloned()
        .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

    // Remove the channel type from the agent's channel types
    let channel_types: Vec<String> = agent
        .channel_types
        .iter()
        .filter(|&ct| ct != &channel_type)
        .cloned()
        .collect();

    let params = serde_json::json!({
        "agentId": agent_id,
        "channelTypes": channel_types,
    });

    let result = state
        .gateway
        .rpc("agents.update", Some(params), 30000)
        .await
        .map_err(|e| e.to_string())?;

    // Verify the operation succeeded
    let success = result.get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        let error_msg = result.get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Failed to remove channel: {}", error_msg));
    }

    // Fetch updated agents list
    fetch_agents_snapshot(&state).await
}
