use super::types::{LlmConfig, RolesConfig};
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use std::fs;
use std::path::PathBuf;

/// 配置加载器
#[derive(Clone)]
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
                default_role: None,
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
