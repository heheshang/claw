//! MCP (Model Context Protocol) server lifecycle management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerStatus {
    pub name: String,
    pub running: bool,
    pub tools: Vec<McpTool>,
    pub error: Option<String>,
}

pub struct McpManager {
    servers: Arc<RwLock<HashMap<String, McpServerConfig>>>,
    processes: Arc<RwLock<HashMap<String, mpsc::Sender<()>>>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add_server(&self, config: McpServerConfig) -> Result<(), String> {
        let mut servers = self.servers.write()
            .map_err(|e| format!("Lock error: {}", e))?;
        servers.insert(config.name.clone(), config);
        Ok(())
    }

    pub fn remove_server(&self, name: &str) -> Result<(), String> {
        // Stop the server if running
        self.stop_server(name)?;

        let mut servers = self.servers.write()
            .map_err(|e| format!("Lock error: {}", e))?;
        servers.remove(name);
        Ok(())
    }

    pub fn list_servers(&self) -> Result<Vec<String>, String> {
        let servers = self.servers.read()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(servers.keys().cloned().collect())
    }

    pub async fn start_server(&self, name: &str) -> Result<(), String> {
        let config = {
            let servers = self.servers.read()
                .map_err(|e| format!("Lock error: {}", e))?;
            servers.get(name)
                .cloned()
                .ok_or_else(|| format!("Server not found: {}", name))?
        };

        log::info!("[MCP] Starting server: {}", name);

        // Create a cancellation channel
        let (tx, mut rx) = mpsc::channel::<()>(1);

        // Store the sender
        {
            let mut processes = self.processes.write()
                .map_err(|e| format!("Lock error: {}", e))?;
            processes.insert(name.to_string(), tx);
        }

        // Spawn the process
        let name_clone = name.to_string();
        let processes_clone = self.processes.clone();

        tokio::spawn(async move {
            let mut child = Command::new(&config.command)
                .args(&config.args)
                .envs(&config.env)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to spawn MCP server");

            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // Handle stdout (JSON-RPC messages)
            if let Some(stdout) = stdout {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log::debug!("[MCP {}] stdout: {}", name_clone, line);
                }
            }

            // Handle stderr (logs)
            if let Some(stderr) = stderr {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log::debug!("[MCP {}] stderr: {}", name_clone, line);
                }
            }

            // Wait for cancellation or process exit
            tokio::select! {
                _ = rx.recv() => {
                    log::info!("[MCP] Server {} shutdown requested", name_clone);
                    child.kill().await.ok();
                }
                status = child.wait() => {
                    match status {
                        Ok(s) => log::info!("[MCP] Server {} exited: {:?}", name_clone, s.code()),
                        Err(e) => log::error!("[MCP] Server {} wait error: {}", name_clone, e),
                    }
                }
            }

            // Remove from processes
            let mut processes = processes_clone.write()
                .map_err(|e| format!("Lock error: {}", e)).unwrap();
            processes.remove(&name_clone);
        });

        Ok(())
    }

    pub fn stop_server(&self, name: &str) -> Result<(), String> {
        let tx = {
            let processes = self.processes.read()
                .map_err(|e| format!("Lock error: {}", e))?;
            processes.get(name).cloned()
        };

        if let Some(tx) = tx {
            let name_clone = name.to_string();
            tokio::spawn(async move {
                tx.send(()).await.ok();
                log::info!("[MCP] Sent shutdown signal to {}", name_clone);
            });
        }

        Ok(())
    }

    pub fn get_status(&self, name: &str) -> Result<McpServerStatus, String> {
        let config = {
            let servers = self.servers.read()
                .map_err(|e| format!("Lock error: {}", e))?;
            servers.get(name).cloned()
        };

        let running = {
            let processes = self.processes.read()
                .map_err(|e| format!("Lock error: {}", e))?;
            processes.contains_key(name)
        };

        match config {
            Some(c) => Ok(McpServerStatus {
                name: c.name,
                running,
                tools: vec![], // Tools would be discovered on start
                error: None,
            }),
            None => Err(format!("Server not found: {}", name)),
        }
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize default MCP servers from configuration
pub fn init_default_servers(mcp_dir: &PathBuf) -> McpManager {
    let manager = McpManager::new();

    // Look for MCP configurations
    let config_paths = vec![
        mcp_dir.join("servers.json"),
        dirs::home_dir()
            .map(|h| h.join(".claude/mcp/servers.json"))
            .unwrap_or_else(|| PathBuf::from(".claude/mcp/servers.json")),
    ];

    for path in config_paths {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(servers) = serde_json::from_str::<Vec<McpServerConfig>>(&content) {
                    for server in servers {
                        if let Err(e) = manager.add_server(server) {
                            log::warn!("[MCP] Failed to add server: {}", e);
                        }
                    }
                }
            }
        }
    }

    manager
}
