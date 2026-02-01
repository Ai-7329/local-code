use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::client::LspClient;
use crate::tools::{Tool, ToolResult};

/// LSP定義ジャンプツール
pub struct LspDefinitionTool {
    client: Arc<Mutex<Option<LspClient>>>,
}

impl LspDefinitionTool {
    pub fn new(client: Arc<Mutex<Option<LspClient>>>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for LspDefinitionTool {
    fn name(&self) -> &str {
        "lsp_definition"
    }

    fn description(&self) -> &str {
        "Jump to the definition of a symbol at the specified position"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (0-indexed)"
                },
                "character": {
                    "type": "integer",
                    "description": "Character position (0-indexed)"
                }
            },
            "required": ["file_path", "line", "character"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let file_path = params.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing file_path"))?;
        let line = params.get("line")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing line"))? as u32;
        let character = params.get("character")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing character"))? as u32;

        let guard = self.client.lock().await;
        let client = guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("LSP client not initialized"))?;

        let path = PathBuf::from(file_path);
        client.did_open(&path).await?;
        match client.goto_definition(&path, line, character).await {
            Ok(Some(response)) => {
                Ok(ToolResult::success(serde_json::to_string_pretty(&response)?))
            }
            Ok(None) => {
                Ok(ToolResult::success("No definition found"))
            }
            Err(e) => {
                Ok(ToolResult::failure(format!("LSP error: {}", e)))
            }
        }
    }
}

/// LSP参照検索ツール
pub struct LspReferencesTool {
    client: Arc<Mutex<Option<LspClient>>>,
}

impl LspReferencesTool {
    pub fn new(client: Arc<Mutex<Option<LspClient>>>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for LspReferencesTool {
    fn name(&self) -> &str {
        "lsp_references"
    }

    fn description(&self) -> &str {
        "Find all references to a symbol at the specified position"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (0-indexed)"
                },
                "character": {
                    "type": "integer",
                    "description": "Character position (0-indexed)"
                }
            },
            "required": ["file_path", "line", "character"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let file_path = params.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing file_path"))?;
        let line = params.get("line")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing line"))? as u32;
        let character = params.get("character")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing character"))? as u32;

        let guard = self.client.lock().await;
        let client = guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("LSP client not initialized"))?;

        let path = PathBuf::from(file_path);
        client.did_open(&path).await?;
        match client.find_references(&path, line, character).await {
            Ok(Some(locations)) => {
                let output = locations.iter()
                    .map(|loc| format!("{}:{}:{}",
                        loc.uri.path(),
                        loc.range.start.line + 1,
                        loc.range.start.character + 1
                    ))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(ToolResult::success(if output.is_empty() {
                    "No references found".to_string()
                } else {
                    output
                }))
            }
            Ok(None) => {
                Ok(ToolResult::success("No references found"))
            }
            Err(e) => {
                Ok(ToolResult::failure(format!("LSP error: {}", e)))
            }
        }
    }
}

/// LSP診断情報ツール（プレースホルダー）
pub struct LspDiagnosticsTool {
    #[allow(dead_code)]
    client: Arc<Mutex<Option<LspClient>>>,
}

impl LspDiagnosticsTool {
    pub fn new(client: Arc<Mutex<Option<LspClient>>>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for LspDiagnosticsTool {
    fn name(&self) -> &str {
        "lsp_diagnostics"
    }

    fn description(&self) -> &str {
        "Get diagnostics (errors, warnings) for a file"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let file_path = params.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing file_path"))?;

        let guard = self.client.lock().await;
        let client = guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("LSP client not initialized"))?;

        let path = PathBuf::from(file_path);
        client.did_open(&path).await?;

        match client.document_diagnostics(&path).await {
            Ok(result) => {
                if let Some(items) = result.get("items").and_then(|v| v.as_array()) {
                    if items.is_empty() {
                        return Ok(ToolResult::success("No diagnostics found"));
                    }
                }
                Ok(ToolResult::success(serde_json::to_string_pretty(&result)?))
            }
            Err(e) => Ok(ToolResult::failure(format!("LSP error: {}", e))),
        }
    }
}
