use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;
use glob::glob as glob_pattern;

use crate::tools::{Tool, ToolResult};

/// 内容検索ツール
pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., '*.rs')"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let pattern = params.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing pattern parameter"))?;

        let search_path = params.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let file_glob = params.get("glob")
            .and_then(|v| v.as_str());

        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => return Ok(ToolResult::failure(format!("Invalid regex: {}", e))),
        };

        let mut results: Vec<String> = Vec::new();
        let path = Path::new(search_path);

        if path.is_file() {
            // 単一ファイル検索
            if let Ok(content) = fs::read_to_string(path).await {
                for (i, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        results.push(format!("{}:{}:{}", path.display(), i + 1, line));
                    }
                }
            }
        } else if path.is_dir() {
            // ディレクトリ検索
            let glob_pattern_str = if let Some(g) = file_glob {
                format!("{}/{}", search_path, g)
            } else {
                format!("{}/**/*", search_path)
            };

            if let Ok(entries) = glob_pattern(&glob_pattern_str) {
                for entry in entries.flatten() {
                    if entry.is_file() {
                        if let Ok(content) = fs::read_to_string(&entry).await {
                            for (i, line) in content.lines().enumerate() {
                                if regex.is_match(line) {
                                    results.push(format!("{}:{}:{}", entry.display(), i + 1, line));
                                    if results.len() >= 100 {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if results.len() >= 100 {
                        break;
                    }
                }
            }
        }

        if results.is_empty() {
            Ok(ToolResult::success("No matches found"))
        } else {
            let truncated = if results.len() >= 100 { " (truncated)" } else { "" };
            Ok(ToolResult::success(format!(
                "Found {} matches{}:\n{}",
                results.len(),
                truncated,
                results.join("\n")
            )))
        }
    }
}
