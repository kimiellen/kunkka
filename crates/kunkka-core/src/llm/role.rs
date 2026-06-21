use crate::llm::config::ConfigLoader;
use crate::llm::provider::ProviderManager;
use crate::llm::types::*;
use crate::{CoreError, Result};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 角色管理器
pub struct RoleManager {
    config_loader: ConfigLoader,
    roles_config: Arc<RwLock<RolesConfig>>,
    provider_manager: Arc<RwLock<Option<ProviderManager>>>,
}

impl RoleManager {
    /// 创建新的角色管理器
    pub fn new(
        config_loader: ConfigLoader,
        provider_manager: Arc<RwLock<Option<ProviderManager>>>,
    ) -> Self {
        Self {
            config_loader,
            roles_config: Arc::new(RwLock::new(RolesConfig {
                roles: std::collections::HashMap::new(),
                default_role: None,
            })),
            provider_manager,
        }
    }

    /// 初始化：加载角色配置
    pub async fn initialize(&self) -> Result<()> {
        let roles_config = self.config_loader.load_roles()?;
        *self.roles_config.write().await = roles_config;
        Ok(())
    }

    /// 获取角色配置
    pub async fn get_role(&self, role_name: &str) -> Option<RoleConfig> {
        let roles = self.roles_config.read().await;
        roles.roles.get(role_name).cloned()
    }

    /// 列出所有角色名称
    pub async fn list_roles(&self) -> Vec<String> {
        let roles = self.roles_config.read().await;
        roles.roles.keys().cloned().collect()
    }

    /// 添加角色
    pub async fn add_role(&self, name: String, config: RoleConfig) -> Result<()> {
        // 验证供应商和模型是否存在
        let provider_manager = self.provider_manager.read().await;
        if let Some(manager) = provider_manager.as_ref() {
            if let Some(adapter) = manager.get(&config.provider) {
                if !adapter.is_model_available(&config.model) {
                    return Err(CoreError::Config(format!(
                        "Model '{}' not available in provider '{}'",
                        config.model, config.provider
                    )));
                }
            } else {
                return Err(CoreError::Config(format!(
                    "Provider '{}' not found",
                    config.provider
                )));
            }
        }

        // 更新配置
        let mut roles_config = self.roles_config.write().await;
        roles_config.roles.insert(name, config);

        // 保存到文件
        self.config_loader.save_roles(&roles_config)?;

        Ok(())
    }

    /// 删除角色
    pub async fn remove_role(&self, name: &str) -> Result<()> {
        let mut roles_config = self.roles_config.write().await;
        roles_config.roles.remove(name);

        // 保存到文件
        self.config_loader.save_roles(&roles_config)?;

        Ok(())
    }

    /// 解析角色：返回供应商名称和模型名称
    /// 如果 role_name 为空或 "default"，使用默认角色
    pub async fn resolve_role(&self, role_name: &str) -> Result<(String, String, ModelParameters)> {
        let roles = self.roles_config.read().await;

        let effective_name = if role_name.is_empty() || role_name == "default" {
            roles.default_role.as_deref().unwrap_or(role_name)
        } else {
            role_name
        };

        let role = roles
            .roles
            .get(effective_name)
            .ok_or_else(|| CoreError::Config(format!("Role '{}' not found", effective_name)))?;

        Ok((
            role.provider.clone(),
            role.model.clone(),
            role.parameters.clone(),
        ))
    }

    /// 设置默认角色
    pub async fn set_default_role(&self, name: Option<String>) -> Result<()> {
        // 验证角色存在（如果指定了名称）
        if let Some(role_name) = &name {
            let roles = self.roles_config.read().await;
            if !roles.roles.contains_key(role_name) {
                return Err(CoreError::Config(format!("Role '{}' not found", role_name)));
            }
        }

        let mut roles = self.roles_config.write().await;
        roles.default_role = name;

        // 保存到文件
        self.config_loader.save_roles(&roles)?;

        Ok(())
    }

    /// 获取默认角色名称
    pub async fn get_default_role(&self) -> Option<String> {
        let roles = self.roles_config.read().await;
        roles.default_role.clone()
    }

    /// 获取角色对应的供应商适配器
    pub async fn get_provider_for_role(&self, role_name: &str) -> Result<String> {
        let roles = self.roles_config.read().await;
        let role = roles
            .roles
            .get(role_name)
            .ok_or_else(|| CoreError::Config(format!("Role '{}' not found", role_name)))?;

        Ok(role.provider.clone())
    }
}
