pub mod harness;
mod user;

use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use harness::{HarnessState, get_tool_definitions, get_help_text, parse_slash_command, build_system_prompt, PermissionLevel, tool_definitions_with_permissions};
use user::{init_db, AppState, register, login, verify_token, get_user_profile, update_user_profile, change_password, invoke_llm, get_model};

// Re-export types for testing
pub use harness::{Session, SessionManager, ConversationMessage, MessageRole, ContentBlock, ApiMessage, LlmConfig, LlmProvider, ApiClient, Config, ProviderSettings, ToolExecutor, PermissionedToolExecutor};
pub use harness::runtime::MessageContent as HarnessMessageContent;
pub use harness::api::resolve_model_alias;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = init_db().expect("Failed to initialize database");

    // Get workspace root, default to current directory
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("app".to_string()),
                    },
                ))
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(AppState { db: Mutex::new(db) })
        .manage(HarnessState::new(workspace_root))
        .invoke_handler(tauri::generate_handler![
            register,
            login,
            verify_token,
            get_user_profile,
            update_user_profile,
            change_password,
            invoke_llm,
            // Harness commands
            harness_get_session,
            harness_send_message,
            harness_get_tools,
            harness_get_tools_for_permission,
            harness_get_help,
            harness_reset_session,
            harness_set_permission,
            harness_get_api_config,
            harness_save_api_config,
            harness_get_session_list,
            harness_load_session,
            harness_create_session,
            harness_get_session_messages,
            // Daemon commands
            daemon_get_status,
            daemon_trigger_shutdown,
            // Streaming harness commands
            harness_send_message_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ============================================================================
// Harness Commands
// ============================================================================

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub message_count: usize,
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCallInfo>,
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub is_error: bool,
}

#[tauri::command]
fn harness_get_session(state: tauri::State<'_, HarnessState>) -> Result<SessionInfo, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;

    Ok(SessionInfo {
        session_id: session.id.clone(),
        message_count: session.messages.len(),
        model: state.config.read().unwrap().get_model("anthropic", "claude-sonnet-4-20250514"),
    })
}

#[tauri::command]
fn harness_get_tools() -> Result<Vec<serde_json::Value>, String> {
    let tools = get_tool_definitions();
    Ok(tools.into_iter().map(|t| serde_json::to_value(t).unwrap_or_default()).collect())
}

#[tauri::command]
fn harness_get_tools_for_permission(permission: String) -> Result<Vec<serde_json::Value>, String> {
    let level = match permission.to_lowercase().as_str() {
        "readonly" | "read_only" => PermissionLevel::ReadOnly,
        "workspacewrite" | "workspace_write" | "workspace" => PermissionLevel::WorkspaceWrite,
        "dangerfullaccess" | "danger_full_access" | "full" | "danger" => PermissionLevel::DangerFullAccess,
        _ => PermissionLevel::DangerFullAccess,
    };
    Ok(tool_definitions_with_permissions(level))
}

#[tauri::command]
fn harness_set_permission(state: tauri::State<'_, HarnessState>, permission: String) -> Result<String, String> {
    let level = match permission.to_lowercase().as_str() {
        "readonly" | "read_only" => PermissionLevel::ReadOnly,
        "workspacewrite" | "workspace_write" | "workspace" => PermissionLevel::WorkspaceWrite,
        "dangerfullaccess" | "danger_full_access" | "full" | "danger" => PermissionLevel::DangerFullAccess,
        _ => return Err(format!("Unknown permission level: {}", permission)),
    };
    state.tool_executor.lock().unwrap().set_permission(level);
    Ok(format!("Permission set to: {:?}", level))
}

#[tauri::command]
fn harness_get_help() -> Result<String, String> {
    Ok(get_help_text())
}

// ============================================================================
// API Configuration Commands
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub retry_count: Option<u32>,
    pub response_language: Option<String>,
}

#[tauri::command]
fn harness_get_api_config(state: tauri::State<'_, HarnessState>) -> Result<ApiConfig, String> {
    let config = state.config.read().map_err(|e| e.to_string())?;
    let model = config.get_model("anthropic", "claude-sonnet-4-20250514");

    Ok(ApiConfig {
        provider: "anthropic".to_string(),
        api_key: config.providers.anthropic.api_key.clone(),
        base_url: config.providers.anthropic.base_url.clone(),
        model: Some(model),
        max_tokens: config.providers.anthropic.max_tokens,
        temperature: config.providers.anthropic.temperature,
        retry_count: config.providers.anthropic.retry_count,
        response_language: config.providers.anthropic.response_language.clone(),
    })
}

#[tauri::command]
fn harness_save_api_config(state: tauri::State<'_, HarnessState>, config: ApiConfig) -> Result<(), String> {
    let mut cfg = state.config.write().map_err(|e| e.to_string())?;

    match config.provider.as_str() {
        "anthropic" => {
            cfg.providers.anthropic.api_key = config.api_key;
            cfg.providers.anthropic.base_url = config.base_url;
            cfg.providers.anthropic.model = config.model;
            cfg.providers.anthropic.max_tokens = config.max_tokens;
            cfg.providers.anthropic.temperature = config.temperature;
            cfg.providers.anthropic.retry_count = config.retry_count;
            cfg.providers.anthropic.response_language = config.response_language.clone();
        }
        "openai" => {
            cfg.providers.openai.api_key = config.api_key;
            cfg.providers.openai.base_url = config.base_url;
            cfg.providers.openai.model = config.model;
            cfg.providers.openai.max_tokens = config.max_tokens;
            cfg.providers.openai.temperature = config.temperature;
            cfg.providers.openai.retry_count = config.retry_count;
            cfg.providers.openai.response_language = config.response_language.clone();
        }
        "xai" => {
            cfg.providers.xai.api_key = config.api_key;
            cfg.providers.xai.base_url = config.base_url;
            cfg.providers.xai.model = config.model;
            cfg.providers.xai.max_tokens = config.max_tokens;
            cfg.providers.xai.temperature = config.temperature;
            cfg.providers.xai.retry_count = config.retry_count;
            cfg.providers.xai.response_language = config.response_language.clone();
        }
        _ => return Err(format!("Unknown provider: {}", config.provider)),
    }

    // Save to local file
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let local_path = workspace_root.join(".claw").join("settings.local.json");

    if let Some(parent) = local_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Serialize and save
    let json = serde_json::to_string_pretty(&*cfg).map_err(|e| e.to_string())?;
    std::fs::write(&local_path, json).map_err(|e| e.to_string())?;

    // Update API client if key changed
    if let Some(api_key) = cfg.providers.anthropic.api_key.clone() {
        let model = cfg.get_model("anthropic", "claude-sonnet-4-20250514");
        let base_url = cfg.providers.anthropic.base_url.clone();
        let retry_count = cfg.providers.anthropic.retry_count;
        let response_language = cfg.providers.anthropic.response_language.clone();

        let llm_config = harness::LlmConfig {
            provider: harness::LlmProvider::Anthropic,
            model,
            api_key: Some(api_key),
            base_url,
            max_tokens: 4096,
            temperature: None,
            retry_count,
            response_language,
        };

        let api_client = harness::ApiClient::new(llm_config);
        *state.api_client.lock().map_err(|e| e.to_string())? = Some(api_client);
    }

    Ok(())
}

// ============================================================================
// Daemon Commands (Background Tasks)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct DaemonStatus {
    pub components: Vec<ComponentStatus>,
    pub shutdown_requested: bool,
}

#[derive(Debug, Serialize)]
pub struct ComponentStatus {
    pub name: String,
    pub status: String,
    pub last_error: Option<String>,
    pub restart_count: u64,
    pub last_restart_at: Option<String>,
}

#[tauri::command]
async fn daemon_get_status(state: tauri::State<'_, HarnessState>) -> Result<DaemonStatus, String> {
    let snapshot = state.daemon_state.snapshot().await;
    let shutdown_requested = state.daemon_state.shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) > 0;

    let components = snapshot.into_iter().map(|c| {
        let status_str = match c.status {
            harness::HealthStatus::Ok => "ok",
            harness::HealthStatus::Error => "error",
            harness::HealthStatus::Starting => "starting",
        };
        ComponentStatus {
            name: c.name,
            status: status_str.to_string(),
            last_error: c.last_error,
            restart_count: c.restart_count,
            last_restart_at: c.last_restart_at,
        }
    }).collect();

    Ok(DaemonStatus {
        components,
        shutdown_requested,
    })
}

#[tauri::command]
fn daemon_trigger_shutdown(state: tauri::State<'_, HarnessState>) -> Result<(), String> {
    state.daemon_state.shutdown_requested.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn harness_reset_session(state: tauri::State<'_, HarnessState>) -> Result<SessionInfo, String> {
    let mut session = state.session.lock().map_err(|e| e.to_string())?;
    *session = harness::Session::new();

    Ok(SessionInfo {
        session_id: session.id.clone(),
        message_count: session.messages.len(),
        model: state.config.read().unwrap().get_model("anthropic", "claude-sonnet-4-20250514"),
    })
}

// ============================================================================
// Session History Commands
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionHistoryItem {
    pub id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub message_count: usize,
    pub first_message_preview: Option<String>,
}

#[tauri::command]
fn harness_get_session_list(state: tauri::State<'_, HarnessState>) -> Result<Vec<SessionHistoryItem>, String> {
    let sessions = state.session_manager.get_sessions();
    Ok(sessions.into_iter().map(|s| SessionHistoryItem {
        id: s.id,
        created_at_ms: s.created_at_ms,
        updated_at_ms: s.updated_at_ms,
        message_count: s.message_count,
        first_message_preview: s.first_message_preview,
    }).collect())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSessionResponse {
    pub id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub message_count: usize,
}

#[tauri::command]
fn harness_load_session(state: tauri::State<'_, HarnessState>, session_id: String) -> Result<LoadSessionResponse, String> {
    let session = state.session_manager.switch_session(&session_id)?;
    Ok(LoadSessionResponse {
        id: session.id.clone(),
        created_at_ms: session.created_at_ms,
        updated_at_ms: session.updated_at_ms,
        message_count: session.messages.len(),
    })
}

#[tauri::command]
fn harness_create_session(state: tauri::State<'_, HarnessState>) -> Result<LoadSessionResponse, String> {
    let session = state.session_manager.create_session();
    Ok(LoadSessionResponse {
        id: session.id.clone(),
        created_at_ms: session.created_at_ms,
        updated_at_ms: session.updated_at_ms,
        message_count: session.messages.len(),
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageContent {
    pub role: String,
    pub text: String,
}

#[tauri::command]
fn harness_get_session_messages(state: tauri::State<'_, HarnessState>, session_id: String) -> Result<Vec<MessageContent>, String> {
    let messages = state.session_manager.get_session_messages(&session_id)?;
    Ok(messages.into_iter().map(|m| MessageContent {
        role: format!("{:?}", m.role).to_lowercase(),
        text: m.text(),
    }).collect())
}

#[tauri::command]
async fn harness_send_message(
    state: tauri::State<'_, HarnessState>,
    request: SendMessageRequest,
) -> Result<SendMessageResponse, String> {
    let message = request.message.trim();

    if message.is_empty() {
        return Err("Message cannot be empty".to_string());
    }

    // Handle slash commands first
    let (cmd, _args) = parse_slash_command(message);

    match cmd {
        harness::SlashCommand::Help => {
            return Ok(SendMessageResponse {
                text: get_help_text(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Model { name } => {
            if let Some(name) = name {
                let resolved = harness::api::resolve_model_alias(&name);
                let mut config = state.config.write().map_err(|e| e.to_string())?;
                config.model = Some(resolved.clone());
                return Ok(SendMessageResponse {
                    text: format!("Model changed to: {} (resolved to: {})", name, resolved),
                    tool_calls: vec![],
                    session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
                });
            } else {
                let config = state.config.read().map_err(|e| e.to_string())?;
                let model = config.get_model("anthropic", "claude-sonnet-4-20250514");
                return Ok(SendMessageResponse {
                    text: format!("Current model: {}", model),
                    tool_calls: vec![],
                    session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
                });
            }
        }
        harness::SlashCommand::Clear => {
            let mut session = state.session.lock().map_err(|e| e.to_string())?;
            session.clear();
            return Ok(SendMessageResponse {
                text: "Conversation cleared.".to_string(),
                tool_calls: vec![],
                session_id: session.id.clone(),
            });
        }
        harness::SlashCommand::Compact => {
            let mut session = state.session.lock().map_err(|e| e.to_string())?;
            let removed = session.messages.len().saturating_sub(2);
            session.record_compaction("Session compacted", removed);
            return Ok(SendMessageResponse {
                text: format!("Session compacted, {} messages summarized.", removed),
                tool_calls: vec![],
                session_id: session.id.clone(),
            });
        }
        harness::SlashCommand::Session { action } => {
            return Ok(SendMessageResponse {
                text: format!("Session action: {} (not fully implemented)", action),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Status => {
            let session = state.session.lock().map_err(|e| e.to_string())?;
            let config = state.config.read().map_err(|e| e.to_string())?;
            let model = config.get_model("anthropic", "claude-sonnet-4-20250514");
            return Ok(SendMessageResponse {
                text: format!(
                    "Session: {}\nMessages: {}\nModel: {}",
                    session.id,
                    session.messages.len(),
                    model
                ),
                tool_calls: vec![],
                session_id: session.id.clone(),
            });
        }
        harness::SlashCommand::Cost => {
            let tracker = state.usage_tracker.lock().map_err(|e| e.to_string())?;
            match tracker.get_stats() {
                Ok(stats) => {
                    return Ok(SendMessageResponse {
                        text: format!(
                            "Cost Statistics\n\n\
                             Total API Calls: {}\n\
                             Input Tokens: {}\n\
                             Output Tokens: {}\n\
                             Total Tokens: {}\n\
                             Estimated Cost: {} USD\n\n\
                             First call: {}\n\
                             Last call: {}",
                            stats.total_calls,
                            stats.total_input_tokens,
                            stats.total_output_tokens,
                            stats.total_tokens,
                            stats.total_cost_usd,
                            stats.first_call.map(|t| t.to_string()).unwrap_or_else(|| "N/A".to_string()),
                            stats.last_call.map(|t| t.to_string()).unwrap_or_else(|| "N/A".to_string()),
                        ),
                        tool_calls: vec![],
                        session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
                    });
                }
                Err(e) => {
                    return Ok(SendMessageResponse {
                        text: format!("Cost tracking unavailable: {}", e),
                        tool_calls: vec![],
                        session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
                    });
                }
            }
        }
        harness::SlashCommand::Stats => {
            let session = state.session.lock().map_err(|e| e.to_string())?;
            let tracker = state.usage_tracker.lock().map_err(|e| e.to_string())?;
            let stats = tracker.get_stats().ok();
            return Ok(SendMessageResponse {
                text: format!(
                    "📊 Session Statistics\n\n\
                     Session ID: {}\n\
                     Messages: {}\n\
                     System prompts: 1\n\n\
                     API Usage:\n\
                     - Total Calls: {}\n\
                     - Total Tokens: {}\n\
                     - Estimated Cost: {} USD",
                    session.id,
                    session.messages.len(),
                    stats.as_ref().map(|s| s.total_calls).unwrap_or(0),
                    stats.as_ref().map(|s| s.total_tokens).unwrap_or(0),
                    stats.as_ref().map(|s| s.total_cost_usd).unwrap_or(0.0),
                ),
                tool_calls: vec![],
                session_id: session.id.clone(),
            });
        }
        harness::SlashCommand::Tasks => {
            let todo_store_path = std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".claude/todos.json");

            let todos = if todo_store_path.exists() {
                let content = std::fs::read_to_string(&todo_store_path)
                    .unwrap_or_default();
                serde_json::from_str::<Vec<serde_json::Value>>(&content)
                    .map(|t| format!("Found {} todos", t.len()))
                    .unwrap_or_else(|_| "No todos found".to_string())
            } else {
                "No todos found".to_string()
            };

            return Ok(SendMessageResponse {
                text: todos,
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Review => {
            // Get args from the message (everything after /review)
            let file_path = message.trim_start_matches('/').trim();
            if file_path.is_empty() || file_path == "review" {
                return Ok(SendMessageResponse {
                    text: "🔍 Code Review\n\nUsage: /review <file_path>\n\nExample: /review src/main.rs".to_string(),
                    tool_calls: vec![],
                    session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
                });
            }

            // Read the file
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    return Ok(SendMessageResponse {
                        text: format!("❌ Failed to read file '{}': {}", file_path, e),
                        tool_calls: vec![],
                        session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
                    });
                }
            };

            // Limit content size for display
            let preview = if content.len() > 2000 {
                format!("{}\n\n...[Content truncated - {} chars total]...", &content[..2000], content.len())
            } else {
                content
            };

            return Ok(SendMessageResponse {
                text: format!("📝 Code Review: {}\n\n```\n{}\n```\n\nPlease ask me to analyze this code for issues, improvements, or bugs.", file_path, preview),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::SecurityReview => {
            // Get args from the message (everything after /security-review)
            let file_path = message.trim_start_matches('/').trim();
            // Remove "security-review" or "security" prefix
            let file_path = file_path.trim_start_matches("security-review").trim_start_matches("security").trim();

            if file_path.is_empty() {
                return Ok(SendMessageResponse {
                    text: "🔐 Security Review\n\nUsage: /security-review <file_path>\n\nExample: /security-review src/auth.rs".to_string(),
                    tool_calls: vec![],
                    session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
                });
            }

            // Read the file
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    return Ok(SendMessageResponse {
                        text: format!("❌ Failed to read file '{}': {}", file_path, e),
                        tool_calls: vec![],
                        session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
                    });
                }
            };

            // Limit content size for display
            let preview = if content.len() > 2000 {
                format!("{}\n\n...[Content truncated - {} chars total]...", &content[..2000], content.len())
            } else {
                content
            };

            return Ok(SendMessageResponse {
                text: format!("🔐 Security Review: {}\n\n```\n{}\n```\n\nPlease analyze this code for security vulnerabilities, including:\n- Injection attacks\n- Authentication issues\n- Data exposure risks\n- Input validation problems\n- Dependency vulnerabilities", file_path, preview),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Diff => {
            return Ok(SendMessageResponse {
                text: "Git diff: Use git_diff tool for detailed diff".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Commit => {
            return Ok(SendMessageResponse {
                text: "Git commit: Use bash tool to run git commands".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Pr => {
            return Ok(SendMessageResponse {
                text: "Pull requests: Use web_search to find PRs or bash for git commands".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Init => {
            return Ok(SendMessageResponse {
                text: "Project initialization: Please describe what to initialize".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Sandbox => {
            return Ok(SendMessageResponse {
                text: "Sandbox mode: Running in isolated workspace".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Agents => {
            return Ok(SendMessageResponse {
                text: "Available agents: subagent tool for parallel task execution".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Mcp => {
            return Ok(SendMessageResponse {
                text: "MCP server: MCP integration not yet configured".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Skills => {
            return Ok(SendMessageResponse {
                text: "Available skills: Code review, security analysis, git operations, web search".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::SystemPrompt => {
            return Ok(SendMessageResponse {
                text: "System prompt management: Use /system-prompt [prompt] to update".to_string(),
                tool_calls: vec![],
                session_id: state.session.lock().map_err(|e| e.to_string())?.id.clone(),
            });
        }
        harness::SlashCommand::Unknown(_) => {
            // Not a slash command, proceed with normal processing
        }
    }

    // Get config and check API key
    let (config, session_id) = {
        let config = state.config.read().map_err(|e| e.to_string())?;
        let session_id = state.session.lock().map_err(|e| e.to_string())?.id.clone();
        (config.clone(), session_id)
    };

    // Try multiple sources for API key: config file, then environment variables
    let api_key = config.providers.anthropic.api_key.clone()
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok());

    let _api_key = match api_key {
        Some(key) => key,
        None => {
            // Try reading settings.local.json by searching up from current directory
            let mut dir = std::env::current_dir().ok();
            let mut found_key: Option<String> = None;
            for _ in 0..5 {
                if let Some(d) = dir {
                    let settings_path = d.join(".claw/settings.local.json");
                    if settings_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&settings_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(key) = json["providers"]["anthropic"]["api_key"].as_str() {
                                    if !key.is_empty() {
                                        found_key = Some(key.to_string());
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

            match found_key {
                Some(key) => key,
                None => {
                    // Last resort: check env vars again
                    std::env::var("ANTHROPIC_API_KEY")
                        .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
                        .map_err(|_| "API key not configured. Set ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN environment variable, or add to settings.local.json".to_string())?
                }
            }
        }
    };

    let model = get_model();

    // Add user message to session
    {
        let mut session = state.session.lock().map_err(|e| e.to_string())?;
        session.push_user_text(message);
    }

    // Get tools
    let tools: Vec<serde_json::Value> = get_tool_definitions()
        .into_iter()
        .map(|t| serde_json::to_value(t).unwrap_or_default())
        .collect();

    // Get API client
    let api_client = {
        let client = state.api_client.lock().map_err(|e| e.to_string())?;
        client.clone()
    };
    let api_client = api_client.ok_or_else(|| "API client not initialized".to_string())?;

    let mut response_text = String::new();
    let mut all_tool_calls = Vec::new();
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let max_iterations = 10;

    for iteration in 0..max_iterations {
        log::info!("[LOOP] Iteration {}", iteration + 1);

        // Build API messages from session
        let api_messages = {
            let session = state.session.lock().map_err(|e| e.to_string())?;
            let system_content = format!(
                "{}\n\nYou have access to the following tools:\n{}",
                build_system_prompt(),
                serde_json::to_string_pretty(&tools).unwrap_or_default()
            );

            let mut msgs: Vec<harness::ApiMessage> = vec![harness::ApiMessage {
                role: "system".to_string(),
                content: Some(harness::MessageContent::Text(system_content)),
            }];

            for msg in &session.messages {
                // Skip system messages - we add them separately
                if msg.role == harness::MessageRole::System {
                    continue;
                }
                match msg.role {
                    harness::MessageRole::User => {
                        // Collect text content from user message
                        let text_content: String = msg.blocks.iter().filter_map(|b| match b {
                            harness::ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        }).collect::<Vec<_>>().join("\n");
                        if !text_content.is_empty() {
                            msgs.push(harness::ApiMessage {
                                role: "user".to_string(),
                                content: Some(harness::MessageContent::Text(text_content)),
                            });
                        }
                    }
                    harness::MessageRole::Assistant => {
                        // For assistant messages, we need to convert blocks to API format
                        // ToolUse and ToolResult are paired in the same message
                        let mut content_blocks: Vec<serde_json::Value> = Vec::new();
                        let mut text_parts: Vec<String> = Vec::new();
                        let blocks = &msg.blocks;
                        let mut i = 0;

                        while i < blocks.len() {
                            match &blocks[i] {
                                harness::ContentBlock::Text { text } => {
                                    text_parts.push(text.clone());
                                }
                                harness::ContentBlock::ToolUse { id, name, input } => {
                                    // Flush accumulated text first
                                    if !text_parts.is_empty() {
                                        content_blocks.push(serde_json::json!({
                                            "type": "text",
                                            "text": text_parts.join("")
                                        }));
                                        text_parts.clear();
                                    }
                                    // Add the tool use block
                                    content_blocks.push(serde_json::json!({
                                        "type": "tool_use",
                                        "id": id,
                                        "name": name,
                                        "input": serde_json::from_str(input).unwrap_or(serde_json::Value::Null)
                                    }));
                                }
                                harness::ContentBlock::ToolResult { .. } => {
                                    // Should not happen in Assistant message now
                                }
                            }
                            i += 1;
                        }

                        // Flush any remaining text
                        if !text_parts.is_empty() {
                            content_blocks.push(serde_json::json!({
                                "type": "text",
                                "text": text_parts.join("")
                            }));
                        }

                        // Only add assistant message if it has content
                        if !content_blocks.is_empty() {
                            msgs.push(harness::ApiMessage {
                                role: "assistant".to_string(),
                                content: Some(harness::MessageContent::Blocks(content_blocks)),
                            });
                        }
                    }
                    harness::MessageRole::Tool => {
                        // Tool results - convert to user message with content block
                        for block in &msg.blocks {
                            if let harness::ContentBlock::ToolResult { tool_use_id, output, .. } = block {
                                let content_block = serde_json::json!({
                                    "type": "tool_result",
                                    "tool_use_id": tool_use_id,
                                    "content": output
                                });
                                msgs.push(harness::ApiMessage {
                                    role: "user".to_string(),
                                    content: Some(harness::MessageContent::Blocks(vec![content_block])),
                                });
                            }
                        }
                    }
                    harness::MessageRole::System => unreachable!("System messages are handled before the match"),
                }
            }

            msgs
        };

        log::info!("[LOOP] Sending API request with {} messages", api_messages.len());

        // Debug: log message structure in detail
        for (i, msg) in api_messages.iter().enumerate() {
            let content_preview = match &msg.content {
                Some(harness::MessageContent::Text(t)) => {
                    if t.len() > 100 {
                        format!("text: {}...{} chars", &t[..50], &t[t.len()-50..])
                    } else {
                        format!("text: {}", t)
                    }
                }
                Some(harness::MessageContent::Blocks(b)) => {
                    let mut details = Vec::new();
                    for block in b.iter() {
                        if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                            if block_type == "tool_use" {
                                let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                                details.push(format!("tool_use(id={},name={})", id, name));
                            } else if block_type == "tool_result" {
                                let id = block.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("?");
                                details.push(format!("tool_result(tool_use_id={})", id));
                            } else {
                                details.push(block_type.to_string());
                            }
                        }
                    }
                    format!("blocks: [{}]", details.join(", "))
                }
                None => "none".to_string(),
            };
            log::info!("[LOOP] Message {}: role={}, content={}", i, msg.role, content_preview);
        }

        // Stream from API
        let events = api_client.stream_chat(api_messages, tools.clone(), &model).await?;

        let mut iteration_text = String::new();
        let mut iteration_assistant_blocks = Vec::new();
        let mut has_tool_use = false;

        for event in events {
            match event {
                harness::SseEvent::TextDelta(delta) => {
                    iteration_text.push_str(&delta);
                    iteration_assistant_blocks.push(harness::ContentBlock::Text { text: delta });
                }
                harness::SseEvent::ToolUse { id, name, input } => {
                    has_tool_use = true;
                    log::info!("[LOOP] ToolUse: id={}, name={}, input={}", id, name, input);

                    // Execute the tool first to get the result
                    let tool_result = {
                        let executor = state.tool_executor.lock().unwrap();
                        executor.execute(&name, &input)
                    };

                    match tool_result {
                        Ok(output) => {
                            // Store assistant message with tool_use first
                            {
                                let mut session = state.session.lock().map_err(|e| e.to_string())?;
                                session.push_message(harness::ConversationMessage::assistant(
                                    vec![harness::ContentBlock::ToolUse {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input: input.clone(),
                                    }]
                                ));
                                // Then store tool_result as separate message
                                session.push_message(harness::ConversationMessage::tool_result(
                                    &id,
                                    &name,
                                    &output,
                                    false,
                                ));
                            }
                            // Add to tool calls with result
                            all_tool_calls.push(ToolCallInfo {
                                id: id.clone(),
                                name: name.clone(),
                                input: input.clone(),
                                output: Some(output),
                                is_error: false,
                            });
                            log::info!("[LOOP] Tool executed successfully, stored tool_use and tool_result separately");
                        }
                        Err(e) => {
                            {
                                let mut session = state.session.lock().map_err(|e| e.to_string())?;
                                session.push_message(harness::ConversationMessage::assistant(
                                    vec![harness::ContentBlock::ToolUse {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input: input.clone(),
                                    }]
                                ));
                                session.push_message(harness::ConversationMessage::tool_result(
                                    &id,
                                    &name,
                                    e.to_string(),
                                    true,
                                ));
                            }
                            // Add to tool calls with error
                            all_tool_calls.push(ToolCallInfo {
                                id: id.clone(),
                                name: name.clone(),
                                input: input.clone(),
                                output: Some(e.to_string()),
                                is_error: true,
                            });
                            log::error!("[LOOP] Tool execution failed: {}", e);
                        }
                    }
                }
                harness::SseEvent::MessageStop => {}
                harness::SseEvent::Usage { input_tokens, output_tokens } => {
                    total_input_tokens += input_tokens;
                    total_output_tokens += output_tokens;
                }
            }
        }

        // Add assistant message with remaining text only (no tool_use - they're stored immediately)
        let has_text = iteration_assistant_blocks.iter().any(|b| matches!(b, harness::ContentBlock::Text { .. }));
        if has_text {
            let text_blocks: Vec<harness::ContentBlock> = iteration_assistant_blocks.iter()
                .filter_map(|b| match b {
                    harness::ContentBlock::Text { text } if !text.is_empty() => Some(b.clone()),
                    _ => None,
                })
                .collect();
            if !text_blocks.is_empty() {
                let mut session = state.session.lock().map_err(|e| e.to_string())?;
                session.push_message(harness::ConversationMessage::assistant(text_blocks));
            }
        }

        // Accumulate text
        response_text.push_str(&iteration_text);

        // If no tool use, we're done
        if !has_tool_use {
            log::info!("[LOOP] No more tool calls, exiting");
            break;
        }

        log::info!("[LOOP] Tool calls executed, continuing iteration");
    }

    // Record usage to telemetry
    if total_input_tokens > 0 || total_output_tokens > 0 {
        let record = harness::ApiCallRecord {
            timestamp: chrono::Utc::now(),
            provider: "anthropic".to_string(),
            model: model.clone(),
            input_tokens: total_input_tokens,
            output_tokens: total_output_tokens,
            total_tokens: total_input_tokens + total_output_tokens,
            cost_usd: harness::CostCalculator::new()
                .calculate("anthropic", &model, total_input_tokens, total_output_tokens)
                .total_cost,
            duration_ms: 0, // Duration tracking would require more complex integration
        };
        if let Ok(mut tracker) = state.usage_tracker.lock() {
            let _ = tracker.add_record(&record);
        }
    }

    Ok(SendMessageResponse {
        text: response_text,
        tool_calls: all_tool_calls,
        session_id,
    })
}

// ============================================================================
// Streaming Message Command (SSE)
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct StreamEvent {
    pub event_type: String,  // "text_delta" | "tool_use" | "tool_result" | "done" | "error"
    pub content: String,
    pub tool_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,
    pub tool_output: Option<String>,
    pub is_error: Option<bool>,
}

#[tauri::command]
async fn harness_send_message_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, HarnessState>,
    request: SendMessageRequest,
) -> Result<(), String> {
    let message = request.message.trim();

    if message.is_empty() {
        return Err("Message cannot be empty".to_string());
    }

    // Emit stream started event
    log::info!("[STREAM] Emitting stream_start event");
    let _ = app.emit("stream_start", serde_json::json!({ "message_id": format!("assistant-{}", chrono::Utc::now().timestamp_millis()) }));
    log::info!("[STREAM] stream_start event emitted");

    // Handle slash commands that don't need streaming
    let (cmd, args) = parse_slash_command(message);

    // Handle /review and /security-review commands - emit content directly
    match cmd {
        harness::SlashCommand::Review => {
            let file_path = args.unwrap_or("").trim();
            if file_path.is_empty() {
                let _ = app.emit("stream_chunk", StreamEvent {
                    event_type: "text_delta".to_string(),
                    content: "🔍 Code Review\n\nUsage: /review <file_path>\n\nExample: /review src/main.rs".to_string(),
                    tool_id: None,
                    tool_name: None,
                    tool_input: None,
                    tool_output: None,
                    is_error: None,
                });
            } else {
                let path = std::path::Path::new(file_path);

                // Check if it's a directory
                if path.is_dir() {
                    // List directory contents for /review
                    match std::fs::read_dir(path) {
                        Ok(entries) => {
                            let mut files: Vec<String> = Vec::new();
                            let mut dirs: Vec<String> = Vec::new();

                            for entry in entries.take(50) {
                                if let Ok(entry) = entry {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                                        dirs.push(format!("📁 {}", name));
                                    } else {
                                        files.push(format!("📄 {}", name));
                                    }
                                }
                            }

                            let mut content = format!("📂 Directory: {}\n\n", file_path);
                            content.push_str("**Subdirectories:**\n");
                            if dirs.is_empty() {
                                content.push_str("_None_\n");
                            } else {
                                content.push_str(&dirs.join("\n"));
                            }
                            content.push_str("\n**Files:**\n");
                            if files.is_empty() {
                                content.push_str("_None_");
                            } else {
                                content.push_str(&files.join("\n"));
                            }
                            content.push_str("\n\n_Use `/review <filepath>` to review a specific file_");

                            let _ = app.emit("stream_chunk", StreamEvent {
                                event_type: "text_delta".to_string(),
                                content,
                                tool_id: None,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                                is_error: None,
                            });
                        }
                        Err(e) => {
                            let _ = app.emit("stream_chunk", StreamEvent {
                                event_type: "text_delta".to_string(),
                                content: format!("❌ Failed to read directory '{}': {}", file_path, e),
                                tool_id: None,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                                is_error: None,
                            });
                        }
                    }
                } else {
                    match std::fs::read_to_string(path) {
                        Ok(content) => {
                            let preview = if content.len() > 2000 {
                                format!("{}\n\n...[Content truncated - {} chars total]...", &content[..2000], content.len())
                            } else {
                                content
                            };
                            let text = format!("📝 Code Review: {}\n\n```\n{}\n```\n\nPlease ask me to analyze this code for issues, improvements, or bugs.", file_path, preview);
                            let _ = app.emit("stream_chunk", StreamEvent {
                                event_type: "text_delta".to_string(),
                                content: text,
                                tool_id: None,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                                is_error: None,
                            });
                        }
                        Err(e) => {
                            let _ = app.emit("stream_chunk", StreamEvent {
                                event_type: "text_delta".to_string(),
                                content: format!("❌ Failed to read file '{}': {}", file_path, e),
                                tool_id: None,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                                is_error: None,
                            });
                        }
                    }
                }
            }
            let _ = app.emit("stream_done", serde_json::json!({}));
            return Ok(());
        }
        harness::SlashCommand::SecurityReview => {
            let file_path = args.unwrap_or("").trim();
            if file_path.is_empty() {
                let _ = app.emit("stream_chunk", StreamEvent {
                    event_type: "text_delta".to_string(),
                    content: "🔐 Security Review\n\nUsage: /security-review <file_path>\n\nExample: /security-review src/auth.rs".to_string(),
                    tool_id: None,
                    tool_name: None,
                    tool_input: None,
                    tool_output: None,
                    is_error: None,
                });
            } else {
                let path = std::path::Path::new(file_path);

                // Check if it's a directory
                if path.is_dir() {
                    // List directory contents for /security-review
                    match std::fs::read_dir(path) {
                        Ok(entries) => {
                            let mut files: Vec<String> = Vec::new();
                            let mut dirs: Vec<String> = Vec::new();

                            for entry in entries.take(50) {
                                if let Ok(entry) = entry {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                                        dirs.push(format!("📁 {}", name));
                                    } else {
                                        files.push(format!("📄 {}", name));
                                    }
                                }
                            }

                            let mut content = format!("📂 Directory: {}\n\n", file_path);
                            content.push_str("**Subdirectories:**\n");
                            if dirs.is_empty() {
                                content.push_str("_None_\n");
                            } else {
                                content.push_str(&dirs.join("\n"));
                            }
                            content.push_str("\n**Files:**\n");
                            if files.is_empty() {
                                content.push_str("_None_");
                            } else {
                                content.push_str(&files.join("\n"));
                            }
                            content.push_str("\n\n_Use `/security-review <filepath>` to review a specific file_");

                            let _ = app.emit("stream_chunk", StreamEvent {
                                event_type: "text_delta".to_string(),
                                content,
                                tool_id: None,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                                is_error: None,
                            });
                        }
                        Err(e) => {
                            let _ = app.emit("stream_chunk", StreamEvent {
                                event_type: "text_delta".to_string(),
                                content: format!("❌ Failed to read directory '{}': {}", file_path, e),
                                tool_id: None,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                                is_error: None,
                            });
                        }
                    }
                } else {
                    match std::fs::read_to_string(path) {
                        Ok(content) => {
                            let preview = if content.len() > 2000 {
                                format!("{}\n\n...[Content truncated - {} chars total]...", &content[..2000], content.len())
                            } else {
                                content
                            };
                            let text = format!("🔐 Security Review: {}\n\n```\n{}\n```\n\nPlease ask me to analyze this code for security vulnerabilities.", file_path, preview);
                            let _ = app.emit("stream_chunk", StreamEvent {
                                event_type: "text_delta".to_string(),
                                content: text,
                                tool_id: None,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                                is_error: None,
                            });
                        }
                        Err(e) => {
                            let _ = app.emit("stream_chunk", StreamEvent {
                                event_type: "text_delta".to_string(),
                                content: format!("❌ Failed to read file '{}': {}", file_path, e),
                                tool_id: None,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                                is_error: None,
                            });
                        }
                    }
                }
            }
            let _ = app.emit("stream_done", serde_json::json!({}));
            return Ok(());
        }
        _ => {}
    }

    match cmd {
        harness::SlashCommand::Help |
        harness::SlashCommand::Model { .. } |
        harness::SlashCommand::Clear |
        harness::SlashCommand::Compact |
        harness::SlashCommand::Status |
        harness::SlashCommand::Cost |
        harness::SlashCommand::Stats |
        harness::SlashCommand::Tasks |
        harness::SlashCommand::Session { .. } |
        harness::SlashCommand::Diff |
        harness::SlashCommand::Commit |
        harness::SlashCommand::Pr |
        harness::SlashCommand::Init |
        harness::SlashCommand::Sandbox |
        harness::SlashCommand::Agents |
        harness::SlashCommand::Mcp |
        harness::SlashCommand::Skills |
        harness::SlashCommand::SystemPrompt |
        harness::SlashCommand::Review |
        harness::SlashCommand::SecurityReview => {
            // These commands don't need streaming, just emit done
            let _ = app.emit("stream_done", serde_json::json!({}));
            return Ok(());
        }
        harness::SlashCommand::Unknown(_) => {}
    }

    // Get config and API key
    let (config, session_id) = {
        let config = state.config.read().map_err(|e| e.to_string())?;
        let session_id = state.session.lock().map_err(|e| e.to_string())?.id.clone();
        (config.clone(), session_id)
    };

    let api_key = config.providers.anthropic.api_key.clone()
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok());

    let api_key = match api_key {
        Some(key) => key,
        None => {
            let _ = app.emit("stream_error", serde_json::json!({ "error": "API key not configured" }));
            return Err("API key not configured".to_string());
        }
    };

    let model = get_model();

    // Add user message to session
    {
        let mut session = state.session.lock().map_err(|e| e.to_string())?;
        session.push_user_text(message);
    }

    // Get tools
    let tools: Vec<serde_json::Value> = get_tool_definitions()
        .into_iter()
        .map(|t| serde_json::to_value(t).unwrap_or_default())
        .collect();

    // Get API client
    let api_client = {
        let client = state.api_client.lock().map_err(|e| e.to_string())?;
        client.clone()
    };
    let api_client = match api_client {
        Some(c) => c,
        None => {
            let _ = app.emit("stream_error", serde_json::json!({ "error": "API client not initialized" }));
            return Err("API client not initialized".to_string());
        }
    };

    // Build API messages from session
    let api_messages = {
        let session = state.session.lock().map_err(|e| e.to_string())?;
        let system_content = format!(
            "{}\n\nYou have access to the following tools:\n{}",
            build_system_prompt(),
            serde_json::to_string_pretty(&tools).unwrap_or_default()
        );

        let mut msgs: Vec<harness::ApiMessage> = vec![harness::ApiMessage {
            role: "system".to_string(),
            content: Some(harness::MessageContent::Text(system_content)),
        }];

        for msg in &session.messages {
            if msg.role == harness::MessageRole::System {
                continue;
            }
            match msg.role {
                harness::MessageRole::User => {
                    let text_content: String = msg.blocks.iter().filter_map(|b| match b {
                        harness::ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    }).collect::<Vec<_>>().join("\n");
                    if !text_content.is_empty() {
                        msgs.push(harness::ApiMessage {
                            role: "user".to_string(),
                            content: Some(harness::MessageContent::Text(text_content)),
                        });
                    }
                }
                harness::MessageRole::Assistant => {
                    let mut content_blocks: Vec<serde_json::Value> = Vec::new();
                    let mut text_parts: Vec<String> = Vec::new();
                    let blocks = &msg.blocks;
                    let mut i = 0;

                    while i < blocks.len() {
                        match &blocks[i] {
                            harness::ContentBlock::Text { text } => {
                                text_parts.push(text.clone());
                            }
                            harness::ContentBlock::ToolUse { id, name, input } => {
                                if !text_parts.is_empty() {
                                    content_blocks.push(serde_json::json!({
                                        "type": "text",
                                        "text": text_parts.join("")
                                    }));
                                    text_parts.clear();
                                }
                                content_blocks.push(serde_json::json!({
                                    "type": "tool_use",
                                    "id": id,
                                    "name": name,
                                    "input": serde_json::from_str(input).unwrap_or(serde_json::Value::Null)
                                }));
                            }
                            harness::ContentBlock::ToolResult { .. } => {}
                        }
                        i += 1;
                    }

                    if !text_parts.is_empty() {
                        content_blocks.push(serde_json::json!({
                            "type": "text",
                            "text": text_parts.join("")
                        }));
                    }

                    if !content_blocks.is_empty() {
                        msgs.push(harness::ApiMessage {
                            role: "assistant".to_string(),
                            content: Some(harness::MessageContent::Blocks(content_blocks)),
                        });
                    }
                }
                harness::MessageRole::Tool => {
                    for block in &msg.blocks {
                        if let harness::ContentBlock::ToolResult { tool_use_id, output, .. } = block {
                            let content_block = serde_json::json!({
                                "type": "tool_result",
                                "tool_use_id": tool_use_id,
                                "content": output
                            });
                            msgs.push(harness::ApiMessage {
                                role: "user".to_string(),
                                content: Some(harness::MessageContent::Blocks(vec![content_block])),
                            });
                        }
                    }
                }
                harness::MessageRole::System => unreachable!("System messages handled before"),
            }
        }

        msgs
    };

    // Use streaming API to get events as they arrive
    let app_clone = app.clone();
    let model_clone = model.clone();
    let state_clone = state.inner().clone();

    let result = api_client.stream_chat_streaming(
        api_messages,
        tools.clone(),
        &model,
        move |event| {
            let app = app_clone.clone();
            let state = state_clone.clone();
            let model = model_clone.clone();

            async move {
                match event {
                    harness::SseEvent::TextDelta(delta) => {
                        log::info!("[STREAM] Emitting text_delta: {} chars", delta.len());
                        let stream_event = StreamEvent {
                            event_type: "text_delta".to_string(),
                            content: delta,
                            tool_id: None,
                            tool_name: None,
                            tool_input: None,
                            tool_output: None,
                            is_error: None,
                        };
                        let _ = app.emit("stream_chunk", stream_event);
                    }
                    harness::SseEvent::ToolUse { id, name, input } => {
                        // Emit tool use immediately
                        let stream_event = StreamEvent {
                            event_type: "tool_use".to_string(),
                            content: String::new(),
                            tool_id: Some(id.clone()),
                            tool_name: Some(name.clone()),
                            tool_input: Some(input.clone()),
                            tool_output: None,
                            is_error: None,
                        };
                        let _ = app.emit("stream_chunk", stream_event);

                        // Execute tool with timeout (async to support cancellation)
                        let tool_name = name.clone();
                        let tool_input = input.clone();
                        let tool_result = {
                            let executor = state.tool_executor.lock().unwrap();
                            executor.execute(&tool_name, &tool_input)
                        };

                        let (output, is_error) = match tool_result {
                            Ok(output) => {
                                {
                                    let mut session = state.session.lock().unwrap();
                                    session.push_message(harness::ConversationMessage::assistant(
                                        vec![harness::ContentBlock::ToolUse {
                                            id: id.clone(),
                                            name: name.clone(),
                                            input: input.clone(),
                                        }]
                                    ));
                                    session.push_message(harness::ConversationMessage::tool_result(
                                        &id,
                                        &name,
                                        &output,
                                        false,
                                    ));
                                }
                                (output, false)
                            }
                            Err(e) => {
                                {
                                    let mut session = state.session.lock().unwrap();
                                    session.push_message(harness::ConversationMessage::assistant(
                                        vec![harness::ContentBlock::ToolUse {
                                            id: id.clone(),
                                            name: name.clone(),
                                            input: input.clone(),
                                        }]
                                    ));
                                    session.push_message(harness::ConversationMessage::tool_result(
                                        &id,
                                        &name,
                                        &e.to_string(),
                                        true,
                                    ));
                                }
                                (e.to_string(), true)
                            }
                        };

                        // Emit tool result
                        let stream_event = StreamEvent {
                            event_type: "tool_result".to_string(),
                            content: output.clone(),
                            tool_id: Some(id),
                            tool_name: Some(name),
                            tool_input: None,
                            tool_output: Some(output),
                            is_error: Some(is_error),
                        };
                        let _ = app.emit("stream_chunk", stream_event);
                    }
                    harness::SseEvent::Usage { input_tokens, output_tokens } => {
                        if input_tokens > 0 || output_tokens > 0 {
                            let record = harness::ApiCallRecord {
                                timestamp: chrono::Utc::now(),
                                provider: "anthropic".to_string(),
                                model: model.clone(),
                                input_tokens,
                                output_tokens,
                                total_tokens: input_tokens + output_tokens,
                                cost_usd: harness::CostCalculator::new()
                                    .calculate("anthropic", &model, input_tokens, output_tokens)
                                    .total_cost,
                                duration_ms: 0,
                            };
                            if let Ok(mut tracker) = state.usage_tracker.lock() {
                                let _ = tracker.add_record(&record);
                            }
                        }
                    }
                    harness::SseEvent::MessageStop => {
                        log::info!("[STREAM] MessageStop received");
                    }
                }
            }
        }
    ).await;

    match result {
        Ok(()) => {
            // Emit done event
            let _ = app.emit("stream_done", serde_json::json!({ "session_id": session_id }));
        }
        Err(e) => {
            let _ = app.emit("stream_error", serde_json::json!({ "error": e }));
        }
    }

    Ok(())
}
