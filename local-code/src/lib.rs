//! local-code: OLLAMA連携コーディングエージェント
//!
//! Claude Codeライクなプランモード/実行モード切り替え、
//! Skills、agent.md参照、LSP連携を備えたRust製CLIツール。

pub mod agent;
pub mod cli;
pub mod config;
pub mod llm;
pub mod skills;
pub mod tools;

// 主要な型の再エクスポート
pub use agent::{Agent, AgentConfig, AgentContext, Conversation, Message, Mode, ModeManager, Role, CodeVerifier, VerificationResult};
pub use cli::{Command, CommandHandler, CommandResult, Repl};
pub use config::{Config, OllamaConfig, AgentConfig as ConfigAgentConfig, ToolsConfig, SkillsConfig, LspConfig};
pub use llm::{OllamaClient, StreamingResponse, ToolCall, ToolCallParser};
pub use skills::{Skill, SkillExecutor, SkillMetadata, SkillRegistry, TriggerDetector};
pub use tools::{Tool, ToolDefinition, ToolRegistry, ToolResult};

/// バージョン情報
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// デフォルトのOLLAMA URL
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// デフォルトのモデル名
pub const DEFAULT_MODEL: &str = "Rnj-1";
