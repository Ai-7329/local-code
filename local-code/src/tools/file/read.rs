use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

use crate::tools::{Tool, ToolResult};

/// ファイル読み込みツール
pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (0-indexed)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let file_path = params.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing file_path parameter"))?;

        let offset = params.get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let limit = params.get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        let path = Path::new(file_path);

        if !path.exists() {
            return Ok(ToolResult::failure(format!("File not found: {}", file_path)));
        }

        match fs::read_to_string(path).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                let selected: Vec<&str> = lines
                    .into_iter()
                    .skip(offset)
                    .take(limit.unwrap_or(usize::MAX))
                    .collect();

                let numbered: Vec<String> = selected
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>6}\t{}", offset + i + 1, line))
                    .collect();

                let output = format!(
                    "File: {} ({} lines)\n{}",
                    file_path,
                    total_lines,
                    numbered.join("\n")
                );

                Ok(ToolResult::success(output))
            }
            Err(e) => Ok(ToolResult::failure(format!("Failed to read file: {}", e))),
        }
    }
}
