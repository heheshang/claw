//! Permission system for tool execution
//!
//! Three levels:
//! - ReadOnly: read, glob, grep, lspath (no modifications)
//! - WorkspaceWrite: + write, edit (workspace modifications)
//! - DangerFullAccess: + bash (arbitrary shell commands)

use std::path::PathBuf;
use super::executor::ToolExecutor;

/// Permission levels for tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionLevel {
    /// Read-only tools only: read, glob, grep, lspath
    ReadOnly,
    /// Workspace modification allowed: + write, edit
    WorkspaceWrite,
    /// Full access including shell commands: + bash
    DangerFullAccess,
}

impl PermissionLevel {
    /// Check if this level allows the given tool
    pub fn allows(&self, tool_name: &str) -> bool {
        match self {
            PermissionLevel::ReadOnly => matches!(
                tool_name,
                "read" | "glob" | "grep" | "lspath"
            ),
            PermissionLevel::WorkspaceWrite => matches!(
                tool_name,
                "read" | "glob" | "grep" | "lspath" | "write" | "edit"
            ),
            PermissionLevel::DangerFullAccess => true, // All tools allowed
        }
    }
}

/// Wrapper around ToolExecutor that enforces permission levels
pub struct PermissionedToolExecutor {
    inner: ToolExecutor,
    permission: PermissionLevel,
}

impl PermissionedToolExecutor {
    pub fn new(workspace_root: PathBuf, permission: PermissionLevel) -> Self {
        Self {
            inner: ToolExecutor::new(workspace_root),
            permission,
        }
    }

    /// Execute a tool if allowed by the current permission level
    pub fn execute(&self, tool_name: &str, input: &str) -> Result<String, String> {
        if !self.permission.allows(tool_name) {
            let tool_name = tool_name.to_string();
            return Err(format!(
                "Permission denied: tool '{}' requires {:?} permission, but current level is {:?}",
                tool_name, tool_name, self.permission
            ));
        }
        self.inner.execute(tool_name, input)
    }

    /// Get the current permission level
    pub fn permission(&self) -> PermissionLevel {
        self.permission
    }

    /// Update the permission level
    pub fn set_permission(&mut self, permission: PermissionLevel) {
        self.permission = permission;
    }
}

/// Returns tool definitions filtered by permission level
pub fn tool_definitions_with_permissions(permission: PermissionLevel) -> Vec<serde_json::Value> {
    use serde_json::json;

    // All available tool definitions
    let all_tools = vec![
        ("read", "Read file contents"),
        ("write", "Write content to file"),
        ("edit", "Edit file contents"),
        ("glob", "Search files by pattern"),
        ("grep", "Search text in files"),
        ("bash", "Execute shell command"),
        ("lspath", "List directory contents"),
    ];

    all_tools
        .into_iter()
        .filter(|(name, _)| permission.allows(name))
        .map(|(name, description)| {
            json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": description,
                    "parameters": get_tool_params(name)
                }
            })
        })
        .collect()
}

fn get_tool_params(tool_name: &str) -> serde_json::Value {
    use serde_json::json;

    match tool_name {
        "read" => json!({
            "type": "object",
            "properties": {
                "path": json!({
                    "type": "string",
                    "description": "File path to read"
                }),
                "offset": json!({
                    "type": "integer",
                    "description": "Line offset to start reading from"
                }),
                "limit": json!({
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                })
            },
            "required": ["path"]
        }),
        "write" => json!({
            "type": "object",
            "properties": {
                "path": json!({
                    "type": "string",
                    "description": "File path to write"
                }),
                "content": json!({
                    "type": "string",
                    "description": "Content to write to the file"
                })
            },
            "required": ["path", "content"]
        }),
        "edit" => json!({
            "type": "object",
            "properties": {
                "path": json!({
                    "type": "string",
                    "description": "File path to edit"
                }),
                "old_string": json!({
                    "type": "string",
                    "description": "String to find and replace"
                }),
                "new_string": json!({
                    "type": "string",
                    "description": "Replacement string"
                }),
                "replace_all": json!({
                    "type": "boolean",
                    "description": "Replace all occurrences"
                })
            },
            "required": ["path", "old_string", "new_string"]
        }),
        "glob" => json!({
            "type": "object",
            "properties": {
                "pattern": json!({
                    "type": "string",
                    "description": "Glob pattern to match files"
                }),
                "path": json!({
                    "type": "string",
                    "description": "Base path to search from"
                })
            },
            "required": ["pattern"]
        }),
        "grep" => json!({
            "type": "object",
            "properties": {
                "pattern": json!({
                    "type": "string",
                    "description": "Regex pattern to search for"
                }),
                "path": json!({
                    "type": "string",
                    "description": "Base path to search in"
                }),
                "case_insensitive": json!({
                    "type": "boolean",
                    "description": "Case insensitive search"
                })
            },
            "required": ["pattern"]
        }),
        "bash" => json!({
            "type": "object",
            "properties": {
                "command": json!({
                    "type": "string",
                    "description": "Shell command to execute"
                }),
                "timeout_secs": json!({
                    "type": "integer",
                    "description": "Timeout in seconds"
                })
            },
            "required": ["command"]
        }),
        "lspath" => json!({
            "type": "object",
            "properties": {
                "path": json!({
                    "type": "string",
                    "description": "Directory path to list"
                })
            },
            "required": []
        }),
        _ => json!({"type": "object", "properties": {}}),
    }
}
