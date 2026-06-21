use crate::capability::CapabilityError;
use crate::llm::config::ConfigLoader;
use crate::llm::presets;
use crate::llm::provider::ProviderManager;
use crate::llm::role::RoleManager;
use crate::llm::types::*;
use crate::llm::usage::UsageTracker;
use crate::xdg::KunkkaPaths;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
    ChatCompletionResponseStream, CreateChatCompletionRequest,
};
use async_openai::types::embeddings::{CreateEmbeddingRequest, EmbeddingInput};
use async_openai::types::images::{CreateImageRequest, ImageModel, ImageSize};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Chat 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChatParams {
    pub role: String,
    pub messages: Vec<LlmMessage>,
    pub stream: Option<bool>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

/// LLM 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// Chat 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChatResponse {
    pub content: String,
    pub finish_reason: Option<String>,
    pub usage: Option<LlmUsage>,
}

/// Chat 流式 chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChatStreamChunk {
    pub content_delta: String,
    pub finish_reason: Option<String>,
    pub usage: Option<LlmUsage>,
}

/// 使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Embeddings 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEmbeddingsParams {
    pub role: String,
    pub input: Vec<String>,
}

/// Embeddings 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEmbeddingsResponse {
    pub embeddings: Vec<Vec<f32>>,
}

/// Images 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmImagesParams {
    pub role: String,
    pub prompt: String,
    pub size: Option<String>,
    pub n: Option<u32>,
}

/// Images 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmImagesResponse {
    pub urls: Vec<String>,
}

/// LLM 响应类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmResponse {
    /// Chat 响应
    Chat(LlmChatResponse),
    /// Embeddings 响应
    Embeddings(LlmEmbeddingsResponse),
    /// Images 响应
    Images(LlmImagesResponse),
    /// 供应商列表
    Providers(Vec<String>),
    /// 模型列表
    Models(Vec<(String, String)>),
    /// 角色列表
    Roles(Vec<String>),
    /// 预设列表
    Presets(Vec<LlmPresetInfo>),
    /// 供应商详情列表
    ProviderDetails(Vec<LlmProviderInfo>),
    /// 角色详情列表
    RoleDetails(Vec<LlmRoleInfo>),
    /// 供应商测试结果
    ProviderTestResult(LlmProviderTestResult),
    /// 使用统计汇总
    UsageSummary(LlmUsageSummary),
    /// 使用记录列表
    UsageRecords(Vec<LlmUsageRecord>),
    /// 默认角色
    DefaultRole(Option<String>),
    /// 操作成功
    Success,
}

/// 预设信息（用于 CLI 展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPresetInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
}

/// 供应商信息（用于 CLI 展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderInfo {
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub available_models: Vec<String>,
}

/// 角色信息（用于 CLI 展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRoleInfo {
    pub name: String,
    pub description: String,
    pub provider: String,
    pub model: String,
}

pub fn decode_chat_params(params: &[u8]) -> Result<LlmChatParams, CapabilityError> {
    postcard::from_bytes(params).map_err(|e| CapabilityError {
        code: "invalid_params".to_string(),
        message: format!("Failed to decode params: {e}"),
    })
}

pub fn is_streaming_chat(method: &str, params: &[u8]) -> Result<bool, CapabilityError> {
    if method != "chat" {
        return Ok(false);
    }
    let chat = decode_chat_params(params)?;
    Ok(chat.stream.unwrap_or(false))
}

fn build_chat_messages(messages: &[LlmMessage]) -> Vec<ChatCompletionRequestMessage> {
    messages
        .iter()
        .map(|msg| match msg.role.as_str() {
            "system" => ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: async_openai::types::chat::ChatCompletionRequestSystemMessageContent::Text(
                    msg.content.clone(),
                ),
                name: None,
            }),
            _ => ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(msg.content.clone()),
                name: None,
            }),
        })
        .collect()
}

fn build_chat_request(
    model: String,
    params: &LlmChatParams,
    role_params: &ModelParameters,
) -> CreateChatCompletionRequest {
    CreateChatCompletionRequest {
        model,
        messages: build_chat_messages(&params.messages),
        temperature: params.temperature.or(role_params.temperature),
        max_completion_tokens: params.max_tokens.or(role_params.max_tokens),
        top_p: role_params.top_p,
        frequency_penalty: role_params.frequency_penalty,
        presence_penalty: role_params.presence_penalty,
        ..Default::default()
    }
}

pub async fn create_chat_stream(
    params: LlmChatParams,
    state: &LlmState,
) -> Result<ChatCompletionResponseStream, CapabilityError> {
    let (provider_name, model, role_params) = state
        .role_manager
        .resolve_role(&params.role)
        .await
        .map_err(|e| CapabilityError {
        code: "role_error".to_string(),
        message: format!("Failed to resolve role: {:?}", e),
    })?;

    let manager = state.provider_manager.read().await;
    let manager = manager.as_ref().ok_or_else(|| CapabilityError {
        code: "not_initialized".to_string(),
        message: "LLM provider manager not initialized".to_string(),
    })?;

    let adapter = manager.get(&provider_name).ok_or_else(|| CapabilityError {
        code: "provider_not_found".to_string(),
        message: format!("Provider '{}' not found", provider_name),
    })?;

    let request = build_chat_request(model, &params, &role_params);
    adapter
        .client()
        .chat()
        .create_stream(request)
        .await
        .map_err(|e| CapabilityError {
            code: "llm_error".to_string(),
            message: format!("LLM stream request failed: {e}"),
        })
}

/// LLM 状态
pub struct LlmState {
    pub config_loader: ConfigLoader,
    pub provider_manager: Arc<RwLock<Option<ProviderManager>>>,
    pub role_manager: RoleManager,
    pub usage_tracker: UsageTracker,
}

impl LlmState {
    pub fn new(paths: &KunkkaPaths) -> Self {
        let config_loader = ConfigLoader::new(paths);
        let provider_manager = Arc::new(RwLock::new(None));
        let role_manager = RoleManager::new(config_loader.clone(), provider_manager.clone());

        Self {
            config_loader,
            provider_manager,
            role_manager,
            usage_tracker: UsageTracker::new(),
        }
    }

    /// 初始化：加载配置
    pub async fn initialize(&self) -> Result<(), CapabilityError> {
        // 加载供应商配置
        let providers_config =
            self.config_loader
                .load_providers()
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to load providers config: {:?}", e),
                })?;

        // 创建供应商管理器
        let manager = ProviderManager::from_config(&providers_config.providers).map_err(|e| {
            CapabilityError {
                code: "config_error".to_string(),
                message: format!("Failed to create provider manager: {:?}", e),
            }
        })?;

        *self.provider_manager.write().await = Some(manager);

        // 初始化角色管理器
        self.role_manager
            .initialize()
            .await
            .map_err(|e| CapabilityError {
                code: "config_error".to_string(),
                message: format!("Failed to initialize role manager: {:?}", e),
            })?;

        Ok(())
    }
}

/// 处理 LLM 请求
pub async fn handle_llm_request(
    method: &str,
    params: &[u8],
    state: &LlmState,
) -> Result<Vec<u8>, CapabilityError> {
    let response = match method {
        "chat" => {
            let request: LlmChatParams =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;
            handle_chat(request, state).await?
        }
        "embeddings" => {
            let request: LlmEmbeddingsParams =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;
            handle_embeddings(request, state).await?
        }
        "images" => {
            let request: LlmImagesParams =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;
            handle_images(request, state).await?
        }
        "list_providers" => {
            let manager = state.provider_manager.read().await;
            let manager = manager.as_ref().ok_or_else(|| CapabilityError {
                code: "not_initialized".to_string(),
                message: "LLM provider manager not initialized".to_string(),
            })?;
            LlmResponse::Providers(manager.list_providers())
        }
        "list_models" => {
            let manager = state.provider_manager.read().await;
            let manager = manager.as_ref().ok_or_else(|| CapabilityError {
                code: "not_initialized".to_string(),
                message: "LLM provider manager not initialized".to_string(),
            })?;
            LlmResponse::Models(manager.list_available_models())
        }
        "list_roles" => {
            let roles = state.role_manager.list_roles().await;
            LlmResponse::Roles(roles)
        }
        "add_provider" => {
            let request: AddProviderRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let mut providers_config =
                state
                    .config_loader
                    .load_providers()
                    .map_err(|e| CapabilityError {
                        code: "config_error".to_string(),
                        message: format!("Failed to load providers config: {:?}", e),
                    })?;

            providers_config
                .providers
                .insert(request.name, request.config);

            state
                .config_loader
                .save_providers(&providers_config)
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to save providers config: {:?}", e),
                })?;

            // 重新加载
            state.initialize().await?;

            LlmResponse::Success
        }
        "remove_provider" => {
            let request: RemoveProviderRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let mut providers_config =
                state
                    .config_loader
                    .load_providers()
                    .map_err(|e| CapabilityError {
                        code: "config_error".to_string(),
                        message: format!("Failed to load providers config: {:?}", e),
                    })?;

            providers_config.providers.remove(&request.name);

            state
                .config_loader
                .save_providers(&providers_config)
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to save providers config: {:?}", e),
                })?;

            // 重新加载
            state.initialize().await?;

            LlmResponse::Success
        }
        "add_role" => {
            let request: AddRoleRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            state
                .role_manager
                .add_role(request.name, request.config)
                .await
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to add role: {:?}", e),
                })?;

            LlmResponse::Success
        }
        "remove_role" => {
            let request: RemoveRoleRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            state
                .role_manager
                .remove_role(&request.name)
                .await
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to remove role: {:?}", e),
                })?;

            LlmResponse::Success
        }
        "list_presets" => {
            let preset_list = presets::all_presets();
            let preset_infos: Vec<LlmPresetInfo> = preset_list
                .into_iter()
                .map(|p| LlmPresetInfo {
                    name: p.name.to_string(),
                    display_name: p.display_name.to_string(),
                    description: p.description.to_string(),
                })
                .collect();
            LlmResponse::Presets(preset_infos)
        }
        "apply_preset" => {
            let request: ApplyPresetRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let (name, config) =
                presets::create_provider_from_preset(&request.preset_name, &request.api_key)
                    .ok_or_else(|| CapabilityError {
                        code: "invalid_params".to_string(),
                        message: format!("Unknown preset: {}", request.preset_name),
                    })?;

            let mut providers_config =
                state
                    .config_loader
                    .load_providers()
                    .map_err(|e| CapabilityError {
                        code: "config_error".to_string(),
                        message: format!("Failed to load providers config: {:?}", e),
                    })?;

            providers_config.providers.insert(name, config);

            state
                .config_loader
                .save_providers(&providers_config)
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to save providers config: {:?}", e),
                })?;

            // 重新加载
            state.initialize().await?;

            LlmResponse::Success
        }
        "list_providers_detail" => {
            let providers_config =
                state
                    .config_loader
                    .load_providers()
                    .map_err(|e| CapabilityError {
                        code: "config_error".to_string(),
                        message: format!("Failed to load providers config: {:?}", e),
                    })?;

            let provider_infos: Vec<LlmProviderInfo> = providers_config
                .providers
                .iter()
                .map(|(name, config)| LlmProviderInfo {
                    name: name.clone(),
                    provider_type: format!("{:?}", config.provider_type),
                    base_url: config.base_url.clone(),
                    available_models: config.available_models.clone(),
                })
                .collect();

            LlmResponse::ProviderDetails(provider_infos)
        }
        "list_roles_detail" => {
            let roles_config = state
                .config_loader
                .load_roles()
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to load roles config: {:?}", e),
                })?;

            let role_infos: Vec<LlmRoleInfo> = roles_config
                .roles
                .iter()
                .map(|(name, config)| LlmRoleInfo {
                    name: name.clone(),
                    description: config.description.clone(),
                    provider: config.provider.clone(),
                    model: config.model.clone(),
                })
                .collect();

            LlmResponse::RoleDetails(role_infos)
        }
        "list_role_presets" => {
            let preset_list = presets::all_role_presets();
            let preset_infos: Vec<LlmPresetInfo> = preset_list
                .into_iter()
                .map(|p| LlmPresetInfo {
                    name: p.name.to_string(),
                    display_name: p.display_name.to_string(),
                    description: p.description.to_string(),
                })
                .collect();
            LlmResponse::Presets(preset_infos)
        }
        "apply_role_preset" => {
            let request: ApplyRolePresetRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let (name, config) = presets::create_role_from_preset(
                &request.preset_name,
                &request.provider,
                &request.model,
            )
            .ok_or_else(|| CapabilityError {
                code: "invalid_params".to_string(),
                message: format!("Unknown role preset: {}", request.preset_name),
            })?;

            state
                .role_manager
                .add_role(name, config)
                .await
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to add role: {:?}", e),
                })?;

            LlmResponse::Success
        }
        "test_provider" => {
            let request: TestProviderRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let manager = state.provider_manager.read().await;
            let manager = manager.as_ref().ok_or_else(|| CapabilityError {
                code: "not_initialized".to_string(),
                message: "LLM provider manager not initialized".to_string(),
            })?;

            let result =
                manager
                    .test_provider(&request.name)
                    .await
                    .ok_or_else(|| CapabilityError {
                        code: "provider_not_found".to_string(),
                        message: format!("Provider '{}' not found", request.name),
                    })?;

            LlmResponse::ProviderTestResult(LlmProviderTestResult {
                success: result.success,
                latency_ms: result.latency_ms,
                error: result.error,
            })
        }
        "update_provider" => {
            let request: UpdateProviderRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let mut providers_config =
                state
                    .config_loader
                    .load_providers()
                    .map_err(|e| CapabilityError {
                        code: "config_error".to_string(),
                        message: format!("Failed to load providers config: {:?}", e),
                    })?;

            let existing = providers_config
                .providers
                .get_mut(&request.name)
                .ok_or_else(|| CapabilityError {
                    code: "provider_not_found".to_string(),
                    message: format!("Provider '{}' not found", request.name),
                })?;

            // 更新字段
            if let Some(api_key) = request.api_key {
                existing.api_key = Some(api_key);
            }
            if let Some(base_url) = request.base_url {
                existing.base_url = base_url;
            }
            if let Some(models) = request.available_models {
                existing.available_models = models;
            }

            state
                .config_loader
                .save_providers(&providers_config)
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to save providers config: {:?}", e),
                })?;

            state.initialize().await?;

            LlmResponse::Success
        }
        "update_role" => {
            let request: UpdateRoleRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let mut roles_config =
                state
                    .config_loader
                    .load_roles()
                    .map_err(|e| CapabilityError {
                        code: "config_error".to_string(),
                        message: format!("Failed to load roles config: {:?}", e),
                    })?;

            let existing =
                roles_config
                    .roles
                    .get_mut(&request.name)
                    .ok_or_else(|| CapabilityError {
                        code: "role_not_found".to_string(),
                        message: format!("Role '{}' not found", request.name),
                    })?;

            // 更新字段
            if let Some(description) = request.description {
                existing.description = description;
            }
            if let Some(provider) = request.provider {
                existing.provider = provider;
            }
            if let Some(model) = request.model {
                existing.model = model;
            }
            if let Some(temperature) = request.temperature {
                existing.parameters.temperature = Some(temperature);
            }
            if let Some(max_tokens) = request.max_tokens {
                existing.parameters.max_tokens = Some(max_tokens);
            }

            state
                .config_loader
                .save_roles(&roles_config)
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to save roles config: {:?}", e),
                })?;

            state
                .role_manager
                .initialize()
                .await
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to reinitialize role manager: {:?}", e),
                })?;

            LlmResponse::Success
        }
        "usage_summary" => {
            let summary = state.usage_tracker.summary().await;
            LlmResponse::UsageSummary(LlmUsageSummary {
                total_requests: summary.total_requests,
                total_prompt_tokens: summary.total_prompt_tokens,
                total_completion_tokens: summary.total_completion_tokens,
                total_tokens: summary.total_tokens,
            })
        }
        "usage_records" => {
            let request: UsageRecordsRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let records = state.usage_tracker.recent(request.limit).await;
            let usage_records: Vec<LlmUsageRecord> = records
                .into_iter()
                .map(|r| LlmUsageRecord {
                    timestamp: r.timestamp,
                    provider: r.provider,
                    model: r.model,
                    role: r.role,
                    prompt_tokens: r.prompt_tokens,
                    completion_tokens: r.completion_tokens,
                    total_tokens: r.total_tokens,
                })
                .collect();

            LlmResponse::UsageRecords(usage_records)
        }
        "clear_usage" => {
            state.usage_tracker.clear().await;
            LlmResponse::Success
        }
        "set_default_role" => {
            let request: SetDefaultRoleRequest =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            state
                .role_manager
                .set_default_role(request.role_name)
                .await
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to set default role: {:?}", e),
                })?;

            LlmResponse::Success
        }
        "get_default_role" => {
            let default_role = state.role_manager.get_default_role().await;
            LlmResponse::DefaultRole(default_role)
        }
        _ => {
            return Err(CapabilityError {
                code: "unknown_method".to_string(),
                message: format!("Unknown LLM method: {method}"),
            });
        }
    };

    postcard::to_stdvec(&response).map_err(|e| CapabilityError {
        code: "encode_error".to_string(),
        message: format!("Failed to encode response: {e}"),
    })
}

/// 处理 Chat 请求
async fn handle_chat(
    params: LlmChatParams,
    state: &LlmState,
) -> Result<LlmResponse, CapabilityError> {
    // 解析角色
    let (provider_name, model, role_params) = state
        .role_manager
        .resolve_role(&params.role)
        .await
        .map_err(|e| CapabilityError {
        code: "role_error".to_string(),
        message: format!("Failed to resolve role: {:?}", e),
    })?;

    // 获取供应商适配器
    let manager = state.provider_manager.read().await;
    let manager = manager.as_ref().ok_or_else(|| CapabilityError {
        code: "not_initialized".to_string(),
        message: "LLM provider manager not initialized".to_string(),
    })?;

    let adapter = manager.get(&provider_name).ok_or_else(|| CapabilityError {
        code: "provider_not_found".to_string(),
        message: format!("Provider '{}' not found", provider_name),
    })?;

    let request = build_chat_request(model.clone(), &params, &role_params);

    // 发送请求
    let response = adapter
        .client()
        .chat()
        .create(request)
        .await
        .map_err(|e| CapabilityError {
            code: "llm_error".to_string(),
            message: format!("LLM request failed: {e}"),
        })?;

    // 提取响应
    let choice = response.choices.first().ok_or_else(|| CapabilityError {
        code: "llm_error".to_string(),
        message: "No choices in response".to_string(),
    })?;

    let content = choice.message.content.clone().unwrap_or_default();
    let finish_reason = choice.finish_reason.as_ref().map(|r| format!("{:?}", r));

    let usage = response.usage.map(|u| LlmUsage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });

    // 记录使用量
    if let Some(ref u) = usage {
        state
            .usage_tracker
            .record(
                provider_name.clone(),
                model.clone(),
                params.role.clone(),
                u.prompt_tokens,
                u.completion_tokens,
            )
            .await;
    }

    Ok(LlmResponse::Chat(LlmChatResponse {
        content,
        finish_reason,
        usage,
    }))
}

/// 处理 Embeddings 请求
async fn handle_embeddings(
    params: LlmEmbeddingsParams,
    state: &LlmState,
) -> Result<LlmResponse, CapabilityError> {
    // 解析角色
    let (provider_name, model, _) = state
        .role_manager
        .resolve_role(&params.role)
        .await
        .map_err(|e| CapabilityError {
            code: "role_error".to_string(),
            message: format!("Failed to resolve role: {:?}", e),
        })?;

    // 获取供应商适配器
    let manager = state.provider_manager.read().await;
    let manager = manager.as_ref().ok_or_else(|| CapabilityError {
        code: "not_initialized".to_string(),
        message: "LLM provider manager not initialized".to_string(),
    })?;

    let adapter = manager.get(&provider_name).ok_or_else(|| CapabilityError {
        code: "provider_not_found".to_string(),
        message: format!("Provider '{}' not found", provider_name),
    })?;

    // 构建请求
    let request = CreateEmbeddingRequest {
        model: model.clone(),
        input: EmbeddingInput::StringArray(params.input),
        encoding_format: None,
        dimensions: None,
        user: None,
    };

    // 发送请求
    let response = adapter
        .client()
        .embeddings()
        .create(request)
        .await
        .map_err(|e| CapabilityError {
            code: "llm_error".to_string(),
            message: format!("LLM request failed: {e}"),
        })?;

    // 提取响应
    let embeddings: Vec<Vec<f32>> = response.data.iter().map(|d| d.embedding.clone()).collect();

    Ok(LlmResponse::Embeddings(LlmEmbeddingsResponse {
        embeddings,
    }))
}

/// 处理 Images 请求
async fn handle_images(
    params: LlmImagesParams,
    state: &LlmState,
) -> Result<LlmResponse, CapabilityError> {
    // 解析角色
    let (provider_name, model, _) = state
        .role_manager
        .resolve_role(&params.role)
        .await
        .map_err(|e| CapabilityError {
            code: "role_error".to_string(),
            message: format!("Failed to resolve role: {:?}", e),
        })?;

    // 获取供应商适配器
    let manager = state.provider_manager.read().await;
    let manager = manager.as_ref().ok_or_else(|| CapabilityError {
        code: "not_initialized".to_string(),
        message: "LLM provider manager not initialized".to_string(),
    })?;

    let adapter = manager.get(&provider_name).ok_or_else(|| CapabilityError {
        code: "provider_not_found".to_string(),
        message: format!("Provider '{}' not found", provider_name),
    })?;

    // 构建请求
    let request = CreateImageRequest {
        model: Some(ImageModel::Other(model.clone())),
        prompt: params.prompt,
        size: params.size.map(|s| match s.as_str() {
            "256x256" => ImageSize::S256x256,
            "512x512" => ImageSize::S512x512,
            "1024x1024" => ImageSize::S1024x1024,
            "1792x1024" => ImageSize::S1792x1024,
            "1024x1792" => ImageSize::S1024x1792,
            _ => ImageSize::Auto,
        }),
        n: params.n.map(|n| n as u8),
        response_format: None,
        quality: None,
        style: None,
        user: None,
        background: None,
        moderation: None,
        output_compression: None,
        output_format: None,
        partial_images: None,
        stream: None,
    };

    // 发送请求
    let response = adapter
        .client()
        .images()
        .generate(request)
        .await
        .map_err(|e| CapabilityError {
            code: "llm_error".to_string(),
            message: format!("LLM request failed: {e}"),
        })?;

    // 提取响应
    let urls: Vec<String> = response
        .data
        .iter()
        .filter_map(|d| match d.as_ref() {
            async_openai::types::images::Image::Url { url, .. } => Some(url.clone()),
            _ => None,
        })
        .collect();

    Ok(LlmResponse::Images(LlmImagesResponse { urls }))
}

/// 添加供应商请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AddProviderRequest {
    name: String,
    config: ProviderConfig,
}

/// 删除供应商请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoveProviderRequest {
    name: String,
}

/// 添加角色请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AddRoleRequest {
    name: String,
    config: RoleConfig,
}

/// 删除角色请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoveRoleRequest {
    name: String,
}

/// 应用预设请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApplyPresetRequest {
    preset_name: String,
    api_key: String,
}

/// 应用角色预设请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApplyRolePresetRequest {
    preset_name: String,
    provider: String,
    model: String,
}

/// 测试供应商请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestProviderRequest {
    name: String,
}

/// 供应商测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderTestResult {
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// 更新供应商请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateProviderRequest {
    name: String,
    api_key: Option<String>,
    base_url: Option<String>,
    available_models: Option<Vec<String>>,
}

/// 更新角色请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateRoleRequest {
    name: String,
    description: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

/// 使用统计汇总（用于 CLI 展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsageSummary {
    pub total_requests: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_tokens: u64,
}

/// 使用记录（用于 CLI 展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsageRecord {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub role: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 查询使用记录请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageRecordsRequest {
    limit: usize,
}

/// 设置默认角色请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SetDefaultRoleRequest {
    role_name: Option<String>,
}
