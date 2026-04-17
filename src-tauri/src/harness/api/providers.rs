//! LLM Provider configuration and detection

use serde::{Deserialize, Serialize};

/// Model alias resolution - converts short aliases to full model names
pub fn resolve_model_alias(alias: &str) -> String {
    match alias.to_lowercase().as_str() {
        // Anthropic model aliases
        "opus" | "claude-opus-4-6" => "claude-opus-4-6".to_string(),
        "sonnet" | "claude-sonnet-4-6" => "claude-sonnet-4-6".to_string(),
        "haiku" | "claude-haiku-4-5" | "claude-haiku-4-5-20251213" => "claude-haiku-4-5-20251213".to_string(),
        // Legacy alias
        "claude-3-5-sonnet" | "claude-3-5-sonnet-latest" => "claude-sonnet-4-6".to_string(),
        "claude-3-opus" => "claude-opus-4-6".to_string(),
        "claude-3-haiku" => "claude-haiku-4-5-20251213".to_string(),
        // Return as-is if no match
        other => other.to_string(),
    }
}

/// Get all available model aliases
pub fn get_model_aliases() -> Vec<(&'static str, &'static str)> {
    vec![
        ("opus", "claude-opus-4-6"),
        ("sonnet", "claude-sonnet-4-6"),
        ("haiku", "claude-haiku-4-5-20251213"),
    ]
}

/// Check if a string is a model alias
pub fn is_model_alias(alias: &str) -> bool {
    matches!(alias.to_lowercase().as_str(), "opus" | "sonnet" | "haiku")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    XAI,
}

impl LlmProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::XAI => "xai",
        }
    }

    pub fn base_url(&self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::OpenAI => "https://api.openai.com",
            Self::XAI => "https://api.x.ai",
        }
    }
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: u32,
    pub temperature: Option<f64>,
    /// Number of retries on API error
    #[serde(default)]
    pub retry_count: Option<u32>,
    /// Language for LLM responses
    #[serde(default)]
    pub response_language: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
            temperature: None,
            retry_count: Some(3),
            response_language: None,
        }
    }
}

impl LlmConfig {
    pub fn from_env() -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .or_else(|_| std::env::var("XAI_API_KEY"))
            .ok();

        let provider = if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            LlmProvider::Anthropic
        } else if std::env::var("XAI_API_KEY").is_ok() {
            LlmProvider::XAI
        } else {
            LlmProvider::OpenAI
        };

        Self {
            provider,
            model: std::env::var("CLAUDE_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
            api_key,
            base_url: None,
            max_tokens: 4096,
            temperature: None,
            retry_count: Some(3),
            response_language: None,
        }
    }

    pub fn base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| self.provider.base_url().to_string())
    }
}
