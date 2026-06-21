use super::types::{ModelParameters, ProviderConfig, ProviderType, RoleConfig};

/// 预设供应商配置
pub struct ProviderPreset {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub config: ProviderConfig,
}

/// 预设角色配置
pub struct RolePreset {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub config: RoleConfig,
}

/// 获取所有预设供应商
pub fn all_presets() -> Vec<ProviderPreset> {
    vec![
        openai_preset(),
        zhipu_preset(),
        kimi_preset(),
        xiaomi_preset(),
    ]
}

/// 获取所有预设角色
pub fn all_role_presets() -> Vec<RolePreset> {
    vec![
        thinker_role_preset(),
        coder_role_preset(),
        collector_role_preset(),
        reviewer_role_preset(),
    ]
}

/// 根据名称获取预设
pub fn get_preset(name: &str) -> Option<ProviderPreset> {
    all_presets().into_iter().find(|p| p.name == name)
}

/// 根据名称获取角色预设
pub fn get_role_preset(name: &str) -> Option<RolePreset> {
    all_role_presets().into_iter().find(|p| p.name == name)
}

/// 列出所有预设名称
pub fn list_preset_names() -> Vec<(&'static str, &'static str)> {
    all_presets()
        .iter()
        .map(|p| (p.name, p.display_name))
        .collect()
}

/// 列出所有角色预设名称
pub fn list_role_preset_names() -> Vec<(&'static str, &'static str)> {
    all_role_presets()
        .iter()
        .map(|p| (p.name, p.display_name))
        .collect()
}

fn openai_preset() -> ProviderPreset {
    ProviderPreset {
        name: "openai",
        display_name: "OpenAI",
        description: "OpenAI GPT 系列模型 (GPT-4o, GPT-4o-mini 等)",
        config: ProviderConfig {
            provider_type: ProviderType::ApiKey,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: None,
            available_models: vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
            rate_limit: None,
            auth_method: None,
            credentials: None,
        },
    }
}

fn zhipu_preset() -> ProviderPreset {
    ProviderPreset {
        name: "zhipu",
        display_name: "智谱 AI",
        description: "智谱 GLM 系列模型 (GLM-4, GLM-4-Flash 等)",
        config: ProviderConfig {
            provider_type: ProviderType::ApiKey,
            base_url: "https://open.bigmodel.cn/api/paas/v4".to_string(),
            api_key: None,
            available_models: vec![
                "glm-4".to_string(),
                "glm-4-flash".to_string(),
                "glm-4v".to_string(),
                "glm-3-turbo".to_string(),
            ],
            rate_limit: None,
            auth_method: None,
            credentials: None,
        },
    }
}

fn kimi_preset() -> ProviderPreset {
    ProviderPreset {
        name: "kimi",
        display_name: "Kimi (月之暗面)",
        description: "Kimi 系列模型 (Moonshot-v1 等)",
        config: ProviderConfig {
            provider_type: ProviderType::ApiKey,
            base_url: "https://api.moonshot.cn/v1".to_string(),
            api_key: None,
            available_models: vec![
                "moonshot-v1-8k".to_string(),
                "moonshot-v1-32k".to_string(),
                "moonshot-v1-128k".to_string(),
            ],
            rate_limit: None,
            auth_method: None,
            credentials: None,
        },
    }
}

fn xiaomi_preset() -> ProviderPreset {
    ProviderPreset {
        name: "xiaomi",
        display_name: "小米 MiMo",
        description: "小米 MiMo 系列模型",
        config: ProviderConfig {
            provider_type: ProviderType::ApiKey,
            base_url: "https://api.xiaomi.com/v1".to_string(),
            api_key: None,
            available_models: vec!["mimo-7b".to_string()],
            rate_limit: None,
            auth_method: None,
            credentials: None,
        },
    }
}

/// 从预设创建供应商配置（需要用户提供 API key）
pub fn create_provider_from_preset(
    preset_name: &str,
    api_key: &str,
) -> Option<(String, ProviderConfig)> {
    let preset = get_preset(preset_name)?;
    let mut config = preset.config;
    config.api_key = Some(api_key.to_string());
    Some((preset_name.to_string(), config))
}

/// 从角色预设创建角色配置（需要指定供应商和模型）
pub fn create_role_from_preset(
    preset_name: &str,
    provider: &str,
    model: &str,
) -> Option<(String, RoleConfig)> {
    let preset = get_role_preset(preset_name)?;
    let mut config = preset.config;
    config.provider = provider.to_string();
    config.model = model.to_string();
    Some((preset_name.to_string(), config))
}

// ========== 角色预设 ==========

fn thinker_role_preset() -> RolePreset {
    RolePreset {
        name: "thinker",
        display_name: "深度思考",
        description: "复杂推理、分析、规划任务",
        config: RoleConfig {
            description: "深度思考角色，用于复杂推理、分析和规划任务".to_string(),
            provider: String::new(), // 需要用户指定
            model: String::new(),    // 需要用户指定
            parameters: ModelParameters {
                temperature: Some(0.7),
                max_tokens: Some(4096),
                top_p: Some(0.9),
                frequency_penalty: None,
                presence_penalty: None,
            },
        },
    }
}

fn coder_role_preset() -> RolePreset {
    RolePreset {
        name: "coder",
        display_name: "代码生成",
        description: "编码、调试、代码重构",
        config: RoleConfig {
            description: "代码生成角色，用于编码、调试和代码重构".to_string(),
            provider: String::new(),
            model: String::new(),
            parameters: ModelParameters {
                temperature: Some(0.3),
                max_tokens: Some(8192),
                top_p: Some(0.95),
                frequency_penalty: None,
                presence_penalty: None,
            },
        },
    }
}

fn collector_role_preset() -> RolePreset {
    RolePreset {
        name: "collector",
        display_name: "信息收集",
        description: "快速查询、总结、信息提取",
        config: RoleConfig {
            description: "信息收集角色，用于快速查询、总结和信息提取".to_string(),
            provider: String::new(),
            model: String::new(),
            parameters: ModelParameters {
                temperature: Some(0.5),
                max_tokens: Some(2048),
                top_p: Some(0.9),
                frequency_penalty: None,
                presence_penalty: None,
            },
        },
    }
}

fn reviewer_role_preset() -> RolePreset {
    RolePreset {
        name: "reviewer",
        display_name: "代码审查",
        description: "代码审查、安全检查、建议",
        config: RoleConfig {
            description: "代码审查角色，用于代码审查、安全检查和改进建议".to_string(),
            provider: String::new(),
            model: String::new(),
            parameters: ModelParameters {
                temperature: Some(0.2),
                max_tokens: Some(4096),
                top_p: Some(0.9),
                frequency_penalty: None,
                presence_penalty: None,
            },
        },
    }
}
