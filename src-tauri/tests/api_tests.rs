//! API client tests

use ssk_lib::{LlmConfig, LlmProvider};

#[test]
fn test_llm_config_default() {
    let config = LlmConfig::default();
    assert!(matches!(config.provider, LlmProvider::Anthropic));
    assert_eq!(config.model, "claude-sonnet-4-20250514");
    assert!(config.api_key.is_none());
    assert!(config.base_url.is_none());
    assert_eq!(config.max_tokens, 4096);
    assert!(config.temperature.is_none());
    assert_eq!(config.retry_count, Some(3));
    assert!(config.response_language.is_none());
}

#[test]
fn test_llm_config_with_values() {
    let config = LlmConfig {
        provider: LlmProvider::OpenAI,
        model: "gpt-4".to_string(),
        api_key: Some("sk-test".to_string()),
        base_url: Some("https://api.openai.com".to_string()),
        max_tokens: 8192,
        temperature: Some(0.5),
        retry_count: Some(5),
        response_language: Some("English".to_string()),
    };

    assert!(matches!(config.provider, LlmProvider::OpenAI));
    assert_eq!(config.model, "gpt-4");
    assert_eq!(config.api_key, Some("sk-test".to_string()));
    assert_eq!(config.base_url, Some("https://api.openai.com".to_string()));
    assert_eq!(config.max_tokens, 8192);
    assert_eq!(config.temperature, Some(0.5));
    assert_eq!(config.retry_count, Some(5));
    assert_eq!(config.response_language, Some("English".to_string()));
}

#[test]
fn test_llm_provider_base_url() {
    assert_eq!(LlmProvider::Anthropic.base_url(), "https://api.anthropic.com");
    assert_eq!(LlmProvider::OpenAI.base_url(), "https://api.openai.com");
    assert_eq!(LlmProvider::XAI.base_url(), "https://api.x.ai");
}

#[test]
fn test_llm_config_base_url() {
    let config = LlmConfig {
        provider: LlmProvider::Anthropic,
        model: "claude-3".to_string(),
        api_key: None,
        base_url: Some("https://custom.api.com".to_string()),
        max_tokens: 4096,
        temperature: None,
        retry_count: None,
        response_language: None,
    };

    assert_eq!(config.base_url(), "https://custom.api.com");
}

#[test]
fn test_llm_config_base_url_fallback() {
    let config = LlmConfig {
        provider: LlmProvider::Anthropic,
        model: "claude-3".to_string(),
        api_key: None,
        base_url: None,
        max_tokens: 4096,
        temperature: None,
        retry_count: None,
        response_language: None,
    };

    assert_eq!(config.base_url(), "https://api.anthropic.com");
}
