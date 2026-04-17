//! Session manager for multiple session history

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::session::{Session, SessionMetadata, ConversationMessage, MessageRole, generate_session_id};

/// Session summary for listing (lightweight)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub message_count: usize,
    pub first_message_preview: Option<String>,
}

/// Manager for all sessions in a workspace
pub struct SessionManager {
    sessions_dir: PathBuf,
    current_session: RwLock<Session>,
}

impl SessionManager {
    pub fn new(workspace_root: PathBuf) -> Self {
        let sessions_dir = workspace_root.join(".claude").join("sessions");
        let _ = fs::create_dir_all(&sessions_dir);

        let session_path = workspace_root.join(".claude").join("session.jsonl");
        let current_session = Session::new_with_path(session_path);

        Self {
            sessions_dir,
            current_session: RwLock::new(current_session),
        }
    }

    /// Get all session summaries (history)
    pub fn get_sessions(&self) -> Vec<SessionSummary> {
        let mut summaries = Vec::new();

        // Add current session first
        {
            let current = self.current_session.read().unwrap();
            if !current.messages.is_empty() {
                let preview = current.messages.iter()
                    .find(|m| m.role == MessageRole::User)
                    .map(|m| {
                        let text = m.text();
                        if text.len() > 50 {
                            format!("{}...", &text[..50])
                        } else {
                            text
                        }
                    });

                summaries.push(SessionSummary {
                    id: current.id.clone(),
                    created_at_ms: current.created_at_ms,
                    updated_at_ms: current.updated_at_ms,
                    message_count: current.messages.len(),
                    first_message_preview: preview,
                });
            }
        }

        // Load sessions from directory
        if let Ok(entries) = fs::read_dir(&self.sessions_dir) {
            let current_id = {
                let current = self.current_session.read().unwrap();
                current.id.clone()
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if file_stem == current_id {
                        continue;
                    }

                    if let Ok(session) = self.load_session_file(&path) {
                        let preview = session.messages.iter()
                            .find(|m| m.role == MessageRole::User)
                            .map(|m| {
                                let text = m.text();
                                if text.len() > 50 {
                                    format!("{}...", &text[..50])
                                } else {
                                    text
                                }
                            });

                        summaries.push(SessionSummary {
                            id: session.id.clone(),
                            created_at_ms: session.created_at_ms,
                            updated_at_ms: session.updated_at_ms,
                            message_count: session.messages.len(),
                            first_message_preview: preview,
                        });
                    }
                }
            }
        }

        // Sort by updated_at_ms descending
        summaries.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));

        summaries
    }

    fn load_session_file(&self, path: &Path) -> Result<Session, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

        let mut messages = Vec::new();
        let mut metadata: Option<SessionMetadata> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(msg) = serde_json::from_str::<ConversationMessage>(line) {
                messages.push(msg);
            } else if let Ok(meta) = serde_json::from_str::<SessionMetadata>(line) {
                metadata = Some(meta);
            }
        }

        let now = current_time_millis();
        let metadata = metadata.unwrap_or_else(|| SessionMetadata {
            id: generate_session_id(),
            created_at_ms: now,
            updated_at_ms: now,
            compaction: None,
        });

        let session = Session {
            id: metadata.id,
            created_at_ms: metadata.created_at_ms,
            updated_at_ms: metadata.updated_at_ms,
            messages,
            compaction: metadata.compaction,
            file_path: Some(path.to_path_buf()),
            writer: None,
        };

        Ok(session)
    }

    /// Load a session by ID
    pub fn load_session(&self, session_id: &str) -> Result<Session, String> {
        let current_id = {
            let current = self.current_session.read().unwrap();
            current.id.clone()
        };

        if current_id == session_id {
            return Ok(self.get_current_session());
        }

        // Try sessions_dir first
        let session_path = self.sessions_dir.join(format!("{}.jsonl", session_id));
        if session_path.exists() {
            return self.load_session_file(&session_path);
        }

        // Fallback to session.jsonl (initial session file)
        let current_session_path = self.current_session.read().unwrap().file_path.clone();
        if let Some(ref path) = current_session_path {
            if path.exists() && path.file_stem().and_then(|s| s.to_str()) == Some(session_id) {
                return self.load_session_file(path);
            }
        }

        Err(format!("Session not found: {}", session_id))
    }

    /// Switch to a different session
    pub fn switch_session(&self, session_id: &str) -> Result<Session, String> {
        if session_id.is_empty() {
            return Err("Session ID cannot be empty".to_string());
        }

        let current_id = {
            let current = self.current_session.read().unwrap();
            current.id.clone()
        };

        if current_id == session_id {
            return Ok(self.get_current_session());
        }

        // Save current session before switching
        {
            let mut current = self.current_session.write().unwrap();
            if !current.messages.is_empty() {
                // Save to sessions_dir with session id as filename
                let save_path = self.sessions_dir.join(format!("{}.jsonl", current.id));
                current.file_path = Some(save_path);
                let _ = current.save();
            }

            let session_path = self.sessions_dir.join(format!("{}.jsonl", session_id));
            if session_path.exists() {
                let new_session = self.load_session_file(&session_path)?;
                *current = new_session;
            } else {
                let _now = current_time_millis();
                let mut new_session = Session::new();
                new_session.id = session_id.to_string();
                new_session.file_path = Some(session_path);
                let _ = new_session.init_file();
                *current = new_session;
            }
        }

        Ok(self.get_current_session())
    }

    /// Create a new session
    pub fn create_session(&self) -> Session {
        {
            let mut current = self.current_session.write().unwrap();
            if !current.messages.is_empty() {
                // Save to sessions_dir with session id as filename
                let save_path = self.sessions_dir.join(format!("{}.jsonl", current.id));
                current.file_path = Some(save_path);
                let _ = current.save();
            }
        }

        let session_id = generate_session_id();

        let path = self.sessions_dir.join(format!("{}.jsonl", session_id));
        let mut new_session = Session::new();
        new_session.id = session_id.clone();
        new_session.file_path = Some(path);
        let _ = new_session.init_file();

        *self.current_session.write().unwrap() = new_session.clone();
        new_session
    }

    /// Get current session
    pub fn get_current_session(&self) -> Session {
        self.current_session.read().unwrap().clone()
    }

    /// Get session messages for display
    pub fn get_session_messages(&self, session_id: &str) -> Result<Vec<ConversationMessage>, String> {
        let session = self.load_session(session_id)?;
        Ok(session.messages.clone())
    }
}

fn current_time_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

