use kunkka_core::app_manifest::{
    AppManifest, AppPermissions, CapabilitiesConfig, LlmCapabilityConfig, WorkerCommand,
};
use kunkka_core::capability::llm::*;
use kunkka_core::llm::config::ConfigLoader;
use kunkka_core::llm::types::*;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_worker_sdk::AppId;
use std::collections::{BTreeMap, HashMap};
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

fn manifest_with_llm_roles(roles: Vec<&str>) -> AppManifest {
    AppManifest {
        app_id: AppId::new("test-app"),
        worker: WorkerCommand {
            program: "/bin/true".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        },
        permissions: AppPermissions::default(),
        capabilities: CapabilitiesConfig {
            fs: None,
            shell: None,
            http: None,
            llm: Some(LlmCapabilityConfig {
                roles: roles.into_iter().map(String::from).collect(),
            }),
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

#[tokio::test]
async fn test_llm_state_initialize() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let roles = state.role_manager.list_roles().await;
    assert!(roles.is_empty());
}

#[tokio::test]
async fn test_handle_list_providers() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    // 创建供应商配置
    let providers = create_test_providers();
    let config = LlmConfig { providers };
    let config_loader = ConfigLoader::new(&paths);
    config_loader.save_providers(&config).unwrap();

    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let params = postcard::to_stdvec(&()).unwrap();
    let manifest = manifest_with_llm_roles(vec!["thinker"]);
    let response = handle_llm_request(&manifest, "list_providers", &params, &state)
        .await
        .unwrap();
    let llm_response: LlmResponse = postcard::from_bytes(&response).unwrap();

    match llm_response {
        LlmResponse::Providers(providers) => {
            assert_eq!(providers.len(), 1);
            assert!(providers.contains(&"openai".to_string()));
        }
        _ => panic!("Expected Providers response"),
    }
}

#[tokio::test]
async fn test_handle_list_models() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    // 创建供应商配置
    let providers = create_test_providers();
    let config = LlmConfig { providers };
    let config_loader = ConfigLoader::new(&paths);
    config_loader.save_providers(&config).unwrap();

    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let params = postcard::to_stdvec(&()).unwrap();
    let manifest = manifest_with_llm_roles(vec!["thinker"]);
    let response = handle_llm_request(&manifest, "list_models", &params, &state)
        .await
        .unwrap();
    let llm_response: LlmResponse = postcard::from_bytes(&response).unwrap();

    match llm_response {
        LlmResponse::Models(models) => {
            assert_eq!(models.len(), 2);
            assert!(models.contains(&("openai".to_string(), "gpt-4o".to_string())));
            assert!(models.contains(&("openai".to_string(), "gpt-4o-mini".to_string())));
        }
        _ => panic!("Expected Models response"),
    }
}

#[tokio::test]
async fn test_handle_list_roles() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    // 创建角色配置
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

    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let params = postcard::to_stdvec(&()).unwrap();
    let manifest = manifest_with_llm_roles(vec!["thinker"]);
    let response = handle_llm_request(&manifest, "list_roles", &params, &state)
        .await
        .unwrap();
    let llm_response: LlmResponse = postcard::from_bytes(&response).unwrap();

    match llm_response {
        LlmResponse::Roles(roles) => {
            assert_eq!(roles.len(), 1);
            assert!(roles.contains(&"thinker".to_string()));
        }
        _ => panic!("Expected Roles response"),
    }
}

#[tokio::test]
async fn test_handle_chat() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    // 创建供应商配置
    let providers = create_test_providers();
    let config = LlmConfig { providers };
    let config_loader = ConfigLoader::new(&paths);
    config_loader.save_providers(&config).unwrap();

    // 创建角色配置
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

    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let chat_params = LlmChatParams {
        role: "thinker".to_string(),
        messages: vec![LlmMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }],
        stream: None,
        temperature: None,
        max_tokens: None,
    };

    let params = postcard::to_stdvec(&chat_params).unwrap();
    let manifest = manifest_with_llm_roles(vec!["thinker"]);
    let result = handle_llm_request(&manifest, "chat", &params, &state).await;

    // 由于 API key 无效，预期会失败
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "llm_error");
    assert!(err.message.contains("401 Unauthorized"));
}

#[tokio::test]
async fn test_handle_unknown_method() {
    let (_root, paths) = test_paths();
    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let params = postcard::to_stdvec(&()).unwrap();
    let manifest = manifest_with_llm_roles(vec!["thinker"]);
    let result = handle_llm_request(&manifest, "unknown", &params, &state).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_llm_denied_when_not_configured() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let manifest = AppManifest {
        app_id: AppId::new("test-app"),
        worker: WorkerCommand {
            program: "/bin/true".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        },
        permissions: AppPermissions::default(),
        capabilities: CapabilitiesConfig::default(),
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    };

    let params = postcard::to_stdvec(&()).unwrap();
    let result = handle_llm_request(&manifest, "list_providers", &params, &state).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
    assert!(err.message.contains("not configured"));
}

#[tokio::test]
async fn test_llm_denied_when_role_not_in_list() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    let providers = create_test_providers();
    let config = LlmConfig { providers };
    let config_loader = ConfigLoader::new(&paths);
    config_loader.save_providers(&config).unwrap();

    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let manifest = manifest_with_llm_roles(vec!["coder"]);

    let chat_params = LlmChatParams {
        role: "thinker".to_string(),
        messages: vec![LlmMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }],
        stream: None,
        temperature: None,
        max_tokens: None,
    };

    let params = postcard::to_stdvec(&chat_params).unwrap();
    let result = handle_llm_request(&manifest, "chat", &params, &state).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
    assert!(err.message.contains("not allowed"));
}

#[tokio::test]
async fn test_llm_denied_when_roles_list_empty() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(&paths.config_dir).unwrap();

    let providers = create_test_providers();
    let config = LlmConfig { providers };
    let config_loader = ConfigLoader::new(&paths);
    config_loader.save_providers(&config).unwrap();

    let state = LlmState::new(&paths);
    state.initialize().await.unwrap();

    let manifest = manifest_with_llm_roles(vec![]);

    let chat_params = LlmChatParams {
        role: "thinker".to_string(),
        messages: vec![LlmMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }],
        stream: None,
        temperature: None,
        max_tokens: None,
    };

    let params = postcard::to_stdvec(&chat_params).unwrap();
    let result = handle_llm_request(&manifest, "chat", &params, &state).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
    assert!(err.message.contains("no allowed roles"));
}
