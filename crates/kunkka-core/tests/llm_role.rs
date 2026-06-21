use kunkka_core::llm::config::ConfigLoader;
use kunkka_core::llm::provider::ProviderManager;
use kunkka_core::llm::role::RoleManager;
use kunkka_core::llm::types::*;
use kunkka_core::xdg::KunkkaPaths;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;

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

fn create_test_providers() -> HashMap<String, ProviderConfig> {
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
    providers
}

#[tokio::test]
async fn test_role_manager_initialize_empty() {
    let (_root, paths) = test_paths();
    let config_loader = ConfigLoader::new(&paths);
    let provider_manager = Arc::new(RwLock::new(None));

    let role_manager = RoleManager::new(config_loader, provider_manager);
    role_manager.initialize().await.unwrap();

    let roles = role_manager.list_roles().await;
    assert!(roles.is_empty());
}

#[tokio::test]
async fn test_role_manager_load_roles() {
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
            },
            "coder": {
                "description": "编码角色",
                "provider": "openai",
                "model": "gpt-4o-mini",
                "parameters": {
                    "temperature": 0.3,
                    "max_tokens": 8192
                }
            }
        }
    }"#;

    std::fs::write(paths.config_dir.join("llm-roles.json"), config_content).unwrap();

    let config_loader = ConfigLoader::new(&paths);
    let provider_manager = Arc::new(RwLock::new(None));

    let role_manager = RoleManager::new(config_loader, provider_manager);
    role_manager.initialize().await.unwrap();

    let roles = role_manager.list_roles().await;
    assert_eq!(roles.len(), 2);
    assert!(roles.contains(&"thinker".to_string()));
    assert!(roles.contains(&"coder".to_string()));
}

#[tokio::test]
async fn test_role_manager_get_role() {
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

    let config_loader = ConfigLoader::new(&paths);
    let provider_manager = Arc::new(RwLock::new(None));

    let role_manager = RoleManager::new(config_loader, provider_manager);
    role_manager.initialize().await.unwrap();

    let role = role_manager.get_role("thinker").await;
    assert!(role.is_some());

    let role = role.unwrap();
    assert_eq!(role.description, "深度思考角色");
    assert_eq!(role.provider, "openai");
    assert_eq!(role.model, "gpt-4o");
}

#[tokio::test]
async fn test_role_manager_add_role() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    let config_loader = ConfigLoader::new(&paths);
    let providers = create_test_providers();
    let provider_manager = Arc::new(RwLock::new(Some(
        ProviderManager::from_config(&providers).unwrap(),
    )));

    let role_manager = RoleManager::new(config_loader, provider_manager);
    role_manager.initialize().await.unwrap();

    let new_role = RoleConfig {
        description: "测试角色".to_string(),
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        parameters: ModelParameters {
            temperature: Some(0.5),
            max_tokens: Some(2048),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
        },
    };

    role_manager
        .add_role("test_role".to_string(), new_role)
        .await
        .unwrap();

    let roles = role_manager.list_roles().await;
    assert_eq!(roles.len(), 1);
    assert!(roles.contains(&"test_role".to_string()));
}

#[tokio::test]
async fn test_role_manager_remove_role() {
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

    let config_loader = ConfigLoader::new(&paths);
    let provider_manager = Arc::new(RwLock::new(None));

    let role_manager = RoleManager::new(config_loader, provider_manager);
    role_manager.initialize().await.unwrap();

    role_manager.remove_role("thinker").await.unwrap();

    let roles = role_manager.list_roles().await;
    assert!(roles.is_empty());
}

#[tokio::test]
async fn test_role_manager_resolve_role() {
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

    let config_loader = ConfigLoader::new(&paths);
    let provider_manager = Arc::new(RwLock::new(None));

    let role_manager = RoleManager::new(config_loader, provider_manager);
    role_manager.initialize().await.unwrap();

    let (provider, model, params) = role_manager.resolve_role("thinker").await.unwrap();
    assert_eq!(provider, "openai");
    assert_eq!(model, "gpt-4o");
    assert_eq!(params.temperature, Some(0.7));
    assert_eq!(params.max_tokens, Some(4096));
}

#[tokio::test]
async fn test_role_manager_resolve_nonexistent_role() {
    let (_root, paths) = test_paths();
    let config_loader = ConfigLoader::new(&paths);
    let provider_manager = Arc::new(RwLock::new(None));

    let role_manager = RoleManager::new(config_loader, provider_manager);
    role_manager.initialize().await.unwrap();

    let result = role_manager.resolve_role("nonexistent").await;
    assert!(result.is_err());
}
