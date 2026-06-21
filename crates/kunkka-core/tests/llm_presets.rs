use kunkka_core::llm::presets::{
    all_presets, create_provider_from_preset, get_preset, list_preset_names,
};
use kunkka_core::llm::types::ProviderType;

#[test]
fn test_all_presets_count() {
    let presets = all_presets();
    assert_eq!(presets.len(), 4);
}

#[test]
fn test_list_preset_names() {
    let names = list_preset_names();
    assert_eq!(names.len(), 4);
    assert!(names.iter().any(|(name, _)| *name == "openai"));
    assert!(names.iter().any(|(name, _)| *name == "zhipu"));
    assert!(names.iter().any(|(name, _)| *name == "kimi"));
    assert!(names.iter().any(|(name, _)| *name == "xiaomi"));
}

#[test]
fn test_get_preset_openai() {
    let preset = get_preset("openai").unwrap();
    assert_eq!(preset.name, "openai");
    assert_eq!(preset.display_name, "OpenAI");
    assert_eq!(preset.config.provider_type, ProviderType::ApiKey);
    assert_eq!(preset.config.base_url, "https://api.openai.com/v1");
    assert!(preset
        .config
        .available_models
        .contains(&"gpt-4o".to_string()));
}

#[test]
fn test_get_preset_zhipu() {
    let preset = get_preset("zhipu").unwrap();
    assert_eq!(preset.name, "zhipu");
    assert_eq!(preset.display_name, "智谱 AI");
    assert_eq!(
        preset.config.base_url,
        "https://open.bigmodel.cn/api/paas/v4"
    );
    assert!(preset
        .config
        .available_models
        .contains(&"glm-4".to_string()));
}

#[test]
fn test_get_preset_kimi() {
    let preset = get_preset("kimi").unwrap();
    assert_eq!(preset.name, "kimi");
    assert_eq!(preset.display_name, "Kimi (月之暗面)");
    assert_eq!(preset.config.base_url, "https://api.moonshot.cn/v1");
    assert!(preset
        .config
        .available_models
        .contains(&"moonshot-v1-8k".to_string()));
}

#[test]
fn test_get_preset_xiaomi() {
    let preset = get_preset("xiaomi").unwrap();
    assert_eq!(preset.name, "xiaomi");
    assert_eq!(preset.display_name, "小米 MiMo");
    assert_eq!(preset.config.base_url, "https://api.xiaomi.com/v1");
    assert!(preset
        .config
        .available_models
        .contains(&"mimo-7b".to_string()));
}

#[test]
fn test_get_preset_not_found() {
    let preset = get_preset("nonexistent");
    assert!(preset.is_none());
}

#[test]
fn test_create_provider_from_preset() {
    let (name, config) = create_provider_from_preset("openai", "sk-test-key").unwrap();
    assert_eq!(name, "openai");
    assert_eq!(config.api_key, Some("sk-test-key".to_string()));
    assert_eq!(config.base_url, "https://api.openai.com/v1");
    assert!(config.available_models.contains(&"gpt-4o".to_string()));
}

#[test]
fn test_create_provider_from_preset_not_found() {
    let result = create_provider_from_preset("nonexistent", "key");
    assert!(result.is_none());
}

#[test]
fn test_preset_configs_have_no_api_key() {
    let presets = all_presets();
    for preset in presets {
        assert!(
            preset.config.api_key.is_none(),
            "Preset {} should not have api_key",
            preset.name
        );
    }
}

#[test]
fn test_preset_configs_have_correct_type() {
    let presets = all_presets();
    for preset in presets {
        assert_eq!(preset.config.provider_type, ProviderType::ApiKey);
    }
}
