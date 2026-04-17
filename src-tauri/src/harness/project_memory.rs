//! CLAUDE.md project memory support
//!
//! This module handles loading and parsing CLAUDE.md files
//! which contain project-specific instructions and context.

use std::path::PathBuf;
use std::fs;

/// CLAUDE.md file locations to search (in order of priority)
const CLAUDE_FILES: &[&str] = &[
    "CLAUDE.md",
    ".claude/CLAUDE.md",
    ".github/CLAUDE.md",
];

/// Load CLAUDE.md content from the workspace
pub fn load_claude_md(workspace_root: &PathBuf) -> Option<String> {
    // First check for environment variable override
    if let Ok(path) = std::env::var("CLAUDE_MD_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return fs::read_to_string(&path).ok();
        }
    }

    // Search standard locations
    for &filename in CLAUDE_FILES {
        let path = workspace_root.join(filename);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                log::info!("[PROJECT_MEMORY] Loaded {} for project context", path.display());
                return Some(content);
            }
        }
    }

    // Search parent directories (up to 5 levels)
    let mut dir = workspace_root.as_path();
    for _ in 0..5 {
        if let Some(parent) = dir.parent() {
            for &filename in CLAUDE_FILES {
                let path = parent.join(filename);
                if path.exists() {
                    if let Ok(content) = fs::read_to_string(&path) {
                        log::info!("[PROJECT_MEMORY] Loaded {} for project context", path.display());
                        return Some(content);
                    }
                }
            }
            dir = parent;
        } else {
            break;
        }
    }

    log::debug!("[PROJECT_MEMORY] No CLAUDE.md found");
    None
}

/// Parse CLAUDE.md and extract sections
#[derive(Debug, Clone)]
pub struct ProjectMemory {
    pub full_content: String,
    pub sections: Vec<MemorySection>,
}

#[derive(Debug, Clone)]
pub struct MemorySection {
    pub name: String,
    pub content: String,
    pub line_start: usize,
    pub line_end: usize,
}

impl ProjectMemory {
    pub fn parse(content: &str) -> Self {
        let mut sections = Vec::new();
        let mut current_section: Option<(String, String, usize)> = None;

        for (i, line) in content.lines().enumerate() {
            // Check for section headers (## Section Name or ### Section)
            if line.starts_with("## ") || line.starts_with("### ") {
                // Save previous section
                if let Some((name, content, start)) = current_section.take() {
                    sections.push(MemorySection {
                        name,
                        content: content.trim().to_string(),
                        line_start: start,
                        line_end: i - 1,
                    });
                }

                let name = line.trim_start_matches('#').trim().to_string();
                current_section = Some((name, String::new(), i));
            } else if let Some((_, ref mut content, _)) = current_section {
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(line);
            }
        }

        // Save last section
        if let Some((name, content, start)) = current_section {
            sections.push(MemorySection {
                name,
                content: content.trim().to_string(),
                line_start: start,
                line_end: content.lines().count(),
            });
        }

        Self {
            full_content: content.to_string(),
            sections,
        }
    }

    /// Get relevant sections based on keywords
    pub fn get_relevant(&self, keywords: &[&str]) -> Vec<&MemorySection> {
        self.sections
            .iter()
            .filter(|s| {
                keywords.iter().any(|k| {
                    s.name.to_lowercase().contains(&k.to_lowercase())
                        || s.content.to_lowercase().contains(&k.to_lowercase())
                })
            })
            .collect()
    }
}

/// Format CLAUDE.md content for inclusion in system prompt
pub fn format_for_system_prompt(memory: &ProjectMemory) -> String {
    if memory.full_content.is_empty() {
        return String::new();
    }

    format!(
        "\n\n## Project Context (from CLAUDE.md)\n\n{}\n\n## End Project Context\n",
        memory.full_content
    )
}
