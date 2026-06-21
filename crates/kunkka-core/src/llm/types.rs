use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 供应商类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    ApiKey,
    Subscription,
    Local,
}

/// 供应商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: ProviderType,
    pub base_url: String,
    pub api_key: Option<String>,
    pub available_models: Vec<String>,
    pub rate_limit: Option<RateLimit>,
    pub auth_method: Option<String>,
    pub credentials: Option<serde_json::Value>,
}

/// 速率限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_minute: Option<u32>,
    pub tokens_per_minute: Option<u32>,
}

/// LLM 配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub providers: HashMap<String, ProviderConfig>,
}

/// 角色配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleConfig {
    pub description: String,
    pub provider: String,
    pub model: String,
    pub parameters: ModelParameters,
}

/// 模型参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameters {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
}

/// 角色配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolesConfig {
    pub roles: HashMap<String, RoleConfig>,
    /// 默认角色名称（当不指定角色时使用）
    #[serde(default)]
    pub default_role: Option<String>,
}
