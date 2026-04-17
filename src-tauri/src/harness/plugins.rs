//! Plugin system for extending claw-code functionality
//!
//! Plugins are external tools/commands that can be loaded dynamically
//! to extend the harness capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

/// Plugin manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub commands: Vec<PluginCommand>,
    pub dependencies: Option<Vec<String>>,
}

/// A command provided by a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    pub name: String,
    pub description: String,
    pub args: Vec<PluginArg>,
}

/// Argument specification for a plugin command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginArg {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
}

/// Plugin instance (loaded plugin)
#[derive(Debug, Clone)]
pub struct Plugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub enabled: bool,
}

/// Plugin manager for loading and managing plugins
pub struct PluginManager {
    plugins: RwLock<HashMap<String, Plugin>>,
    plugin_dirs: Vec<PathBuf>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            plugin_dirs: Self::default_plugin_dirs(),
        }
    }

    fn default_plugin_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // User plugins
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join(".claude/plugins"));
        }

        // Project plugins
        if let Ok(cwd) = std::env::current_dir() {
            dirs.push(cwd.join(".claude/plugins"));
        }

        // Global plugins
        dirs.push(PathBuf::from("/usr/local/lib/claw/plugins"));

        dirs
    }

    /// Add a plugin directory to search
    pub fn add_plugin_dir(&mut self, dir: PathBuf) {
        self.plugin_dirs.push(dir);
    }

    /// Discover and load all plugins
    pub fn discover_plugins(&self) -> Result<Vec<Plugin>, String> {
        let mut discovered = Vec::new();

        for dir in &self.plugin_dirs {
            if !dir.exists() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Ok(plugin) = self.load_plugin(&path) {
                            discovered.push(plugin);
                        }
                    }
                }
            }
        }

        Ok(discovered)
    }

    /// Load a plugin from a directory
    pub fn load_plugin(&self, path: &PathBuf) -> Result<Plugin, String> {
        let manifest_path = path.join("plugin.json");

        if !manifest_path.exists() {
            return Err(format!("No plugin.json found in {}", path.display()));
        }

        let content = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read plugin.json: {}", e))?;

        let manifest: PluginManifest = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse plugin.json: {}", e))?;

        let plugin = Plugin {
            manifest,
            path: path.clone(),
            enabled: true,
        };

        // Register the plugin
        {
            let mut plugins = self.plugins.write()
                .map_err(|e| format!("Lock error: {}", e))?;
            plugins.insert(plugin.manifest.name.clone(), plugin.clone());
        }

        log::info!("[PLUGIN] Loaded plugin: {}", plugin.manifest.name);

        Ok(plugin)
    }

    /// Enable a plugin
    pub fn enable_plugin(&self, name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write()
            .map_err(|e| format!("Lock error: {}", e))?;

        if let Some(plugin) = plugins.get_mut(name) {
            plugin.enabled = true;
            log::info!("[PLUGIN] Enabled: {}", name);
            Ok(())
        } else {
            Err(format!("Plugin not found: {}", name))
        }
    }

    /// Disable a plugin
    pub fn disable_plugin(&self, name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write()
            .map_err(|e| format!("Lock error: {}", e))?;

        if let Some(plugin) = plugins.get_mut(name) {
            plugin.enabled = false;
            log::info!("[PLUGIN] Disabled: {}", name);
            Ok(())
        } else {
            Err(format!("Plugin not found: {}", name))
        }
    }

    /// List all loaded plugins
    pub fn list_plugins(&self) -> Result<Vec<PluginInfo>, String> {
        let plugins = self.plugins.read()
            .map_err(|e| format!("Lock error: {}", e))?;

        Ok(plugins.values().map(|p| PluginInfo {
            name: p.manifest.name.clone(),
            version: p.manifest.version.clone(),
            description: p.manifest.description.clone(),
            enabled: p.enabled,
            commands: p.manifest.commands.len(),
        }).collect())
    }

    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Result<Plugin, String> {
        let plugins = self.plugins.read()
            .map_err(|e| format!("Lock error: {}", e))?;

        plugins.get(name)
            .cloned()
            .ok_or_else(|| format!("Plugin not found: {}", name))
    }

    /// Execute a plugin command
    pub fn execute_command(
        &self,
        plugin_name: &str,
        command_name: &str,
        args: &HashMap<String, String>,
    ) -> Result<String, String> {
        let plugin = self.get_plugin(plugin_name)?;

        if !plugin.enabled {
            return Err(format!("Plugin {} is disabled", plugin_name));
        }

        let cmd = plugin.manifest.commands.iter()
            .find(|c| c.name == command_name)
            .ok_or_else(|| format!("Command {} not found in plugin {}", command_name, plugin_name))?;

        // Build command
        let plugin_bin = plugin.path.join(&cmd.name);
        if !plugin_bin.exists() {
            return Err(format!("Plugin binary not found: {}", plugin_bin.display()));
        }

        let output = std::process::Command::new(&plugin_bin)
            .args(args.iter().map(|(k, v)| format!("--{}={}", k, v)))
            .current_dir(&plugin.path)
            .output()
            .map_err(|e| format!("Failed to execute plugin: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub commands: usize,
}
