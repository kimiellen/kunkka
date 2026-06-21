# LLM Provider Capability - 供应商层实现计划

## 1. 目标

实现 LLM Provider Capability 的供应商层，包括：
- 配置文件加载与解析
- 供应商适配器接口
- API Key 和订阅账号认证
- 可用模型管理

## 2. 文件结构

```
crates/kunkka-core/src/
├── llm/
│   ├── mod.rs              # 模块入口
│   ├── config.rs           # 配置加载
│   ├── provider.rs         # 供应商适配器
│   └── types.rs            # 类型定义
└── capability/
    └── llm.rs              # Capability handler
```

## 3. 实现任务

### 任务 1: 添加 async-openai 依赖

**文件**: `Cargo.toml` (workspace root), `crates/kunkka-core/Cargo.toml`

**步骤**:
1. 在 workspace `Cargo.toml` 添加依赖：
   ```toml
   async-openai = { version = "0.23", features = ["chat", "embeddings", "images"] }
   ```
2. 在 `crates/kunkka-core/Cargo.toml` 添加：
   ```toml
   async-openai.workspace = true
   ```

### 任务 2: 实现类型定义

**文件**: `crates/kunkka-core/src/llm/types.rs`

**内容**:
```rust
use serde::{Deserialize, Serialize};

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
    pub providers: std::collections::HashMap<String, ProviderConfig>,
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
    pub roles: std::collections::HashMap<String, RoleConfig>,
}
```

### 任务 3: 实现配置加载

**文件**: `crates/kunkka-core/src/llm/config.rs`

**内容**:
```rust
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use super::types::{LlmConfig, RolesConfig};
use std::fs;
use std::path::PathBuf;

/// 配置加载器
pub struct ConfigLoader {
    config_dir: PathBuf,
}

impl ConfigLoader {
    pub fn new(paths: &KunkkaPaths) -> Self {
        Self {
            config_dir: paths.config_dir.clone(),
        }
    }

    /// 加载供应商配置
    pub fn load_providers(&self) -> Result<LlmConfig> {
        let config_path = self.config_dir.join("llm-providers.json");
        
        if !config_path.exists() {
            return Ok(LlmConfig {
                providers: std::collections::HashMap::new(),
            });
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| CoreError::Config(format!("Failed to read providers config: {e}")))?;

        let config: LlmConfig = serde_json::from_str(&content)
            .map_err(|e| CoreError::Config(format!("Failed to parse providers config: {e}")))?;

        Ok(config)
    }

    /// 加载角色配置
    pub fn load_roles(&self) -> Result<RolesConfig> {
        let config_path = self.config_dir.join("llm-roles.json");
        
        if !config_path.exists() {
            return Ok(RolesConfig {
                roles: std::collections::HashMap::new(),
            });
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| CoreError::Config(format!("Failed to read roles config: {e}")))?;

        let config: RolesConfig = serde_json::from_str(&content)
            .map_err(|e| CoreError::Config(format!("Failed to parse roles config: {e}")))?;

        Ok(config)
    }

    /// 保存供应商配置
    pub fn save_providers(&self, config: &LlmConfig) -> Result<()> {
        let config_path = self.config_dir.join("llm-providers.json");
        
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| CoreError::Config(format!("Failed to serialize providers config: {e}")))?;

        fs::write(&config_path, content)
            .map_err(|e| CoreError::Config(format!("Failed to write providers config: {e}")))?;

        Ok(())
    }

    /// 保存角色配置
    pub fn save_roles(&self, config: &RolesConfig) -> Result<()> {
        let config_path = self.config_dir.join("llm-roles.json");
        
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| CoreError::Config(format!("Failed to serialize roles config: {e}")))?;

        fs::write(&config_path, content)
            .map_err(|e| CoreError::Config(format!("Failed to write roles config: {e}")))?;

        Ok(())
    }
}
```

### 任务 4: 实现供应商适配器

**文件**: `crates/kunkka-core/src/llm/provider.rs`

**内容**:
```rust
use async_openai::config::{Config, OpenAIConfig};
use async_openai::Client;
use super::types::{ProviderConfig, ProviderType};
use crate::{CoreError, Result};

/// 供应商适配器
pub struct ProviderAdapter {
    client: Client<OpenAIConfig>,
    config: ProviderConfig,
}

impl ProviderAdapter {
    /// 创建新的供应商适配器
    pub fn new(name: &str, config: &ProviderConfig) -> Result<Self> {
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
}
```

### 任务 5: 实现 Capability Handler

**文件**: `crates/kunkka-core/src/capability/llm.rs`

**内容**:
```rust
use crate::app_manifest::AppManifest;
use crate::capability::CapabilityError;
use crate::llm::config::ConfigLoader;
use crate::llm::provider::ProviderManager;
use crate::llm::types::*;
use crate::xdg::KunkkaPaths;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// LLM 请求类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmRequest {
    /// 列出供应商
    ListProviders,
    /// 列出可用模型
    ListModels,
    /// 列出角色
    ListRoles,
    /// 添加供应商
    AddProvider { name: String, config: ProviderConfig },
    /// 删除供应商
    RemoveProvider { name: String },
    /// 添加角色
    AddRole { name: String, config: RoleConfig },
    /// 删除角色
    RemoveRole { name: String },
}

/// LLM 响应类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmResponse {
    /// 供应商列表
    Providers(Vec<String>),
    /// 模型列表
    Models(Vec<(String, String)>),
    /// 角色列表
    Roles(Vec<String>),
    /// 操作成功
    Success,
}

/// LLM 状态
pub struct LlmState {
    pub config_loader: ConfigLoader,
    pub provider_manager: Arc<RwLock<Option<ProviderManager>>>,
    pub roles_config: Arc<RwLock<RolesConfig>>,
}

impl LlmState {
    pub fn new(paths: &KunkkaPaths) -> Self {
        Self {
            config_loader: ConfigLoader::new(paths),
            provider_manager: Arc::new(RwLock::new(None)),
            roles_config: Arc::new(RwLock::new(RolesConfig {
                roles: std::collections::HashMap::new(),
            })),
        }
    }

    /// 初始化：加载配置
    pub async fn initialize(&self) -> Result<(), CapabilityError> {
        // 加载供应商配置
        let providers_config = self.config_loader.load_providers()
            .map_err(|e| CapabilityError {
                code: "config_error".to_string(),
                message: format!("Failed to load providers config: {e}"),
            })?;

        // 创建供应商管理器
        let manager = ProviderManager::from_config(&providers_config.providers)
            .map_err(|e| CapabilityError {
                code: "config_error".to_string(),
                message: format!("Failed to create provider manager: {e}"),
            })?;

        *self.provider_manager.write().await = Some(manager);

        // 加载角色配置
        let roles_config = self.config_loader.load_roles()
            .map_err(|e| CapabilityError {
                code: "config_error".to_string(),
                message: format!("Failed to load roles config: {e}"),
            })?;

        *self.roles_config.write().await = roles_config;

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
            let roles = state.roles_config.read().await;
            LlmResponse::Roles(roles.roles.keys().cloned().collect())
        }
        "add_provider" => {
            let request: AddProviderRequest = postcard::from_bytes(params)
                .map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let mut providers_config = state.config_loader.load_providers()
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to load providers config: {e}"),
                })?;

            providers_config.providers.insert(request.name, request.config);

            state.config_loader.save_providers(&providers_config)
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to save providers config: {e}"),
                })?;

            // 重新加载
            state.initialize().await?;

            LlmResponse::Success
        }
        "remove_provider" => {
            let request: RemoveProviderRequest = postcard::from_bytes(params)
                .map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("Failed to decode params: {e}"),
                })?;

            let mut providers_config = state.config_loader.load_providers()
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to load providers config: {e}"),
                })?;

            providers_config.providers.remove(&request.name);

            state.config_loader.save_providers(&providers_config)
                .map_err(|e| CapabilityError {
                    code: "config_error".to_string(),
                    message: format!("Failed to save providers config: {e}"),
                })?;

            // 重新加载
            state.initialize().await?;

            LlmResponse::Success
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
```

## 4. 测试计划

### 单元测试

1. **配置加载测试**
   - 测试加载空配置
   - 测试加载完整配置
   - 测试配置文件不存在的情况

2. **供应商管理器测试**
   - 测试创建管理器
   - 测试列出供应商
   - 测试列出模型

3. **Capability Handler 测试**
   - 测试 list_providers
   - 测试 list_models
   - 测试 add_provider
   - 测试 remove_provider

### 集成测试

1. **端到端测试**
   - 通过 IPC 调用 LLM capability
   - 测试配置文件的读写

## 5. 验证步骤

```bash
# 格式检查
cargo fmt --all --check

# 测试
cargo test -p kunkka-core --test llm_provider

# Clippy
cargo clippy -p kunkka-core -- -D warnings

# 全量测试
cargo test --workspace
```
