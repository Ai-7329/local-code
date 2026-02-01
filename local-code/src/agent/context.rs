use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs;

/// プロジェクトコンテキスト（agent.md, CLAUDE.md等）
#[derive(Default)]
pub struct AgentContext {
    /// 読み込まれたコンテキストファイルの内容
    pub content: Option<String>,
    /// 読み込み元ファイルパス
    pub source_path: Option<PathBuf>,
}

impl AgentContext {
    /// プロジェクトルートからコンテキストファイルを探索・読み込み
    pub async fn load_from_project(project_root: &Path) -> Result<Self> {
        let candidates = [
            "agent.md",
            "AGENT.md",
            "CLAUDE.md",
            "claude.md",
        ];

        for filename in candidates {
            let path = project_root.join(filename);
            if path.exists() {
                let content = fs::read_to_string(&path).await?;
                tracing::info!("Loaded context from: {}", path.display());
                return Ok(Self {
                    content: Some(content),
                    source_path: Some(path),
                });
            }
        }

        tracing::info!("No agent context file found in {}", project_root.display());
        Ok(Self {
            content: None,
            source_path: None,
        })
    }

    /// システムプロンプト用にフォーマット
    pub fn as_system_prompt(&self) -> Option<String> {
        self.content.as_ref().map(|c| {
            format!(
                "# Project Context\n\
                 The following instructions are from the project's agent configuration file:\n\n\
                 {}\n",
                c
            )
        })
    }

    /// コンテキストが存在するかチェック
    pub fn has_context(&self) -> bool {
        self.content.is_some()
    }
}

