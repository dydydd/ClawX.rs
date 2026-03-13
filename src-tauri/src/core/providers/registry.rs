//! Provider registry - defines all supported provider vendors
//!
//! This module mirrors the Electron provider registry from:
//! electron/shared/providers/registry.ts

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Provider authentication modes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthMode {
    ApiKey,
    OauthDevice,
    OauthBrowser,
    Local,
}

/// Provider protocol types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderProtocol {
    OpenaiCompletions,
    OpenaiResponses,
    AnthropicMessages,
}

/// Provider vendor categories
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderVendorCategory {
    Official,
    Compatible,
    Local,
    Custom,
}

/// Provider model entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModelEntry {
    pub id: String,
    pub name: String,
}

/// Provider backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderBackendConfig {
    pub base_url: String,
    pub api: ProviderProtocol,
    pub api_key_env: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<ProviderModelEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

/// Provider type info (UI-facing)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTypeInfo {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub placeholder: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub requires_api_key: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_base_url: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_model_id: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_model_id_in_dev_mode_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id_placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_oauth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_api_key: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_url: Option<String>,
}

/// Provider vendor definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDefinition {
    #[serde(flatten)]
    pub type_info: ProviderTypeInfo,
    pub category: ProviderVendorCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_config: Option<ProviderBackendConfig>,
    pub supported_auth_modes: Vec<ProviderAuthMode>,
    pub default_auth_mode: ProviderAuthMode,
    pub supports_multiple_accounts: bool,
}

/// All supported provider vendor definitions
pub static PROVIDER_VENDORS: LazyLock<Vec<ProviderDefinition>> = LazyLock::new(|| {
    vec![
        // Anthropic
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "anthropic".to_string(),
                name: "Anthropic".to_string(),
                icon: "🤖".to_string(),
                placeholder: "sk-ant-api03-...".to_string(),
                model: Some("Claude".to_string()),
                requires_api_key: true,
                default_base_url: None,
                show_base_url: None,
                show_model_id: None,
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: None,
                default_model_id: Some("claude-opus-4-6".to_string()),
                is_oauth: None,
                supports_api_key: None,
                api_key_url: None,
            },
            category: ProviderVendorCategory::Official,
            env_var: Some("ANTHROPIC_API_KEY".to_string()),
            provider_config: None,
            supported_auth_modes: vec![ProviderAuthMode::ApiKey],
            default_auth_mode: ProviderAuthMode::ApiKey,
            supports_multiple_accounts: true,
        },
        // OpenAI
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "openai".to_string(),
                name: "OpenAI".to_string(),
                icon: "💚".to_string(),
                placeholder: "sk-proj-...".to_string(),
                model: Some("GPT".to_string()),
                requires_api_key: true,
                default_base_url: None,
                show_base_url: None,
                show_model_id: None,
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: None,
                default_model_id: Some("gpt-5.2".to_string()),
                is_oauth: Some(true),
                supports_api_key: Some(true),
                api_key_url: None,
            },
            category: ProviderVendorCategory::Official,
            env_var: Some("OPENAI_API_KEY".to_string()),
            provider_config: Some(ProviderBackendConfig {
                base_url: "https://api.openai.com/v1".to_string(),
                api: ProviderProtocol::OpenaiResponses,
                api_key_env: "OPENAI_API_KEY".to_string(),
                models: None,
                headers: None,
            }),
            supported_auth_modes: vec![ProviderAuthMode::ApiKey, ProviderAuthMode::OauthBrowser],
            default_auth_mode: ProviderAuthMode::ApiKey,
            supports_multiple_accounts: true,
        },
        // Google
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "google".to_string(),
                name: "Google".to_string(),
                icon: "🔷".to_string(),
                placeholder: "AIza...".to_string(),
                model: Some("Gemini".to_string()),
                requires_api_key: true,
                default_base_url: None,
                show_base_url: None,
                show_model_id: None,
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: None,
                default_model_id: Some("gemini-3.1-pro-preview".to_string()),
                is_oauth: Some(true),
                supports_api_key: Some(true),
                api_key_url: None,
            },
            category: ProviderVendorCategory::Official,
            env_var: Some("GEMINI_API_KEY".to_string()),
            provider_config: None,
            supported_auth_modes: vec![ProviderAuthMode::ApiKey, ProviderAuthMode::OauthBrowser],
            default_auth_mode: ProviderAuthMode::ApiKey,
            supports_multiple_accounts: true,
        },
        // OpenRouter
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "openrouter".to_string(),
                name: "OpenRouter".to_string(),
                icon: "🌐".to_string(),
                placeholder: "sk-or-v1-...".to_string(),
                model: Some("Multi-Model".to_string()),
                requires_api_key: true,
                default_base_url: None,
                show_base_url: None,
                show_model_id: Some(true),
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: Some("anthropic/claude-opus-4.6".to_string()),
                default_model_id: Some("anthropic/claude-opus-4.6".to_string()),
                is_oauth: None,
                supports_api_key: None,
                api_key_url: None,
            },
            category: ProviderVendorCategory::Compatible,
            env_var: Some("OPENROUTER_API_KEY".to_string()),
            provider_config: Some(ProviderBackendConfig {
                base_url: "https://openrouter.ai/api/v1".to_string(),
                api: ProviderProtocol::OpenaiCompletions,
                api_key_env: "OPENROUTER_API_KEY".to_string(),
                models: None,
                headers: Some({
                    let mut h = std::collections::HashMap::new();
                    h.insert("HTTP-Referer".to_string(), "https://claw-x.com".to_string());
                    h.insert("X-Title".to_string(), "ClawX".to_string());
                    h
                }),
            }),
            supported_auth_modes: vec![ProviderAuthMode::ApiKey],
            default_auth_mode: ProviderAuthMode::ApiKey,
            supports_multiple_accounts: true,
        },
        // ByteDance Ark
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "ark".to_string(),
                name: "ByteDance Ark".to_string(),
                icon: "A".to_string(),
                placeholder: "your-ark-api-key".to_string(),
                model: Some("Doubao".to_string()),
                requires_api_key: true,
                default_base_url: Some("https://ark.cn-beijing.volces.com/api/v3".to_string()),
                show_base_url: Some(true),
                show_model_id: Some(true),
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: Some("ep-20260228000000-xxxxx".to_string()),
                default_model_id: None,
                is_oauth: None,
                supports_api_key: None,
                api_key_url: None,
            },
            category: ProviderVendorCategory::Official,
            env_var: Some("ARK_API_KEY".to_string()),
            provider_config: Some(ProviderBackendConfig {
                base_url: "https://ark.cn-beijing.volces.com/api/v3".to_string(),
                api: ProviderProtocol::OpenaiCompletions,
                api_key_env: "ARK_API_KEY".to_string(),
                models: None,
                headers: None,
            }),
            supported_auth_modes: vec![ProviderAuthMode::ApiKey],
            default_auth_mode: ProviderAuthMode::ApiKey,
            supports_multiple_accounts: true,
        },
        // Moonshot (CN)
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "moonshot".to_string(),
                name: "Moonshot (CN)".to_string(),
                icon: "🌙".to_string(),
                placeholder: "sk-...".to_string(),
                model: Some("Kimi".to_string()),
                requires_api_key: true,
                default_base_url: Some("https://api.moonshot.cn/v1".to_string()),
                show_base_url: None,
                show_model_id: None,
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: None,
                default_model_id: Some("kimi-k2.5".to_string()),
                is_oauth: None,
                supports_api_key: None,
                api_key_url: None,
            },
            category: ProviderVendorCategory::Official,
            env_var: Some("MOONSHOT_API_KEY".to_string()),
            provider_config: Some(ProviderBackendConfig {
                base_url: "https://api.moonshot.cn/v1".to_string(),
                api: ProviderProtocol::OpenaiCompletions,
                api_key_env: "MOONSHOT_API_KEY".to_string(),
                models: Some(vec![
                    ProviderModelEntry {
                        id: "kimi-k2.5".to_string(),
                        name: "Kimi K2.5".to_string(),
                    },
                ]),
                headers: None,
            }),
            supported_auth_modes: vec![ProviderAuthMode::ApiKey],
            default_auth_mode: ProviderAuthMode::ApiKey,
            supports_multiple_accounts: true,
        },
        // SiliconFlow (CN)
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "siliconflow".to_string(),
                name: "SiliconFlow (CN)".to_string(),
                icon: "🌊".to_string(),
                placeholder: "sk-...".to_string(),
                model: Some("Multi-Model".to_string()),
                requires_api_key: true,
                default_base_url: Some("https://api.siliconflow.cn/v1".to_string()),
                show_base_url: None,
                show_model_id: Some(true),
                show_model_id_in_dev_mode_only: Some(true),
                model_id_placeholder: Some("deepseek-ai/DeepSeek-V3".to_string()),
                default_model_id: Some("deepseek-ai/DeepSeek-V3".to_string()),
                is_oauth: None,
                supports_api_key: None,
                api_key_url: None,
            },
            category: ProviderVendorCategory::Compatible,
            env_var: Some("SILICONFLOW_API_KEY".to_string()),
            provider_config: Some(ProviderBackendConfig {
                base_url: "https://api.siliconflow.cn/v1".to_string(),
                api: ProviderProtocol::OpenaiCompletions,
                api_key_env: "SILICONFLOW_API_KEY".to_string(),
                models: None,
                headers: None,
            }),
            supported_auth_modes: vec![ProviderAuthMode::ApiKey],
            default_auth_mode: ProviderAuthMode::ApiKey,
            supports_multiple_accounts: true,
        },
        // MiniMax (Global)
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "minimax-portal".to_string(),
                name: "MiniMax (Global)".to_string(),
                icon: "☁️".to_string(),
                placeholder: "sk-...".to_string(),
                model: Some("MiniMax".to_string()),
                requires_api_key: false,
                default_base_url: None,
                show_base_url: None,
                show_model_id: None,
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: None,
                default_model_id: Some("MiniMax-M2.5".to_string()),
                is_oauth: Some(true),
                supports_api_key: Some(true),
                api_key_url: Some("https://intl.minimaxi.com/".to_string()),
            },
            category: ProviderVendorCategory::Official,
            env_var: Some("MINIMAX_API_KEY".to_string()),
            provider_config: Some(ProviderBackendConfig {
                base_url: "https://api.minimax.io/anthropic".to_string(),
                api: ProviderProtocol::AnthropicMessages,
                api_key_env: "MINIMAX_API_KEY".to_string(),
                models: None,
                headers: None,
            }),
            supported_auth_modes: vec![ProviderAuthMode::OauthDevice, ProviderAuthMode::ApiKey],
            default_auth_mode: ProviderAuthMode::OauthDevice,
            supports_multiple_accounts: true,
        },
        // MiniMax (CN)
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "minimax-portal-cn".to_string(),
                name: "MiniMax (CN)".to_string(),
                icon: "☁️".to_string(),
                placeholder: "sk-...".to_string(),
                model: Some("MiniMax".to_string()),
                requires_api_key: false,
                default_base_url: None,
                show_base_url: None,
                show_model_id: None,
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: None,
                default_model_id: Some("MiniMax-M2.5".to_string()),
                is_oauth: Some(true),
                supports_api_key: Some(true),
                api_key_url: Some("https://platform.minimaxi.com/".to_string()),
            },
            category: ProviderVendorCategory::Official,
            env_var: Some("MINIMAX_CN_API_KEY".to_string()),
            provider_config: Some(ProviderBackendConfig {
                base_url: "https://api.minimaxi.com/anthropic".to_string(),
                api: ProviderProtocol::AnthropicMessages,
                api_key_env: "MINIMAX_CN_API_KEY".to_string(),
                models: None,
                headers: None,
            }),
            supported_auth_modes: vec![ProviderAuthMode::OauthDevice, ProviderAuthMode::ApiKey],
            default_auth_mode: ProviderAuthMode::OauthDevice,
            supports_multiple_accounts: true,
        },
        // Qwen
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "qwen-portal".to_string(),
                name: "Qwen".to_string(),
                icon: "☁️".to_string(),
                placeholder: "sk-...".to_string(),
                model: Some("Qwen".to_string()),
                requires_api_key: false,
                default_base_url: None,
                show_base_url: None,
                show_model_id: None,
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: None,
                default_model_id: Some("coder-model".to_string()),
                is_oauth: Some(true),
                supports_api_key: None,
                api_key_url: None,
            },
            category: ProviderVendorCategory::Official,
            env_var: Some("QWEN_API_KEY".to_string()),
            provider_config: Some(ProviderBackendConfig {
                base_url: "https://portal.qwen.ai/v1".to_string(),
                api: ProviderProtocol::OpenaiCompletions,
                api_key_env: "QWEN_API_KEY".to_string(),
                models: None,
                headers: None,
            }),
            supported_auth_modes: vec![ProviderAuthMode::OauthDevice],
            default_auth_mode: ProviderAuthMode::OauthDevice,
            supports_multiple_accounts: true,
        },
        // Ollama
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "ollama".to_string(),
                name: "Ollama".to_string(),
                icon: "🦙".to_string(),
                placeholder: "Not required".to_string(),
                model: None,
                requires_api_key: false,
                default_base_url: Some("http://localhost:11434/v1".to_string()),
                show_base_url: Some(true),
                show_model_id: Some(true),
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: Some("qwen3:latest".to_string()),
                default_model_id: None,
                is_oauth: None,
                supports_api_key: None,
                api_key_url: None,
            },
            category: ProviderVendorCategory::Local,
            env_var: None,
            provider_config: None,
            supported_auth_modes: vec![ProviderAuthMode::Local],
            default_auth_mode: ProviderAuthMode::Local,
            supports_multiple_accounts: true,
        },
        // Custom
        ProviderDefinition {
            type_info: ProviderTypeInfo {
                id: "custom".to_string(),
                name: "Custom".to_string(),
                icon: "⚙️".to_string(),
                placeholder: "API key...".to_string(),
                model: None,
                requires_api_key: true,
                default_base_url: None,
                show_base_url: Some(true),
                show_model_id: Some(true),
                show_model_id_in_dev_mode_only: None,
                model_id_placeholder: Some("your-provider/model-id".to_string()),
                default_model_id: None,
                is_oauth: None,
                supports_api_key: None,
                api_key_url: None,
            },
            category: ProviderVendorCategory::Custom,
            env_var: Some("CUSTOM_API_KEY".to_string()),
            provider_config: None,
            supported_auth_modes: vec![ProviderAuthMode::ApiKey],
            default_auth_mode: ProviderAuthMode::ApiKey,
            supports_multiple_accounts: true,
        },
    ]
});

/// Extra environment-only providers (not exposed in UI)
pub static EXTRA_ENV_PROVIDERS: LazyLock<std::collections::HashMap<String, String>> = LazyLock::new(|| {
    let mut map = std::collections::HashMap::new();
    map.insert("groq".to_string(), "GROQ_API_KEY".to_string());
    map.insert("deepgram".to_string(), "DEEPGRAM_API_KEY".to_string());
    map.insert("cerebras".to_string(), "CEREBRAS_API_KEY".to_string());
    map.insert("xai".to_string(), "XAI_API_KEY".to_string());
    map.insert("mistral".to_string(), "MISTRAL_API_KEY".to_string());
    map
});

/// Get provider definition by vendor ID
pub fn get_provider_definition(vendor_id: &str) -> Option<&'static ProviderDefinition> {
    PROVIDER_VENDORS.iter().find(|p| p.type_info.id == vendor_id)
}

/// Get the environment variable name for a provider type
pub fn get_provider_env_var(vendor_id: &str) -> Option<&'static str> {
    if let Some(def) = get_provider_definition(vendor_id) {
        def.env_var.as_deref()
    } else {
        EXTRA_ENV_PROVIDERS.get(vendor_id).map(|s| s.as_str())
    }
}

/// Get all environment variable names for a provider type (primary first)
pub fn get_provider_env_vars(vendor_id: &str) -> Vec<&'static str> {
    get_provider_env_var(vendor_id).map(|v| vec![v]).unwrap_or_default()
}

/// Get the default model string for a provider type
pub fn get_provider_default_model(vendor_id: &str) -> Option<&'static str> {
    get_provider_definition(vendor_id)
        .and_then(|p| p.type_info.default_model_id.as_deref())
}

/// Get the provider backend config
pub fn get_provider_backend_config(vendor_id: &str) -> Option<&'static ProviderBackendConfig> {
    get_provider_definition(vendor_id)
        .and_then(|p| p.provider_config.as_ref())
}

/// Get all provider types that have env var mappings
pub fn get_keyable_provider_types() -> Vec<&'static str> {
    PROVIDER_VENDORS
        .iter()
        .filter(|p| p.env_var.is_some())
        .map(|p| p.type_info.id.as_str())
        .chain(EXTRA_ENV_PROVIDERS.keys().map(|k| k.as_str()))
        .collect()
}

/// Get provider type info list
pub fn get_provider_type_info_list() -> Vec<&'static ProviderTypeInfo> {
    PROVIDER_VENDORS
        .iter()
        .map(|p| &p.type_info)
        .collect()
}
