use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

use crate::tools::{Tool, ToolResult};

/// ファイル書き込みツール
pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file (creates or overwrites)"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let file_path = params.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing file_path parameter"))?;

        let content = params.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing content parameter"))?;

        let path = Path::new(file_path);

        // 親ディレクトリが存在しない場合は作成
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }

        match fs::write(path, content).await {
            Ok(_) => {
                let lines = content.lines().count();
                Ok(ToolResult::success(format!(
                    "Successfully wrote {} lines to {}",
                    lines,
                    file_path
                )))
            }
            Err(e) => Ok(ToolResult::failure(format!("Failed to write file: {}", e))),
        }
    }
}
