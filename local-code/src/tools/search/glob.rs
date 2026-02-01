use anyhow::Result;
use async_trait::async_trait;
use glob::glob as glob_pattern;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::tools::{Tool, ToolResult};

/// Globパターン検索ツール
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g., '**/*.rs')"
                },
                "path": {
                    "type": "string",
                    "description": "Base directory to search in (defaults to current directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let pattern = params.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing pattern parameter"))?;

        let base_path = params.get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let full_pattern = base_path.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let mut matches: Vec<String> = Vec::new();

        match glob_pattern(&pattern_str) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    matches.push(entry.display().to_string());
                }
            }
            Err(e) => {
                return Ok(ToolResult::failure(format!("Invalid glob pattern: {}", e)));
            }
        }

        if matches.is_empty() {
            Ok(ToolResult::success("No files found matching the pattern"))
        } else {
            Ok(ToolResult::success(format!(
                "Found {} files:\n{}",
                matches.len(),
                matches.join("\n")
            )))
        }
    }
}
