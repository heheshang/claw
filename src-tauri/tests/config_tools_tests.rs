//! Config and tools tests

use ssk_lib::{Config, ProviderSettings};

#[test]
fn test_provider_settings_default() {
    let settings = ProviderSettings::default();
    assert!(settings.api_key.is_none());
    assert!(settings.base_url.is_none());
    assert!(settings.model.is_none());
    assert!(settings.max_tokens.is_none());
    assert!(settings.temperature.is_none());
    assert_eq!(settings.retry_count, Some(3));
    assert!(settings.response_language.is_none());
}

#[test]
fn test_provider_settings_with_values() {
    let settings = ProviderSettings {
        api_key: Some("sk-test".to_string()),
        base_url: Some("https://api.example.com".to_string()),
        model: Some("claude-3".to_string()),
        max_tokens: Some(4096),
        temperature: Some(0.7),
        retry_count: Some(5),
        response_language: Some("中文".to_string()),
    };

    assert_eq!(settings.api_key, Some("sk-test".to_string()));
    assert_eq!(settings.base_url, Some("https://api.example.com".to_string()));
    assert_eq!(settings.model, Some("claude-3".to_string()));
    assert_eq!(settings.max_tokens, Some(4096));
    assert_eq!(settings.temperature, Some(0.7));
    assert_eq!(settings.retry_count, Some(5));
    assert_eq!(settings.response_language, Some("中文".to_string()));
}

#[test]
fn test_config_default() {
    let config = Config::default();
    assert!(config.model.is_none());
    assert!(config.providers.anthropic.api_key.is_none());
    assert!(!config.permissions.allow_dangerous);
    assert!(config.permissions.allowed_dirs.is_empty());
}

#[test]
fn test_config_get_api_key() {
    let mut config = Config::default();
    config.providers.anthropic.api_key = Some("sk-anthropic".to_string());
    config.providers.openai.api_key = Some("sk-openai".to_string());
    config.providers.xai.api_key = Some("sk-xai".to_string());

    assert_eq!(config.get_api_key("anthropic"), Some("sk-anthropic".to_string()));
    assert_eq!(config.get_api_key("openai"), Some("sk-openai".to_string()));
    assert_eq!(config.get_api_key("xai"), Some("sk-xai".to_string()));
    assert_eq!(config.get_api_key("unknown"), None);
}

#[test]
fn test_config_get_model() {
    let mut config = Config::default();
    config.providers.anthropic.model = Some("claude-opus-4".to_string());

    assert_eq!(config.get_model("anthropic", "default-model"), "claude-opus-4");
    assert_eq!(config.get_model("unknown", "default-model"), "default-model");
}
