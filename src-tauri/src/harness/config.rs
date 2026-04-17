//! Configuration management following claw-code style
//!
//! Configuration hierarchy:
//! - User: ~/.claw/settings.json
//! - Project: .claw/settings.json
//! - Local: .claw/settings.local.json (gitignored)
//!
//! Environment variables override config file settings.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration entry with source tracking
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    pub source: ConfigSource,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    User,
    Project,
    Local,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// LLM model to use
    #[serde(default)]
    pub model: Option<String>,

    /// API keys for providers
    #[serde(default)]
    pub env: BTreeMap<String, String>,

    /// Provider configuration
    #[serde(default)]
    pub providers: ProviderConfig,

    /// Permission settings
    #[serde(default)]
    pub permissions: PermissionConfig,

    /// Tool aliases
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ProviderConfig {
    #[serde(default)]
    pub anthropic: ProviderSettings,
    #[serde(default)]
    pub openai: ProviderSettings,
    #[serde(default)]
    pub xai: ProviderSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    /// Number of retries on API error (default: 3)
    #[serde(default)]
    pub retry_count: Option<u32>,
    /// Language for LLM responses (e.g., "中文", "English")
    #[serde(default)]
    pub response_language: Option<String>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            model: None,
            max_tokens: None,
            temperature: None,
            retry_count: Some(3),
            response_language: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionConfig {
    /// Allow dangerous operations like file writes
    #[serde(default)]
    pub allow_dangerous: bool,
    /// Allowed directories (empty = all)
    #[serde(default)]
    pub allowed_dirs: Vec<String>,
}

impl Config {
    /// Load configuration from discovered paths
    /// Uses workspace_root as the primary directory for project-level configs
    pub fn load() -> Self {
        Self::load_with_root(std::env::current_dir().ok())
    }

    /// Load configuration with explicit root directory
    pub fn load_with_root(root: Option<PathBuf>) -> Self {
        let root = root.unwrap_or_else(|| PathBuf::from("."));
        let entries = Self::discover_entries_with_root(&root);
        let mut merged = Config::default();

        for entry in entries {
            if entry.path.exists() {
                if let Ok(content) = fs::read_to_string(&entry.path) {
                    if let Ok(file_config) = serde_json::from_str::<Config>(&content) {
                        merged = Self::deep_merge(merged, file_config);
                    }
                }
            }
        }

        // Environment variables override file config
        merged.apply_env_overrides();
        merged
    }

    /// Discover configuration file paths
    fn discover_entries_with_root(root: &Path) -> Vec<ConfigEntry> {
        let config_home = Self::config_home();

        vec![
            // User-level legacy
            ConfigEntry {
                source: ConfigSource::User,
                path: dirs::home_dir()
                    .map(|h| h.join(".claw.json"))
                    .unwrap_or_else(|| PathBuf::from(".claw.json")),
            },
            // User-level settings
            ConfigEntry {
                source: ConfigSource::User,
                path: config_home.join("settings.json"),
            },
            // Project-level legacy
            ConfigEntry {
                source: ConfigSource::Project,
                path: root.join(".claw.json"),
            },
            // Project-level settings
            ConfigEntry {
                source: ConfigSource::Project,
                path: root.join(".claw").join("settings.json"),
            },
            // Local settings (gitignored)
            ConfigEntry {
                source: ConfigSource::Local,
                path: root.join(".claw").join("settings.local.json"),
            },
        ]
    }

    /// Get config home directory
    fn config_home() -> PathBuf {
        std::env::var_os("CLAW_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".claw")))
            .unwrap_or_else(|| PathBuf::from(".claw"))
    }

    /// Deep merge two configs (later overrides earlier)
    fn deep_merge(mut base: Config, override_: Config) -> Config {
        if let Some(model) = override_.model {
            base.model = Some(model);
        }

        for (key, value) in override_.env {
            base.env.insert(key, value);
        }

        // Merge provider settings
        if override_.providers.anthropic.api_key.is_some() {
            base.providers.anthropic.api_key = override_.providers.anthropic.api_key;
        }
        if override_.providers.anthropic.base_url.is_some() {
            base.providers.anthropic.base_url = override_.providers.anthropic.base_url;
        }
        if override_.providers.anthropic.model.is_some() {
            base.providers.anthropic.model = override_.providers.anthropic.model;
        }
        if override_.providers.anthropic.max_tokens.is_some() {
            base.providers.anthropic.max_tokens = override_.providers.anthropic.max_tokens;
        }
        if override_.providers.anthropic.temperature.is_some() {
            base.providers.anthropic.temperature = override_.providers.anthropic.temperature;
        }
        if override_.providers.anthropic.retry_count.is_some() {
            base.providers.anthropic.retry_count = override_.providers.anthropic.retry_count;
        }
        if override_.providers.anthropic.response_language.is_some() {
            base.providers.anthropic.response_language = override_.providers.anthropic.response_language;
        }

        // Merge other providers (anthropic already done above, now do openai and xai)
        if override_.providers.openai.api_key.is_some() {
            base.providers.openai.api_key = override_.providers.openai.api_key;
        }
        if override_.providers.openai.base_url.is_some() {
            base.providers.openai.base_url = override_.providers.openai.base_url;
        }
        if override_.providers.openai.model.is_some() {
            base.providers.openai.model = override_.providers.openai.model;
        }
        if override_.providers.openai.max_tokens.is_some() {
            base.providers.openai.max_tokens = override_.providers.openai.max_tokens;
        }
        if override_.providers.openai.temperature.is_some() {
            base.providers.openai.temperature = override_.providers.openai.temperature;
        }
        if override_.providers.openai.retry_count.is_some() {
            base.providers.openai.retry_count = override_.providers.openai.retry_count;
        }
        if override_.providers.openai.response_language.is_some() {
            base.providers.openai.response_language = override_.providers.openai.response_language;
        }

        if override_.providers.xai.api_key.is_some() {
            base.providers.xai.api_key = override_.providers.xai.api_key;
        }
        if override_.providers.xai.base_url.is_some() {
            base.providers.xai.base_url = override_.providers.xai.base_url;
        }
        if override_.providers.xai.model.is_some() {
            base.providers.xai.model = override_.providers.xai.model;
        }
        if override_.providers.xai.max_tokens.is_some() {
            base.providers.xai.max_tokens = override_.providers.xai.max_tokens;
        }
        if override_.providers.xai.temperature.is_some() {
            base.providers.xai.temperature = override_.providers.xai.temperature;
        }
        if override_.providers.xai.retry_count.is_some() {
            base.providers.xai.retry_count = override_.providers.xai.retry_count;
        }
        if override_.providers.xai.response_language.is_some() {
            base.providers.xai.response_language = override_.providers.xai.response_language;
        }

        if override_.permissions.allow_dangerous {
            base.permissions.allow_dangerous = true;
        }
        for dir in override_.permissions.allowed_dirs {
            if !base.permissions.allowed_dirs.contains(&dir) {
                base.permissions.allowed_dirs.push(dir);
            }
        }

        for (alias, cmd) in override_.aliases {
            base.aliases.insert(alias, cmd);
        }

        base
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        // ANTHROPIC_API_KEY
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            self.providers.anthropic.api_key = Some(key);
        }
        // OPENAI_API_KEY
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            self.providers.openai.api_key = Some(key);
        }
        // XAI_API_KEY
        if let Ok(key) = std::env::var("XAI_API_KEY") {
            self.providers.xai.api_key = Some(key);
        }
        // CLAUDE_MODEL
        if let Ok(model) = std::env::var("CLAUDE_MODEL") {
            self.model = Some(model);
        }
        // CLAW_PROVIDER
        if let Ok(_provider) = std::env::var("CLAW_PROVIDER") {
            // Will be used when resolving active provider
        }
    }

    /// Get effective API key for a provider
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        match provider {
            "anthropic" => self.providers.anthropic.api_key.clone(),
            "openai" => self.providers.openai.api_key.clone(),
            "xai" => self.providers.xai.api_key.clone(),
            _ => None,
        }
    }

    /// Get effective model for a provider
    pub fn get_model(&self, provider: &str, default: &str) -> String {
        let model = match provider {
            "anthropic" => self.providers.anthropic.model.clone(),
            "openai" => self.providers.openai.model.clone(),
            "xai" => self.providers.xai.model.clone(),
            _ => None,
        };
        model.unwrap_or_else(|| default.to_string())
    }
}

