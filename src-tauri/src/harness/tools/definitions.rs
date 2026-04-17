//! Tool definitions for LLM function calling

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read".to_string(),
            description: Some("Read file contents".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "offset": { "type": "integer" },
                    "limit": { "type": "integer" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "write".to_string(),
            description: Some("Write content to file".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "edit".to_string(),
            description: Some("Edit file contents".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_string": { "type": "string" },
                    "new_string": { "type": "string" },
                    "replace_all": { "type": "boolean", "default": false }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
        ToolDefinition {
            name: "glob".to_string(),
            description: Some("Search files by pattern".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "path": { "type": "string" }
                },
                "required": ["pattern"]
            }),
        },
        ToolDefinition {
            name: "grep".to_string(),
            description: Some("Search text in files".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "path": { "type": "string" },
                    "case_insensitive": { "type": "boolean", "default": false }
                },
                "required": ["pattern"]
            }),
        },
        ToolDefinition {
            name: "bash".to_string(),
            description: Some("Execute shell command".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "timeout_secs": { "type": "integer", "default": 30 }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "lspath".to_string(),
            description: Some("List directory contents".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                }
            }),
        },
        ToolDefinition {
            name: "web_search".to_string(),
            description: Some("Search the web for information. Use when you need to find up-to-date information, facts, or answers that aren't in your existing knowledge.".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to find information on the web"
                    },
                    "recency_days": {
                        "type": "integer",
                        "description": "Limit results to within N days (optional)",
                    },
                    "num_results": {
                        "type": "integer",
                        "description": "Number of results to return (default: 5)",
                        "default": 5
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "web_fetch".to_string(),
            description: Some("Fetch content from a URL. Use to retrieve the full content of a webpage, document, or API endpoint.".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch content from"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "What specific information to extract from the page (optional)"
                    }
                },
                "required": ["url"]
            }),
        },
        ToolDefinition {
            name: "todo_create".to_string(),
            description: Some("Create a new todo item".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The todo content/task description"
                    },
                    "priority": {
                        "type": "string",
                        "description": "Priority level: high, medium, low (default: medium)"
                    }
                },
                "required": ["content"]
            }),
        },
        ToolDefinition {
            name: "todo_list".to_string(),
            description: Some("List all todo items".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "Filter by status: pending, in_progress, completed (default: all)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "todo_update".to_string(),
            description: Some("Update a todo item (mark complete, change content, etc.)".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The todo ID to update"
                    },
                    "content": {
                        "type": "string",
                        "description": "New content (optional)"
                    },
                    "status": {
                        "type": "string",
                        "description": "New status: pending, in_progress, completed (optional)"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "todo_delete".to_string(),
            description: Some("Delete a todo item".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The todo ID to delete"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "git_status".to_string(),
            description: Some("Show the working tree status of a git repository".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "short": {
                        "type": "boolean",
                        "description": "Use short format (default: true)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "git_diff".to_string(),
            description: Some("Show changes between commits, commit and working tree, etc.".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "commit": {
                        "type": "string",
                        "description": "Commit hash, branch name, or compare two commits (e.g., 'HEAD~1..HEAD')"
                    },
                    "file": {
                        "type": "string",
                        "description": "Only show changes for this specific file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "git_log".to_string(),
            description: Some("Show commit logs".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "max_count": {
                        "type": "integer",
                        "description": "Limit the number of commits to show (default: 10)"
                    },
                    "format": {
                        "type": "string",
                        "description": "Log format string (default: '%h %s')"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "git_branch".to_string(),
            description: Some("List, create, or delete branches".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "list": {
                        "type": "boolean",
                        "description": "List branches (default: true)"
                    },
                    "branch_name": {
                        "type": "string",
                        "description": "Name for the new branch (optional)"
                    },
                    "delete": {
                        "type": "boolean",
                        "description": "Delete a branch"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "subagent".to_string(),
            description: Some("Execute a task in a sub-agent context. Use when you need to perform multiple independent tasks in parallel.".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "The task description for the sub-agent to execute"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "The actual prompt/question to give the sub-agent"
                    }
                },
                "required": ["task", "prompt"]
            }),
        },
        ToolDefinition {
            name: "notebook_read".to_string(),
            description: Some("Read a Jupyter notebook (.ipynb) and return its contents".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the notebook file"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "notebook_edit".to_string(),
            description: Some("Edit a Jupyter notebook cell or add new cells".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the notebook file"
                    },
                    "cell_index": {
                        "type": "integer",
                        "description": "Cell index to edit (0-based)"
                    },
                    "source": {
                        "type": "string",
                        "description": "New source code for the cell"
                    },
                    "cell_type": {
                        "type": "string",
                        "description": "Cell type: code or markdown"
                    }
                },
                "required": ["path"]
            }),
        },
    ]
}
