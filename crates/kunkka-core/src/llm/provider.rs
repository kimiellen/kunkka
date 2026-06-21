use super::types::{ProviderConfig, ProviderType};
use crate::Result;
use async_openai::config::OpenAIConfig;
use async_openai::Client;

/// 供应商适配器
pub struct ProviderAdapter {
    client: Client<OpenAIConfig>,
    config: ProviderConfig,
}

impl ProviderAdapter {
    /// 创建新的供应商适配器
    pub fn new(_name: &str, config: &ProviderConfig) -> Result<Self> {
        let openai_config = match config.provider_type {
            ProviderType::ApiKey | ProviderType::Local => {
                let api_key = config.api_key.as_deref().unwrap_or("dummy");
                OpenAIConfig::new()
                    .with_api_base(&config.base_url)
                    .with_api_key(api_key)
            }
            ProviderType::Subscription => {
                // 订阅模式：使用 OAuth token 或其他认证方式
                let api_key = config.api_key.as_deref().unwrap_or("");
                OpenAIConfig::new()
                    .with_api_base(&config.base_url)
                    .with_api_key(api_key)
            }
        };

        let client = Client::with_config(openai_config);

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// 获取客户端引用
    pub fn client(&self) -> &Client<OpenAIConfig> {
        &self.client
    }

    /// 获取配置引用
    pub fn config(&self) -> &ProviderConfig {
        &self.config
    }

    /// 检查模型是否可用
    pub fn is_model_available(&self, model: &str) -> bool {
        self.config.available_models.contains(&model.to_string())
    }

    /// 测试供应商连接（发送一个简单的 chat 请求）
    pub async fn test_connection(&self) -> Result<ProviderTestResult> {
        let start = std::time::Instant::now();

        // 尝试发送一个简单的 chat 请求来测试连接
        use async_openai::types::chat::{
            ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
            ChatCompletionRequestUserMessageContent, CreateChatCompletionRequest,
        };

        let request = CreateChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text("hi".to_string()),
                    name: None,
                },
            )],
            max_completion_tokens: Some(1),
            ..Default::default()
        };

        match self.client.chat().create(request).await {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(ProviderTestResult {
                    success: true,
                    latency_ms: Some(latency),
                    error: None,
                })
            }
            Err(e) => {
                let latency = start.elapsed().as_millis() as u64;
                // 某些错误（如模型不存在）仍然表示连接成功
                let error_str = format!("{e}");
                let success = !error_str.contains("connection")
                    && !error_str.contains("timeout")
                    && !error_str.contains("dns");
                Ok(ProviderTestResult {
                    success,
                    latency_ms: Some(latency),
                    error: if success { None } else { Some(error_str) },
                })
            }
        }
    }
}

/// 供应商测试结果
#[derive(Debug, Clone)]
pub struct ProviderTestResult {
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// 供应商管理器
pub struct ProviderManager {
    adapters: std::collections::HashMap<String, ProviderAdapter>,
}

impl ProviderManager {
    /// 从配置创建供应商管理器
    pub fn from_config(
        providers: &std::collections::HashMap<String, ProviderConfig>,
    ) -> Result<Self> {
        let mut adapters = std::collections::HashMap::new();

        for (name, config) in providers {
            let adapter = ProviderAdapter::new(name, config)?;
            adapters.insert(name.clone(), adapter);
        }

        Ok(Self { adapters })
    }

    /// 获取供应商适配器
    pub fn get(&self, name: &str) -> Option<&ProviderAdapter> {
        self.adapters.get(name)
    }

    /// 列出所有供应商名称
    pub fn list_providers(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }

    /// 列出所有可用模型
    pub fn list_available_models(&self) -> Vec<(String, String)> {
        let mut models = Vec::new();
        for (provider_name, adapter) in &self.adapters {
            for model in &adapter.config().available_models {
                models.push((provider_name.clone(), model.clone()));
            }
        }
        models
    }

    /// 测试指定供应商连接
    pub async fn test_provider(&self, name: &str) -> Option<ProviderTestResult> {
        let adapter = self.adapters.get(name)?;
        Some(
            adapter
                .test_connection()
                .await
                .unwrap_or(ProviderTestResult {
                    success: false,
                    latency_ms: None,
                    error: Some("Failed to test connection".to_string()),
                }),
        )
    }
}
