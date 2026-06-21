use kunkka_core::llm::config::ConfigLoader;
use kunkka_core::llm::provider::ProviderManager;
use kunkka_core::llm::types::*;
use kunkka_core::xdg::KunkkaPaths;
use std::collections::HashMap;
use tempfile::tempdir;

fn test_paths() -> (tempfile::TempDir, KunkkaPaths) {
    let root = tempdir().unwrap();
    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };
    (root, paths)
}

#[test]
fn test_load_empty_providers_config() {
    let (_root, paths) = test_paths();
    let loader = ConfigLoader::new(&paths);

    let config = loader.load_providers().unwrap();
    assert!(config.providers.is_empty());
}

#[test]
fn test_load_providers_config() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    let config_content = r#"{
        "providers": {
            "openai": {
                "provider_type": "api_key",
                "base_url": "https://api.openai.com/v1",
                "api_key": "sk-test",
                "available_models": ["gpt-4o", "gpt-4o-mini"]
            }
        }
    }"#;

    std::fs::write(paths.config_dir.join("llm-providers.json"), config_content).unwrap();

    let loader = ConfigLoader::new(&paths);
    let config = loader.load_providers().unwrap();

    assert_eq!(config.providers.len(), 1);
    assert!(config.providers.contains_key("openai"));

    let openai = config.providers.get("openai").unwrap();
    assert_eq!(openai.provider_type, ProviderType::ApiKey);
    assert_eq!(openai.base_url, "https://api.openai.com/v1");
    assert_eq!(openai.available_models, vec!["gpt-4o", "gpt-4o-mini"]);
}

#[test]
fn test_save_and_load_providers_config() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    let loader = ConfigLoader::new(&paths);

    let mut providers = HashMap::new();
    providers.insert(
        "openai".to_string(),
        ProviderConfig {
            provider_type: ProviderType::ApiKey,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: Some("sk-test".to_string()),
            available_models: vec!["gpt-4o".to_string()],
            rate_limit: None,
            auth_method: None,
            credentials: None,
        },
    );

    let config = LlmConfig { providers };
    loader.save_providers(&config).unwrap();

    let loaded_config = loader.load_providers().unwrap();
    assert_eq!(loaded_config.providers.len(), 1);
    assert!(loaded_config.providers.contains_key("openai"));
}

#[test]
fn test_load_empty_roles_config() {
    let (_root, paths) = test_paths();
    let loader = ConfigLoader::new(&paths);

    let config = loader.load_roles().unwrap();
    assert!(config.roles.is_empty());
}

#[test]
fn test_load_roles_config() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    let config_content = r#"{
        "roles": {
            "thinker": {
                "description": "深度思考角色",
                "provider": "openai",
                "model": "gpt-4o",
                "parameters": {
                    "temperature": 0.7,
                    "max_tokens": 4096
                }
            }
        }
    }"#;

    std::fs::write(paths.config_dir.join("llm-roles.json"), config_content).unwrap();

    let loader = ConfigLoader::new(&paths);
    let config = loader.load_roles().unwrap();

    assert_eq!(config.roles.len(), 1);
    assert!(config.roles.contains_key("thinker"));

    let thinker = config.roles.get("thinker").unwrap();
    assert_eq!(thinker.description, "深度思考角色");
    assert_eq!(thinker.provider, "openai");
    assert_eq!(thinker.model, "gpt-4o");
}

#[test]
fn test_provider_manager_from_config() {
    let mut providers = HashMap::new();
    providers.insert(
        "openai".to_string(),
        ProviderConfig {
            provider_type: ProviderType::ApiKey,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: Some("sk-test".to_string()),
            available_models: vec!["gpt-4o".to_string(), "gpt-4o-mini".to_string()],
            rate_limit: None,
            auth_method: None,
            credentials: None,
        },
    );

    providers.insert(
        "ollama".to_string(),
        ProviderConfig {
            provider_type: ProviderType::Local,
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: None,
            available_models: vec!["llama2".to_string()],
            rate_limit: None,
            auth_method: None,
            credentials: None,
        },
    );

    let manager = ProviderManager::from_config(&providers).unwrap();

    let provider_list = manager.list_providers();
    assert_eq!(provider_list.len(), 2);
    assert!(provider_list.contains(&"openai".to_string()));
    assert!(provider_list.contains(&"ollama".to_string()));
}

#[test]
fn test_provider_manager_list_models() {
    let mut providers = HashMap::new();
    providers.insert(
        "openai".to_string(),
        ProviderConfig {
            provider_type: ProviderType::ApiKey,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: Some("sk-test".to_string()),
            available_models: vec!["gpt-4o".to_string(), "gpt-4o-mini".to_string()],
            rate_limit: None,
            auth_method: None,
            credentials: None,
        },
    );

    let manager = ProviderManager::from_config(&providers).unwrap();
    let models = manager.list_available_models();

    assert_eq!(models.len(), 2);
    assert!(models.contains(&("openai".to_string(), "gpt-4o".to_string())));
    assert!(models.contains(&("openai".to_string(), "gpt-4o-mini".to_string())));
}

#[test]
fn test_provider_adapter_is_model_available() {
    let config = ProviderConfig {
        provider_type: ProviderType::ApiKey,
        base_url: "https://api.openai.com/v1".to_string(),
        api_key: Some("sk-test".to_string()),
        available_models: vec!["gpt-4o".to_string()],
        rate_limit: None,
        auth_method: None,
        credentials: None,
    };

    let adapter = kunkka_core::llm::provider::ProviderAdapter::new("openai", &config).unwrap();

    assert!(adapter.is_model_available("gpt-4o"));
    assert!(!adapter.is_model_available("gpt-4"));
}
