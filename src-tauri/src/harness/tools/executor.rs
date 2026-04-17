//! Tool executor implementation

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub struct ToolExecutor {
    workspace_root: PathBuf,
    default_timeout: Duration,
}

impl ToolExecutor {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            default_timeout: Duration::from_secs(60),
        }
    }

    pub fn execute(&self, tool_name: &str, input: &str) -> Result<String, String> {
        log::info!("[TOOL] Executing tool: {} with input: {}", tool_name, input);
        let result = match tool_name {
            "read" => self.read(input),
            "write" => self.write(input),
            "edit" => self.edit(input),
            "glob" => self.glob(input),
            "grep" => self.grep(input),
            "bash" => self.bash(input),
            "lspath" => self.ls(input),
            "web_search" => self.web_search(input),
            "web_fetch" => self.web_fetch(input),
            "todo_create" => self.todo_create(input),
            "todo_list" => self.todo_list(input),
            "todo_update" => self.todo_update(input),
            "todo_delete" => self.todo_delete(input),
            "git_status" => self.git_status(input),
            "git_diff" => self.git_diff(input),
            "git_log" => self.git_log(input),
            "git_branch" => self.git_branch(input),
            "subagent" => self.subagent(input),
            "notebook_read" => self.notebook_read(input),
            "notebook_edit" => self.notebook_edit(input),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };
        log::info!("[TOOL] Tool result: {:?}", result);
        result
    }

    /// Async execution with timeout
    pub async fn execute_async(&self, tool_name: &str, input: &str) -> Result<String, String> {
        log::info!("[TOOL] Async executing tool: {} with input: {}", tool_name, input);
        let result = match tool_name {
            "read" => self.read(input),
            "write" => self.write(input),
            "edit" => self.edit(input),
            "glob" => self.glob(input),
            "grep" => self.grep(input),
            "bash" => self.bash_async(input).await,
            "lspath" => self.ls(input),
            "web_search" => self.web_search(input),
            "web_fetch" => self.web_fetch(input),
            "todo_create" => self.todo_create(input),
            "todo_list" => self.todo_list(input),
            "todo_update" => self.todo_update(input),
            "todo_delete" => self.todo_delete(input),
            "git_status" => self.git_status(input),
            "git_diff" => self.git_diff(input),
            "git_log" => self.git_log(input),
            "git_branch" => self.git_branch(input),
            "subagent" => self.subagent(input),
            "notebook_read" => self.notebook_read(input),
            "notebook_edit" => self.notebook_edit(input),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };
        log::info!("[TOOL] Tool result: {:?}", result);
        result
    }

    fn read(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            path: String,
            offset: Option<usize>,
            limit: Option<usize>,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;
        let path = self.resolve_path(&input.path);

        let content =
            fs::read_to_string(&path).map_err(|e| format!("Read error: {}", e))?;
        let lines: Vec<&str> = content.lines().collect();

        let start = input.offset.unwrap_or(0).min(lines.len());
        let end = input
            .limit
            .map_or(lines.len(), |l| start.saturating_add(l).min(lines.len()));
        let selected = lines[start..end].join("\n");

        Ok(serde_json::json!({
            "type": "read",
            "file": {
                "path": path.to_string_lossy(),
                "content": selected,
                "numLines": end - start,
                "startLine": start + 1,
                "totalLines": lines.len()
            }
        })
        .to_string())
    }

    fn write(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            path: String,
            content: String,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;
        let path = self.resolve_path(&input.path);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Dir error: {}", e))?;
        }
        fs::write(&path, &input.content).map_err(|e| format!("Write error: {}", e))?;

        Ok(serde_json::json!({
            "type": "write",
            "path": path.to_string_lossy(),
            "success": true
        })
        .to_string())
    }

    fn edit(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            path: String,
            old_string: String,
            new_string: String,
            #[serde(default)]
            replace_all: bool,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;
        let path = self.resolve_path(&input.path);

        let content =
            fs::read_to_string(&path).map_err(|e| format!("Read error: {}", e))?;

        if !content.contains(&input.old_string) {
            return Err("old_string not found".to_string());
        }

        let updated = if input.replace_all {
            content.replace(&input.old_string, &input.new_string)
        } else {
            content.replacen(&input.old_string, &input.new_string, 1)
        };

        fs::write(&path, &updated).map_err(|e| format!("Write error: {}", e))?;

        Ok(serde_json::json!({
            "type": "edit",
            "path": path.to_string_lossy(),
            "success": true,
            "changes": {
                "oldString": input.old_string,
                "newString": input.new_string,
                "replaceAll": input.replace_all
            }
        })
        .to_string())
    }

    fn glob(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            pattern: String,
            #[serde(default)]
            path: Option<String>,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;
        let base = input
            .path
            .map(|p| self.resolve_path(&p))
            .unwrap_or_else(|| self.workspace_root.clone());

        let pattern = if base.as_os_str().is_empty() {
            input.pattern.clone()
        } else {
            format!("{}/{}", base.display(), input.pattern)
        };
        let matches = glob::glob(&pattern)
            .map_err(|e| format!("Glob error: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|p| p.is_file())
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "type": "glob",
            "pattern": input.pattern,
            "files": matches.clone(),
            "count": matches.len()
        })
        .to_string())
    }

    fn grep(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            pattern: String,
            #[serde(default)]
            path: Option<String>,
            #[serde(default)]
            case_insensitive: bool,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;
        let base = input
            .path
            .map(|p| self.resolve_path(&p))
            .unwrap_or_else(|| self.workspace_root.clone());

        let regex = regex::RegexBuilder::new(&input.pattern)
            .case_insensitive(input.case_insensitive)
            .build()
            .map_err(|e| format!("Regex error: {}", e))?;

        let mut matches = Vec::new();
        for entry in walkdir::WalkDir::new(&base)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    for (i, line) in content.lines().enumerate() {
                        if regex.is_match(line) {
                            matches.push(format!(
                                "{}:{}:{}",
                                entry.path().display(),
                                i + 1,
                                line
                            ));
                        }
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "type": "grep",
            "pattern": input.pattern,
            "matches": matches.clone(),
            "count": matches.len()
        })
        .to_string())
    }

    fn bash(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            command: String,
            #[serde(default = "default_timeout")]
            _timeout_secs: u64,
        }
        fn default_timeout() -> u64 {
            30
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&input.command)
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|e| format!("Command error: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(serde_json::json!({
            "type": "bash",
            "command": input.command,
            "exitCode": output.status.code().unwrap_or(-1),
            "stdout": stdout,
            "stderr": stderr,
            "success": output.status.success()
        })
        .to_string())
    }

    async fn bash_async(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            command: String,
            #[serde(default)]
            timeout_secs: Option<u64>,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let timeout_duration = Duration::from_secs(input.timeout_secs.unwrap_or(30));
        let workspace = self.workspace_root.clone();

        let result = timeout(timeout_duration, async move {
            let output = Command::new("sh")
                .arg("-c")
                .arg(&input.command)
                .current_dir(&workspace)
                .output()
                .await
                .map_err(|e| format!("Command error: {}", e))?;

            Ok::<_, String>((output, input.command))
        }).await;

        match result {
            Ok(Ok((output, command))) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                Ok(serde_json::json!({
                    "type": "bash",
                    "command": command,
                    "exitCode": output.status.code().unwrap_or(-1),
                    "stdout": stdout,
                    "stderr": stderr,
                    "success": output.status.success()
                })
                .to_string())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(format!("Command timed out after {:?}", timeout_duration)),
        }
    }

    fn ls(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            #[serde(default)]
            path: Option<String>,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;
        let path = input
            .path
            .map(|p| self.resolve_path(&p))
            .unwrap_or_else(|| self.workspace_root.clone());

        let entries = fs::read_dir(&path)
            .map_err(|e| format!("Read error: {}", e))?
            .filter_map(|e| e.ok())
            .map(|e| {
                let metadata = e.metadata().ok();
                serde_json::json!({
                    "name": e.file_name().to_string_lossy(),
                    "isDir": metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    "size": metadata.as_ref().map(|m| m.len()).unwrap_or(0)
                })
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "type": "lspath",
            "path": path.to_string_lossy(),
            "entries": entries
        })
        .to_string())
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_root.join(path)
        }
    }

    fn web_search(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            query: String,
            #[serde(default)]
            recency_days: Option<i32>,
            #[serde(default = "default_num_results")]
            num_results: usize,
        }
        fn default_num_results() -> usize {
            5
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        // Use DuckDuckGo HTML search (no API key required)
        let query_encoded = urlencoding::encode(&input.query);
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}&kl=wt-wt",
            query_encoded
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; ClawCode/1.0)")
            .build()
            .map_err(|e| format!("HTTP client error: {}", e))?;

        let response = client
            .get(&url)
            .send()
            .map_err(|e| format!("Search request failed: {}", e))?;

        let body = response
            .text()
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Parse HTML results
        let mut results = Vec::new();
        let mut current_title = String::new();
        let mut current_snippet = String::new();
        let mut current_url = String::new();
        let mut in_result = false;

        for line in body.lines() {
            let line_lower = line.to_lowercase();

            // Look for result divs
            if line_lower.contains("result__") || line_lower.contains("web-result") {
                in_result = true;
            }

            // Extract title (first a tag inside result)
            if in_result && line.contains("<a ") && line.to_lowercase().contains("result__a") {
                if let Some(start) = line.find(">") {
                    let after_start = &line[start + 1..];
                    if let Some(end) = after_start.find("</a>") {
                        current_title = after_start[..end]
                            .replace("<strong>", "")
                            .replace("</strong>", "")
                            .trim()
                            .to_string();
                    }
                }
                if let Some(href_start) = line.find("href=\"") {
                    let after_href = &line[href_start + 6..];
                    if let Some(href_end) = after_href.find("\"") {
                        current_url = after_href[..href_end].to_string();
                    }
                }
            }

            // Extract snippet
            if in_result && line_lower.contains("result__snippet") {
                if let Some(start) = line.find(">") {
                    let after_start = &line[start + 1..];
                    if let Some(end) = after_start.find("</a>") {
                        current_snippet = after_start[..end]
                            .replace("<strong>", "")
                            .replace("</strong>", "")
                            .replace("...", "")
                            .trim()
                            .to_string();
                    }
                }
            }

            // End of result
            if in_result && line_lower.contains("</a>") && line_lower.contains("result__snippet") {
                if !current_title.is_empty() {
                    results.push(serde_json::json!({
                        "title": current_title,
                        "url": current_url,
                        "snippet": current_snippet
                    }));
                }
                current_title.clear();
                current_snippet.clear();
                current_url.clear();
                in_result = false;

                if results.len() >= input.num_results {
                    break;
                }
            }
        }

        // Fallback: if parsing failed, try simpler extraction
        if results.is_empty() {
            for line in body.lines() {
                if line.contains("<a ") && (line.contains("result__a") || line.to_lowercase().contains("href=\"http")) {
                    if let Some(start) = line.find(">") {
                        let after_start = &line[start + 1..];
                        if let Some(end) = after_start.find("</a>") {
                            let title = after_start[..end]
                                .replace("<strong>", "")
                                .replace("</strong>", "")
                                .trim()
                                .to_string();
                            if !title.is_empty() && title.len() > 3 {
                                if let Some(href_start) = line.find("href=\"") {
                                    let after_href = &line[href_start + 6..];
                                    if let Some(href_end) = after_href.find("\"") {
                                        let url = after_href[..href_end].to_string();
                                        results.push(serde_json::json!({
                                            "title": title,
                                            "url": url,
                                            "snippet": "Click to view"
                                        }));
                                    }
                                }
                            }
                        }
                    }
                    if results.len() >= input.num_results {
                        break;
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "type": "web_search",
            "query": input.query,
            "results": results,
            "count": results.len()
        })
        .to_string())
    }

    fn web_fetch(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            url: String,
            #[serde(default)]
            prompt: Option<String>,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        // Validate URL
        if !input.url.starts_with("http://") && !input.url.starts_with("https://") {
            return Err("URL must start with http:// or https://".to_string());
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; ClawCode/1.0)")
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| format!("HTTP client error: {}", e))?;

        let response = client
            .get(&input.url)
            .send()
            .map_err(|e| format!("Fetch request failed: {}", e))?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/plain")
            .to_string();

        let body = response
            .text()
            .map_err(|e| format!("Failed to read response: {}", e))?;

        let body_length = body.len();
        let is_html = content_type.contains("text/html");

        // Handle HTML content
        let extracted_text = if is_html {
            self.extract_text_from_html(&body)
        } else {
            body
        };

        // Truncate if too long
        let max_length = 50000;
        let final_content = if extracted_text.len() > max_length {
            format!("{}...[content truncated, {} total chars]", &extracted_text[..max_length], extracted_text.len())
        } else {
            extracted_text
        };

        Ok(serde_json::json!({
            "type": "web_fetch",
            "url": input.url,
            "contentType": content_type,
            "content": final_content,
            "length": body_length
        })
        .to_string())
    }

    fn extract_text_from_html(&self, html: &str) -> String {
        let mut result = String::new();
        let mut in_script = false;
        let mut in_style = false;
        let mut buffer = String::new();
        let mut in_tag = false;

        for c in html.chars() {
            match c {
                '<' => {
                    in_tag = true;
                    if buffer.len() > 2 && !buffer.trim().is_empty() {
                        result.push_str(&buffer);
                        result.push(' ');
                    }
                    buffer.clear();

                    let tag_lower = buffer.to_lowercase();
                    if tag_lower.contains("<script") {
                        in_script = true;
                    } else if tag_lower.contains("<style") {
                        in_style = true;
                    }
                }
                '>' => {
                    in_tag = false;
                    if in_script {
                        in_script = false;
                        buffer.clear();
                    } else if in_style {
                        in_style = false;
                        buffer.clear();
                    }
                }
                _ => {
                    if !in_tag && !in_script && !in_style {
                        buffer.push(c);
                    }
                }
            }
        }

        // Clean up whitespace
        let cleaned: String = result
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
            .chars()
            .filter(|c| c.is_ascii_graphic() || c.is_whitespace())
            .collect();

        cleaned
    }

    fn todo_create(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            content: String,
            #[serde(default = "default_priority")]
            priority: String,
        }
        fn default_priority() -> String {
            "medium".to_string()
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let todo_store = self.get_todo_store()?;

        let id = format!("todo_{}", chrono::Utc::now().timestamp_millis());
        let todo = TodoItem {
            id: id.clone(),
            content: input.content,
            status: "pending".to_string(),
            priority: input.priority,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        todo_store.add(&todo)?;

        Ok(serde_json::json!({
            "type": "todo_create",
            "todo": todo,
            "success": true
        })
        .to_string())
    }

    fn todo_list(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            #[serde(default)]
            status: Option<String>,
        }

        let _input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let todo_store = self.get_todo_store()?;
        let todos = todo_store.list(_input.status.as_deref())?;

        Ok(serde_json::json!({
            "type": "todo_list",
            "todos": todos,
            "count": todos.len()
        })
        .to_string())
    }

    fn todo_update(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            id: String,
            #[serde(default)]
            content: Option<String>,
            #[serde(default)]
            status: Option<String>,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let todo_store = self.get_todo_store()?;
        todo_store.update(&input.id, input.content.as_deref(), input.status.as_deref())?;

        let todo = todo_store.get(&input.id)?;

        Ok(serde_json::json!({
            "type": "todo_update",
            "todo": todo,
            "success": true
        })
        .to_string())
    }

    fn todo_delete(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            id: String,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let todo_store = self.get_todo_store()?;
        todo_store.delete(&input.id)?;

        Ok(serde_json::json!({
            "type": "todo_delete",
            "id": input.id,
            "success": true
        })
        .to_string())
    }

    fn get_todo_store(&self) -> Result<TodoStore, String> {
        TodoStore::new(self.workspace_root.join(".claude/todos.json"))
    }

    fn git_status(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            #[serde(default)]
            path: Option<String>,
            #[serde(default = "default_true")]
            short: bool,
        }
        fn default_true() -> bool {
            true
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let repo_path = input.path
            .map(|p| self.resolve_path(&p))
            .unwrap_or_else(|| self.workspace_root.clone());

        let mut cmd = std::process::Command::new("git");
        cmd.arg("status");
        if input.short {
            cmd.arg("--short");
        }
        cmd.current_dir(&repo_path);

        let output = cmd.output().map_err(|e| format!("Git error: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && !stderr.is_empty() {
            return Err(format!("Git error: {}", stderr));
        }

        Ok(serde_json::json!({
            "type": "git_status",
            "path": repo_path.to_string_lossy(),
            "output": stdout,
            "exitCode": output.status.code().unwrap_or(-1)
        })
        .to_string())
    }

    fn git_diff(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            #[serde(default)]
            path: Option<String>,
            #[serde(default)]
            commit: Option<String>,
            #[serde(default)]
            file: Option<String>,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let repo_path = input.path
            .map(|p| self.resolve_path(&p))
            .unwrap_or_else(|| self.workspace_root.clone());

        let mut cmd = std::process::Command::new("git");
        cmd.arg("diff");

        if let Some(commit) = input.commit {
            cmd.arg(commit);
        }
        if let Some(file) = input.file {
            cmd.arg("--");
            cmd.arg(file);
        }
        cmd.current_dir(&repo_path);

        let output = cmd.output().map_err(|e| format!("Git error: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && !stderr.is_empty() {
            return Err(format!("Git error: {}", stderr));
        }

        Ok(serde_json::json!({
            "type": "git_diff",
            "path": repo_path.to_string_lossy(),
            "output": stdout,
            "exitCode": output.status.code().unwrap_or(-1)
        })
        .to_string())
    }

    fn git_log(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            #[serde(default)]
            path: Option<String>,
            #[serde(default = "default_max_count")]
            max_count: usize,
            #[serde(default = "default_format")]
            format: String,
        }
        fn default_max_count() -> usize {
            10
        }
        fn default_format() -> String {
            "%h %s".to_string()
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let repo_path = input.path
            .map(|p| self.resolve_path(&p))
            .unwrap_or_else(|| self.workspace_root.clone());

        let mut cmd = std::process::Command::new("git");
        cmd.arg("log");
        cmd.arg(format!("-n{}", input.max_count));
        cmd.arg(format!("--format={}", input.format));
        cmd.current_dir(&repo_path);

        let output = cmd.output().map_err(|e| format!("Git error: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && !stderr.is_empty() {
            return Err(format!("Git error: {}", stderr));
        }

        let commits = stdout.lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "type": "git_log",
            "path": repo_path.to_string_lossy(),
            "commits": commits,
            "count": commits.len()
        })
        .to_string())
    }

    fn git_branch(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            #[serde(default)]
            path: Option<String>,
            #[serde(default = "default_true")]
            list: bool,
            #[serde(default)]
            branch_name: Option<String>,
            #[serde(default)]
            delete: Option<String>,
        }
        fn default_true() -> bool {
            true
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let repo_path = input.path
            .map(|p| self.resolve_path(&p))
            .unwrap_or_else(|| self.workspace_root.clone());

        let mut cmd = std::process::Command::new("git");
        cmd.arg("branch");

        if input.list {
            cmd.arg("-a");
        }
        if let Some(name) = input.branch_name {
            cmd.arg(name);
        }
        if let Some(delete_branch) = input.delete {
            if delete_branch == "true" {
                cmd.arg("-d");
            }
        }
        cmd.current_dir(&repo_path);

        let output = cmd.output().map_err(|e| format!("Git error: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && !stderr.is_empty() {
            return Err(format!("Git error: {}", stderr));
        }

        let branches = stdout.lines()
            .map(|line| {
                let is_current = line.starts_with('*');
                let name = line.trim_start_matches(['*', ' ']).to_string();
                serde_json::json!({
                    "name": name,
                    "current": is_current
                })
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "type": "git_branch",
            "path": repo_path.to_string_lossy(),
            "branches": branches,
            "count": branches.len()
        })
        .to_string())
    }

    fn subagent(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            task: String,
            prompt: String,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        // Note: True subagent execution would require spawning a new LLM session
        // For now, we provide a response indicating the subagent task
        // In a full implementation, this would invoke the harness_send_message
        // in a new session context

        Ok(serde_json::json!({
            "type": "subagent",
            "task": input.task,
            "status": "Note: Subagent tasks are logged for coordination. In full implementation, this would spawn a parallel LLM session.",
            "prompt_preview": if input.prompt.len() > 100 { format!("{}...", &input.prompt[..100]) } else { input.prompt },
            "workspace": self.workspace_root.to_string_lossy()
        })
        .to_string())
    }

    fn notebook_read(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            path: String,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let path = self.resolve_path(&input.path);

        if !path.exists() {
            return Err(format!("Notebook not found: {}", path.display()));
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Read error: {}", e))?;

        let notebook: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid notebook JSON: {}", e))?;

        let cells = notebook.get("cells")
            .and_then(|c| c.as_array())
            .map(|cells| {
                cells.iter().enumerate().map(|(i, cell)| {
                    let cell_type = cell.get("cell_type")
                        .and_then(|c| c.as_str())
                        .unwrap_or("unknown");
                    let source = cell.get("source")
                        .and_then(|s| s.as_str())
                        .unwrap_or("");
                    let outputs = cell.get("outputs")
                        .and_then(|o| o.as_array())
                        .map(|arr| arr.len())
                        .unwrap_or(0);

                    serde_json::json!({
                        "index": i,
                        "type": cell_type,
                        "source": source,
                        "has_outputs": outputs > 0
                    })
                }).collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(serde_json::json!({
            "type": "notebook_read",
            "path": path.to_string_lossy(),
            "nbformat_version": notebook.get("nbformat"),
            "kernel": notebook.get("metadata")
                .and_then(|m| m.get("kernelspec"))
                .and_then(|k| k.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("unknown"),
            "cells": cells,
            "cell_count": cells.len()
        })
        .to_string())
    }

    fn notebook_edit(&self, input: &str) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Input {
            path: String,
            #[serde(default)]
            cell_index: Option<usize>,
            #[serde(default)]
            source: Option<String>,
            #[serde(default)]
            cell_type: Option<String>,
        }

        let input: Input =
            serde_json::from_str(input).map_err(|e| format!("Invalid input: {}", e))?;

        let path = self.resolve_path(&input.path);

        if !path.exists() {
            return Err(format!("Notebook not found: {}", path.display()));
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Read error: {}", e))?;

        let mut notebook: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid notebook JSON: {}", e))?;

        if let Some(cells) = notebook.get_mut("cells").and_then(|c| c.as_array_mut()) {
            if let Some(idx) = input.cell_index {
                if idx < cells.len() {
                    if let Some(source) = &input.source {
                        cells[idx]["source"] = serde_json::json!(source);
                    }
                    if let Some(cell_type) = &input.cell_type {
                        cells[idx]["cell_type"] = serde_json::json!(cell_type);
                    }
                } else {
                    return Err(format!("Cell index {} out of range (0-{})", idx, cells.len() - 1));
                }
            } else {
                // Add new cell
                let new_cell = serde_json::json!({
                    "cell_type": input.cell_type.as_deref().unwrap_or("code"),
                    "metadata": {},
                    "source": input.source.as_deref().unwrap_or(""),
                    "outputs": [],
                    "execution_count": null
                });
                cells.push(new_cell);
            }
        }

        let updated = serde_json::to_string_pretty(&notebook)
            .map_err(|e| format!("Serialize error: {}", e))?;

        fs::write(&path, updated)
            .map_err(|e| format!("Write error: {}", e))?;

        Ok(serde_json::json!({
            "type": "notebook_edit",
            "path": path.to_string_lossy(),
            "success": true,
            "message": "Notebook updated"
        })
        .to_string())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TodoItem {
    id: String,
    content: String,
    status: String,
    priority: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug)]
struct TodoStore {
    path: PathBuf,
}

impl TodoStore {
    fn new(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Dir error: {}", e))?;
        }
        Ok(Self { path })
    }

    fn load(&self) -> Result<Vec<TodoItem>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.path)
            .map_err(|e| format!("Read error: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Parse error: {}", e))
    }

    fn save(&self, todos: &[TodoItem]) -> Result<(), String> {
        let content = serde_json::to_string_pretty(todos)
            .map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(&self.path, content)
            .map_err(|e| format!("Write error: {}", e))
    }

    fn add(&self, todo: &TodoItem) -> Result<(), String> {
        let mut todos = self.load()?;
        todos.push(todo.clone());
        self.save(&todos)
    }

    fn list(&self, status: Option<&str>) -> Result<Vec<TodoItem>, String> {
        let todos = self.load()?;
        match status {
            Some(s) => Ok(todos.into_iter().filter(|t| t.status == s).collect()),
            None => Ok(todos),
        }
    }

    fn get(&self, id: &str) -> Result<TodoItem, String> {
        let todos = self.load()?;
        todos.into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Todo not found: {}", id))
    }

    fn update(&self, id: &str, content: Option<&str>, status: Option<&str>) -> Result<(), String> {
        let mut todos = self.load()?;
        let todo = todos.iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Todo not found: {}", id))?;

        if let Some(c) = content {
            todo.content = c.to_string();
        }
        if let Some(s) = status {
            todo.status = s.to_string();
        }
        todo.updated_at = chrono::Utc::now().to_rfc3339();

        self.save(&todos)
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let mut todos = self.load()?;
        let initial_len = todos.len();
        todos.retain(|t| t.id != id);

        if todos.len() == initial_len {
            return Err(format!("Todo not found: {}", id));
        }

        self.save(&todos)
    }
}
