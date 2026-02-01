pub mod registry;
pub mod file;
pub mod search;
pub mod bash;
pub mod git;
pub mod lsp;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// ツール実行結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 成功したかどうか
    pub success: bool,
    /// 結果出力
    pub output: String,
    /// エラーメッセージ（失敗時）
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error.into()),
        }
    }
}

/// ツールの定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// ツール名
    pub name: String,
    /// 説明
    pub description: String,
    /// パラメータスキーマ（JSON Schema形式）
    pub parameters: Value,
}

/// ツールトレイト - 全ツールが実装する必要がある
#[async_trait]
pub trait Tool: Send + Sync {
    /// ツール名を取得
    fn name(&self) -> &str;

    /// ツールの説明を取得
    fn description(&self) -> &str;

    /// パラメータスキーマを取得（JSON Schema）
    fn parameters_schema(&self) -> Value;

    /// ツールを実行
    async fn execute(&self, params: Value) -> Result<ToolResult>;

    /// ツール定義を取得
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }
}

pub use registry::ToolRegistry;
