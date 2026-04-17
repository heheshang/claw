//! Claw-code style harness agent implementation
//!
//! Module structure (following claw-code architecture):
//! - api/       - HTTP client, SSE parsing, provider config
//! - runtime/   - Session and conversation management
//! - tools/     - Tool definitions and executor
//! - config.rs  - Configuration management
//! - daemon/    - Background task daemon with health monitoring
//! - telemetry/ - Usage tracking and cost calculation
//! - mcp/       - MCP server lifecycle management
//! - project_memory/ - CLAUDE.md project memory support
//! - plugins/   - Plugin system for extensions

use std::sync::RwLock;

pub mod api;
pub mod runtime;
pub mod tools;
pub mod daemon;
pub mod telemetry;
pub mod mcp;
pub mod project_memory;
pub mod plugins;

pub mod config;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

pub use api::{ApiClient, LlmConfig, LlmProvider, SseEvent};
pub use config::{Config, ConfigEntry, ConfigSource, PermissionConfig, ProviderConfig, ProviderSettings};
pub use daemon::{spawn_component_supervisor, spawn_background_task, spawn_periodic_task, DaemonState, HealthStatus, ComponentHealth};
pub use runtime::{build_system_prompt, ApiMessage, ContentBlock, ConversationMessage, MessageContent, MessageRole, Session, SessionManager, SessionSummary};
pub use tools::{get_tool_definitions, ToolExecutor, PermissionLevel, PermissionedToolExecutor, tool_definitions_with_permissions};
pub use telemetry::{UsageTracker, UsageStats, CostCalculator, CostEstimate, ApiCallRecord};

pub struct HarnessState {
    pub session: Mutex<Session>,
    pub session_manager: SessionManager,
    pub config: RwLock<Config>,
    pub api_client: Mutex<Option<ApiClient>>,
    pub tool_executor: Mutex<PermissionedToolExecutor>,
    pub daemon_state: Arc<DaemonState>,
    pub usage_tracker: Mutex<UsageTracker>,
}

impl HarnessState {
    pub fn new(workspace_root: PathBuf) -> Self {
        let config = Config::load_with_root(Some(workspace_root.clone()));
        let provider = "anthropic"; // Default provider

        // Try config file first, then search up directories for settings.local.json
        let api_key = config.get_api_key(provider).or_else(|| {
            let mut dir = std::env::current_dir().ok();
            for _ in 0..5 {
                if let Some(d) = dir {
                    let settings_path = d.join(".claw/settings.local.json");
                    if settings_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&settings_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(key) = json["providers"]["anthropic"]["api_key"].as_str() {
                                    if !key.is_empty() {
                                        return Some(key.to_string());
                                    }
                                }
                            }
                        }
                    }
                    dir = d.parent().map(|p| p.to_path_buf());
                } else {
                    break;
                }
            }
            None
        });

        // Also load base_url from settings.json or settings.local.json (searching up)
        let base_url = {
            let mut dir = std::env::current_dir().ok();
            let mut found_url: Option<String> = None;
            for _ in 0..5 {
                if let Some(d) = dir {
                    // Try settings.local.json first
                    let local_path = d.join(".claw/settings.local.json");
                    if local_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&local_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(url) = json["providers"]["anthropic"]["base_url"].as_str() {
                                    if !url.is_empty() {
                                        found_url = Some(url.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // Try settings.json
                    let settings_path = d.join(".claw/settings.json");
                    if settings_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&settings_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(url) = json["providers"]["anthropic"]["base_url"].as_str() {
                                    if !url.is_empty() {
                                        found_url = Some(url.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    dir = d.parent().map(|p| p.to_path_buf());
                } else {
                    break;
                }
            }
            found_url.or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        };

        // Load model from settings.json or settings.local.json (searching up)
        let model = {
            let mut dir = std::env::current_dir().ok();
            let mut found_model: Option<String> = None;
            for _ in 0..5 {
                if let Some(d) = dir {
                    // Try settings.local.json first
                    let local_path = d.join(".claw/settings.local.json");
                    if local_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&local_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(m) = json["providers"]["anthropic"]["model"].as_str() {
                                    if !m.is_empty() {
                                        found_model = Some(m.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // Try settings.json
                    let settings_path = d.join(".claw/settings.json");
                    if settings_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&settings_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(m) = json["providers"]["anthropic"]["model"].as_str() {
                                    if !m.is_empty() {
                                        found_model = Some(m.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    dir = d.parent().map(|p| p.to_path_buf());
                } else {
                    break;
                }
            }
            found_model
                .or_else(|| config.get_model(provider, "claude-sonnet-4-20250514").into())
                .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
        };

        let llm_config = LlmConfig {
            provider: LlmProvider::Anthropic,
            model: model.clone(),
            api_key: api_key.clone(),
            base_url,
            max_tokens: 4096,
            temperature: None,
            retry_count: Some(3),
            response_language: None,
        };

        let api_client = if api_key.is_some() {
            Some(ApiClient::new(llm_config))
        } else {
            None
        };

        Self {
            session: Mutex::new(Session::new_with_path(workspace_root.join(".claude/session.jsonl"))),
            session_manager: SessionManager::new(workspace_root.clone()),
            config: RwLock::new(config),
            api_client: Mutex::new(api_client),
            tool_executor: Mutex::new(PermissionedToolExecutor::new(workspace_root.clone(), PermissionLevel::DangerFullAccess)),
            daemon_state: Arc::new(DaemonState::new()),
            usage_tracker: Mutex::new(UsageTracker::new(workspace_root.join(".claude/usage.jsonl"))),
        }
    }

    /// Update the active provider and recreate API client if needed
    pub fn set_provider(&mut self, provider: &str) {
        let config = self.config.read().unwrap();
        let api_key = config.get_api_key(provider);
        let model = config.get_model(provider, "claude-sonnet-4-20250514");
        drop(config);

        let llm_provider = match provider {
            "anthropic" => LlmProvider::Anthropic,
            "openai" => LlmProvider::OpenAI,
            "xai" => LlmProvider::XAI,
            _ => LlmProvider::Anthropic,
        };

        let llm_config = LlmConfig {
            provider: llm_provider,
            model,
            api_key,
            base_url: None,
            max_tokens: 4096,
            temperature: None,
            retry_count: Some(3),
            response_language: None,
        };

        if llm_config.api_key.is_some() {
            *self.api_client.lock().unwrap() = Some(ApiClient::new(llm_config));
        } else {
            *self.api_client.lock().unwrap() = None;
        }
    }
}

// ============================================================================
// Slash Commands
// ============================================================================

#[derive(Debug, Clone)]
pub enum SlashCommand {
    Help,
    Model { name: Option<String> },
    Session { action: String },
    Compact,
    Clear,
    Status,
    Cost,
    Stats,
    Tasks,
    Review,
    SecurityReview,
    Diff,
    Commit,
    Pr,
    Init,
    Sandbox,
    Agents,
    Mcp,
    Skills,
    SystemPrompt,
    Unknown(String),
}

pub fn parse_slash_command(input: &str) -> (SlashCommand, Option<&str>) {
    if !input.starts_with('/') {
        return (SlashCommand::Unknown(input.to_string()), None);
    }

    let rest = input.trim_start_matches('/');
    let (cmd, args) = rest.split_once(' ').unwrap_or((rest, ""));

    match cmd.to_lowercase().as_str() {
        "help" => (SlashCommand::Help, None),
        "model" => (
            SlashCommand::Model {
                name: if args.is_empty() {
                    None
                } else {
                    Some(args.to_string())
                },
            },
            None,
        ),
        "session" => (SlashCommand::Session { action: args.to_string() }, None),
        "compact" => (SlashCommand::Compact, None),
        "clear" => (SlashCommand::Clear, None),
        "status" => (SlashCommand::Status, None),
        "cost" => (SlashCommand::Cost, None),
        "stats" => (SlashCommand::Stats, None),
        "tasks" => (SlashCommand::Tasks, None),
        "review" => (SlashCommand::Review, Some(args)),
        "security-review" | "security" => (SlashCommand::SecurityReview, Some(args)),
        "diff" => (SlashCommand::Diff, Some(args)),
        "commit" => (SlashCommand::Commit, Some(args)),
        "pr" => (SlashCommand::Pr, Some(args)),
        "init" => (SlashCommand::Init, Some(args)),
        "sandbox" => (SlashCommand::Sandbox, None),
        "agents" => (SlashCommand::Agents, None),
        "mcp" => (SlashCommand::Mcp, None),
        "skills" => (SlashCommand::Skills, None),
        "system-prompt" | "sp" => (SlashCommand::SystemPrompt, Some(args)),
        _ => (SlashCommand::Unknown(cmd.to_string()), Some(args)),
    }
}

pub fn get_help_text() -> String {
    r#"Available Commands:

Session Commands:
/help - Show this help message
/model [name] - Show or set the model (e.g., /model opus, /model sonnet, /model haiku)
/session [action] - Session management (new, list, load <id>)
/compact - Compact the conversation history
/clear - Clear conversation history
/status - Show current session status
/cost - Show usage cost statistics
/stats - Show session statistics
/tasks - Show todo tasks

Code Commands:
/review [file] - Request code review
/security-review [file] - Request security-focused review
/diff [commit] - Show git diff
/commit [message] - Commit changes
/pr - Show open pull requests
/init - Initialize a new project

System Commands:
/sandbox - Show sandbox mode info
/agents - List available sub-agents
/mcp - MCP server status
/skills - Show available skills
/system-prompt [prompt] - Update system prompt

Model Aliases:
- opus → claude-opus-4-6
- sonnet → claude-sonnet-4-6 (default)
- haiku → claude-haiku-4-5-20251213

Available Tools:
- read(path, offset?, limit?) - Read file contents
- write(path, content) - Write file contents
- edit(path, old_string, new_string, replace_all?) - Edit file
- glob(pattern, path?) - Search files by pattern
- grep(pattern, path?, case_insensitive?) - Search text
- bash(command, timeout_secs?) - Execute shell command
- lspath(path?) - List directory contents
- web_search(query) - Search the web
- web_fetch(url) - Fetch URL content
- git_status, git_diff, git_log, git_branch - Git operations
- todo_create, todo_list, todo_update, todo_delete - Todo management
- notebook_read, notebook_edit - Jupyter notebook operations"
- lspath(path?) - List directory contents"#
        .to_string()
}
