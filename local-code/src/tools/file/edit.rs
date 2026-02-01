use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

use crate::tools::{Tool, ToolResult};

/// ファイル編集ツール（部分置換）
pub struct EditTool;

impl EditTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing old_string with new_string"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The string to replace with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let file_path = params.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing file_path parameter"))?;

        let old_string = params.get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing old_string parameter"))?;

        let new_string = params.get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing new_string parameter"))?;

        let replace_all = params.get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = Path::new(file_path);

        if !path.exists() {
            return Ok(ToolResult::failure(format!("File not found: {}", file_path)));
        }

        let content = match fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::failure(format!("Failed to read file: {}", e))),
        };

        // old_stringの出現回数をカウント
        let occurrences = content.matches(old_string).count();

        if occurrences == 0 {
            return Ok(ToolResult::failure(format!(
                "old_string not found in file: '{}'",
                if old_string.len() > 50 {
                    format!("{}...", &old_string[..50])
                } else {
                    old_string.to_string()
                }
            )));
        }

        if occurrences > 1 && !replace_all {
            return Ok(ToolResult::failure(format!(
                "old_string found {} times. Use replace_all: true to replace all, or provide a more unique string.",
                occurrences
            )));
        }

        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        match fs::write(path, &new_content).await {
            Ok(_) => {
                let replaced = if replace_all { occurrences } else { 1 };
                Ok(ToolResult::success(format!(
                    "Successfully replaced {} occurrence(s) in {}",
                    replaced,
                    file_path
                )))
            }
            Err(e) => Ok(ToolResult::failure(format!("Failed to write file: {}", e))),
        }
    }
}
