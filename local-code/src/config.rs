//! 設定ファイル管理モジュール
//!
//! default.tomlから設定を読み込み、アプリケーション全体で使用できる
//! 型安全な設定構造体を提供します。

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// アプリケーション全体の設定
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// OLLAMA関連設定
    pub ollama: OllamaConfig,
    /// エージェント関連設定
    pub agent: AgentConfig,
    /// ツール関連設定
    pub tools: ToolsConfig,
    /// スキル関連設定
    #[serde(default)]
    pub skills: SkillsConfig,
    /// LSP関連設定
    #[serde(default)]
    pub lsp: LspConfig,
}

/// OLLAMA接続設定
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaConfig {
    /// OLLAMAサーバーのURL
    #[serde(default = "default_ollama_url")]
    pub url: String,
    /// 使用するモデル名
    #[serde(default = "default_model")]
    pub model: String,
    /// リクエストタイムアウト（秒）- 後方互換性のため維持
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// 接続タイムアウト（秒）
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,
    /// 読み取りタイムアウト（秒）
    #[serde(default = "default_read_timeout")]
    pub read_timeout: u64,
    /// リトライ設定
    #[serde(default)]
    pub retry: RetryConfig,
}

/// リトライ設定
#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    /// 最大リトライ回数
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// 初期バックオフ時間（ミリ秒）
    #[serde(default = "default_initial_backoff_ms")]
    pub initial_backoff_ms: u64,
    /// バックオフ倍率
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// 最大バックオフ時間（ミリ秒）
    #[serde(default = "default_max_backoff_ms")]
    pub max_backoff_ms: u64,
}

/// エージェント動作設定
#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    /// 初期モード (plan / execute)
    #[serde(default = "default_initial_mode")]
    pub initial_mode: String,
    /// 会話履歴の最大メッセージ数
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
}

/// ツール実行設定
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsConfig {
    /// Bashコマンドのタイムアウト（秒）
    #[serde(default = "default_bash_timeout")]
    pub bash_timeout: u64,
}

/// スキル設定
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SkillsConfig {
    /// カスタムスキルディレクトリパス（オプション）
    pub custom_path: Option<String>,
}

/// LSP設定
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LspConfig {
    /// LSPサーバーコマンド（未指定の場合は自動検出）
    pub command: Option<String>,
    /// LSPサーバー引数
    #[serde(default)]
    pub args: Vec<String>,
}

// デフォルト値を返す関数群
fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_model() -> String {
    "Rnj-1".to_string()
}

fn default_timeout() -> u64 {
    300
}

fn default_connect_timeout() -> u64 {
    30
}

fn default_read_timeout() -> u64 {
    300
}

fn default_initial_mode() -> String {
    "execute".to_string()
}

fn default_max_messages() -> usize {
    100
}

fn default_bash_timeout() -> u64 {
    120
}

// リトライ設定のデフォルト値
fn default_max_retries() -> u32 {
    3
}

fn default_initial_backoff_ms() -> u64 {
    1000 // 1秒
}

fn default_backoff_multiplier() -> f64 {
    2.0 // エクスポネンシャルバックオフ
}

fn default_max_backoff_ms() -> u64 {
    10000 // 最大10秒
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_backoff_ms: default_initial_backoff_ms(),
            backoff_multiplier: default_backoff_multiplier(),
            max_backoff_ms: default_max_backoff_ms(),
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            url: default_ollama_url(),
            model: default_model(),
            timeout: default_timeout(),
            connect_timeout: default_connect_timeout(),
            read_timeout: default_read_timeout(),
            retry: RetryConfig::default(),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            initial_mode: default_initial_mode(),
            max_messages: default_max_messages(),
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            bash_timeout: default_bash_timeout(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ollama: OllamaConfig::default(),
            agent: AgentConfig::default(),
            tools: ToolsConfig::default(),
            skills: SkillsConfig::default(),
            lsp: LspConfig::default(),
        }
    }
}

impl Config {
    /// TOMLファイルから設定を読み込む
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        Self::parse(&content)
    }

    /// TOML文字列から設定をパース
    pub fn parse(content: &str) -> Result<Self> {
        toml::from_str(content)
            .context("Failed to parse TOML config")
    }

    /// デフォルト設定ファイルパスを取得
    pub fn default_config_path() -> std::path::PathBuf {
        // 実行ファイルからの相対パス、または環境変数から取得
        if let Ok(config_path) = std::env::var("LOCAL_CODE_CONFIG") {
            return std::path::PathBuf::from(config_path);
        }

        // カレントディレクトリのconfig/default.toml
        let cwd_config = std::path::PathBuf::from("config/default.toml");
        if cwd_config.exists() {
            return cwd_config;
        }

        // ホームディレクトリの.local-code/config.toml
        if let Some(home) = dirs::home_dir() {
            let home_config = home.join(".local-code").join("config.toml");
            if home_config.exists() {
                return home_config;
            }
        }

        // デフォルトパス
        std::path::PathBuf::from("config/default.toml")
    }

    /// デフォルト設定ファイルから読み込み（存在しない場合は自動生成）
    pub fn load_default() -> Result<Self> {
        let config_path = Self::default_config_path();

        if config_path.exists() {
            Self::load_from_file(&config_path)
        } else {
            // 設定ファイルを自動生成
            if let Err(e) = Self::create_default_config(&config_path) {
                tracing::warn!("Failed to create default config: {}", e);
            } else {
                tracing::info!("Created default config at {}", config_path.display());
            }
            Ok(Self::default())
        }
    }

    /// デフォルト設定ファイルを生成
    fn create_default_config(path: &std::path::Path) -> Result<()> {
        // 親ディレクトリを作成
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        let default_content = r#"# local-code default configuration

[ollama]
url = "http://localhost:11434"
model = "Rnj-1"
connect_timeout = 30   # seconds
read_timeout = 300     # seconds

[ollama.retry]
max_retries = 3
initial_backoff_ms = 1000
backoff_multiplier = 2.0
max_backoff_ms = 10000

[agent]
initial_mode = "execute"
max_messages = 100

[tools]
bash_timeout = 120     # seconds

[skills]
# custom_path = "/path/to/custom/skills"

[lsp]
# command = "rust-analyzer"
# args = []
"#;

        std::fs::write(path, default_content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// 初期モードをModeに変換
    pub fn get_initial_mode(&self) -> crate::agent::Mode {
        match self.agent.initial_mode.to_lowercase().as_str() {
            "plan" => crate::agent::Mode::Plan,
            _ => crate::agent::Mode::Execute,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default_config() {
        let toml_content = r#"
[ollama]
url = "http://localhost:11434"
model = "Rnj-1"

[agent]
initial_mode = "execute"
max_messages = 100

[tools]
bash_timeout = 120
"#;
        let config = Config::parse(toml_content).unwrap();

        assert_eq!(config.ollama.url, "http://localhost:11434");
        assert_eq!(config.ollama.model, "Rnj-1");
        assert_eq!(config.agent.initial_mode, "execute");
        assert_eq!(config.agent.max_messages, 100);
        assert_eq!(config.tools.bash_timeout, 120);
    }

    #[test]
    fn test_default_values() {
        let config = Config::default();

        assert_eq!(config.ollama.url, "http://localhost:11434");
        assert_eq!(config.ollama.model, "Rnj-1");
        assert_eq!(config.ollama.timeout, 300);
        assert_eq!(config.ollama.connect_timeout, 30);
        assert_eq!(config.ollama.read_timeout, 300);
        assert_eq!(config.agent.initial_mode, "execute");
        assert_eq!(config.agent.max_messages, 100);
        assert_eq!(config.tools.bash_timeout, 120);
    }

    #[test]
    fn test_timeout_config() {
        let toml_content = r#"
[ollama]
url = "http://localhost:11434"
model = "test-model"
connect_timeout = 60
read_timeout = 600

[agent]
initial_mode = "execute"

[tools]
bash_timeout = 120
"#;
        let config = Config::parse(toml_content).unwrap();

        assert_eq!(config.ollama.connect_timeout, 60);
        assert_eq!(config.ollama.read_timeout, 600);
    }

    #[test]
    fn test_retry_config() {
        let toml_content = r#"
[ollama]
url = "http://localhost:11434"

[ollama.retry]
max_retries = 5
initial_backoff_ms = 2000
backoff_multiplier = 1.5
max_backoff_ms = 30000

[agent]
initial_mode = "execute"

[tools]
bash_timeout = 120
"#;
        let config = Config::parse(toml_content).unwrap();

        assert_eq!(config.ollama.retry.max_retries, 5);
        assert_eq!(config.ollama.retry.initial_backoff_ms, 2000);
        assert_eq!(config.ollama.retry.backoff_multiplier, 1.5);
        assert_eq!(config.ollama.retry.max_backoff_ms, 30000);
    }

    #[test]
    fn test_partial_config() {
        let toml_content = r#"
[ollama]
url = "http://custom:11434"

[agent]
initial_mode = "plan"

[tools]
bash_timeout = 60
"#;
        let config = Config::parse(toml_content).unwrap();

        assert_eq!(config.ollama.url, "http://custom:11434");
        assert_eq!(config.ollama.model, "Rnj-1"); // デフォルト値
        assert_eq!(config.agent.initial_mode, "plan");
        assert_eq!(config.tools.bash_timeout, 60);
    }

    #[test]
    fn test_get_initial_mode() {
        let mut config = Config::default();

        config.agent.initial_mode = "plan".to_string();
        assert!(matches!(config.get_initial_mode(), crate::agent::Mode::Plan));

        config.agent.initial_mode = "execute".to_string();
        assert!(matches!(config.get_initial_mode(), crate::agent::Mode::Execute));

        config.agent.initial_mode = "PLAN".to_string();
        assert!(matches!(config.get_initial_mode(), crate::agent::Mode::Plan));
    }
}
