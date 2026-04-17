//! Session management with message history and JSONL persistence
//!
//! Features:
//! - JSONL format for efficient incremental writes
//! - Atomic writes (temp file + rename)
//! - File rotation (256KB max, keep 3 history files)
//! - Session compaction support

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SESSION_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

const MAX_SESSION_FILE_SIZE: u64 = 256 * 1024; // 256KB
#[allow(dead_code)]
const MAX_HISTORY_FILES: usize = 3;

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

pub(crate) fn generate_session_id() -> String {
    let millis = current_time_millis();
    let counter = SESSION_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("sess-{millis}-{counter}")
}

fn rotate_session_file(path: &Path) -> io::Result<()> {
    // Remove oldest history file if exists
    let history3 = path.with_extension("jsonl.3");
    if history3.exists() {
        fs::remove_file(&history3)?;
    }

    // Shift .1 -> .2, .2 -> .3
    let history2 = path.with_extension("jsonl.2");
    let history1 = path.with_extension("jsonl.1");
    if history2.exists() {
        fs::rename(&history2, &history3)?;
    }
    if history1.exists() {
        fs::rename(&history1, &history2)?;
    }

    // Rename current to .1
    if path.exists() {
        fs::rename(path, &history1)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: String },
    ToolResult {
        tool_use_id: String,
        tool_name: String,
        output: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub blocks: Vec<ContentBlock>,
}

impl ConversationMessage {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            blocks: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn assistant(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::Assistant,
            blocks,
        }
    }

    pub fn tool_result(
        tool_use_id: impl Into<String>,
        tool_name: impl Into<String>,
        output: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: MessageRole::Tool,
            blocks: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                tool_name: tool_name.into(),
                output: output.into(),
                is_error,
            }],
        }
    }

    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCompaction {
    pub count: u32,
    pub removed_message_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub compaction: Option<SessionCompaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionFileFormat {
    pub version: u32,
    pub metadata: SessionMetadata,
    pub messages: Vec<ConversationMessage>,
}

pub struct Session {
    pub id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub messages: Vec<ConversationMessage>,
    pub compaction: Option<SessionCompaction>,
    pub file_path: Option<PathBuf>,
    pub writer: Option<BufWriter<File>>,
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
            messages: self.messages.clone(),
            compaction: self.compaction.clone(),
            file_path: self.file_path.clone(),
            writer: None, // Don't clone writer
        }
    }
}

impl Session {
    pub fn new() -> Self {
        let now = current_time_millis();
        Self {
            id: generate_session_id(),
            created_at_ms: now,
            updated_at_ms: now,
            messages: Vec::new(),
            compaction: None,
            file_path: None,
            writer: None,
        }
    }

    pub fn new_with_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        if path.exists() {
            if let Ok(session) = Self::load_from_file(&path) {
                return session;
            }
        }
        let mut session = Self::new();
        session.file_path = Some(path);
        let _ = session.init_file();
        session
    }

    pub fn init_file(&mut self) -> io::Result<()> {
        if let Some(ref path) = self.file_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Check and perform rotation if needed
            if path.exists() {
                if let Ok(metadata) = fs::metadata(path) {
                    if metadata.len() > MAX_SESSION_FILE_SIZE {
                        rotate_session_file(path)?;
                    }
                }
            }

            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            self.writer = Some(BufWriter::new(file));
        }
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;

        // Try JSONL format first (one JSON per line)
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

        if !messages.is_empty() || metadata.is_some() {
            let metadata = metadata.unwrap_or_else(|| {
                let now = current_time_millis();
                SessionMetadata {
                    id: generate_session_id(),
                    created_at_ms: now,
                    updated_at_ms: now,
                    compaction: None,
                }
            });

            let mut session = Self {
                id: metadata.id.clone(),
                created_at_ms: metadata.created_at_ms,
                updated_at_ms: metadata.updated_at_ms,
                messages,
                compaction: metadata.compaction,
                file_path: Some(path.to_path_buf()),
                writer: None,
            };
            session.init_file()?;
            return Ok(session);
        }

        // Try single JSON format
        if let Ok(file_format) =
            serde_json::from_str::<SessionFileFormat>(&content)
        {
            let mut session = Self {
                id: file_format.metadata.id,
                created_at_ms: file_format.metadata.created_at_ms,
                updated_at_ms: file_format.metadata.updated_at_ms,
                messages: file_format.messages,
                compaction: file_format.metadata.compaction,
                file_path: Some(path.to_path_buf()),
                writer: None,
            };
            session.init_file()?;
            return Ok(session);
        }

        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid session file format",
        ))
    }

    fn flush_writer(&mut self) {
        if let Some(ref mut writer) = self.writer {
            let _ = writer.flush();
        }
    }

    pub fn push_message(&mut self, message: ConversationMessage) {
        self.updated_at_ms = current_time_millis();

        // Write to JSONL file if open
        if let Some(ref mut writer) = self.writer {
            if let Ok(json) = serde_json::to_string(&message) {
                let _ = writeln!(writer, "{}", json);
            }
        }

        self.messages.push(message);
    }

    pub fn push_user_text(&mut self, text: impl Into<String>) {
        self.push_message(ConversationMessage::user_text(text));
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.compaction = None;
    }

    pub fn record_compaction(&mut self, summary: &str, removed_count: usize) {
        let count = self.compaction.as_ref().map(|c| c.count + 1).unwrap_or(1);
        self.compaction = Some(SessionCompaction {
            count,
            removed_message_count: removed_count,
            summary: summary.to_string(),
        });
    }

    pub fn save(&mut self) -> io::Result<()> {
        self.flush_writer();

        if let Some(ref path) = self.file_path {
            // Atomic write: temp file + rename
            let temp_path = path.with_extension("tmp");
            {
                let file = File::create(&temp_path)?;
                let mut writer = BufWriter::new(file);

                // Write metadata as first line
                let metadata = SessionMetadata {
                    id: self.id.clone(),
                    created_at_ms: self.created_at_ms,
                    updated_at_ms: self.updated_at_ms,
                    compaction: self.compaction.clone(),
                };
                if let Ok(json) = serde_json::to_string(&metadata) {
                    let _ = writeln!(writer, "{}", json);
                }

                // Write messages
                for msg in &self.messages {
                    if let Ok(json) = serde_json::to_string(msg) {
                        let _ = writeln!(writer, "{}", json);
                    }
                }
            }
            fs::rename(&temp_path, path)?;
        }
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.flush_writer();
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
