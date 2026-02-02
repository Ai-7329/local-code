use std::sync::Arc;
use tokio::sync::RwLock;

/// エージェントの動作モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// 計画モード: 読み取り専用ツールのみ使用可能
    Plan,
    /// 実行モード: 全ツール使用可能
    #[default]
    Execute,
}

/// Planモードで許可されるツール（読み取り専用）
const PLAN_TOOLS: &[&str] = &[
    "read",
    "glob",
    "grep",
    "git_status",
    "git_diff",
    "git_log",
    "lsp_definition",
    "lsp_references",
    "lsp_diagnostics",
];

/// Executeモードで許可されるツール（全ツール）
const EXECUTE_TOOLS: &[&str] = &[
    "read",
    "write",
    "edit",
    "bash",
    "glob",
    "grep",
    "git_status",
    "git_diff",
    "git_add",
    "git_commit",
    "git_log",
    "lsp_definition",
    "lsp_references",
    "lsp_diagnostics",
];

/// 確認が必要な危険なツール（書き込み系）
pub const DANGEROUS_TOOLS: &[&str] = &["bash", "write", "edit", "git_commit"];

/// ツールが確認を必要とするか判定
pub fn requires_confirmation(tool_name: &str) -> bool {
    DANGEROUS_TOOLS.contains(&tool_name)
}

impl Mode {
    /// モードごとに許可されるツール名のスライスを取得（毎回Vecを生成しない）
    pub fn allowed_tools(&self) -> &'static [&'static str] {
        match self {
            Mode::Plan => PLAN_TOOLS,
            Mode::Execute => EXECUTE_TOOLS,
        }
    }

    /// 指定ツールが現在のモードで使用可能かチェック
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.allowed_tools().contains(&tool_name)
    }

    /// モード名を文字列で取得
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Plan => "plan",
            Mode::Execute => "execute",
        }
    }

    /// 文字列からモードを取得
    pub fn parse_mode(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "plan" => Some(Mode::Plan),
            "execute" | "exec" => Some(Mode::Execute),
            _ => None,
        }
    }

    /// 次のモードを取得（サイクル: Plan → Execute → Plan）
    pub fn next(&self) -> Self {
        match self {
            Mode::Plan => Mode::Execute,
            Mode::Execute => Mode::Plan,
        }
    }

    /// モードに対応するアイコンを取得
    pub fn icon(&self) -> &'static str {
        match self {
            Mode::Plan => "📋",
            Mode::Execute => "⏵⏵",
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// モードマネージャー - スレッドセーフなモード管理
#[derive(Clone)]
pub struct ModeManager {
    current: Arc<RwLock<Mode>>,
}

impl ModeManager {
    pub fn new(initial_mode: Mode) -> Self {
        Self {
            current: Arc::new(RwLock::new(initial_mode)),
        }
    }

    /// 現在のモードを取得
    pub async fn current(&self) -> Mode {
        *self.current.read().await
    }

    /// モードを切り替え
    pub async fn set(&self, mode: Mode) {
        *self.current.write().await = mode;
    }

    /// Planモードに切り替え
    pub async fn to_plan(&self) {
        self.set(Mode::Plan).await;
    }

    /// Executeモードに切り替え
    pub async fn to_execute(&self) {
        self.set(Mode::Execute).await;
    }

    /// ツールが現在のモードで使用可能かチェック
    pub async fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.current().await.is_tool_allowed(tool_name)
    }

    /// 現在許可されているツール名一覧を取得
    pub async fn allowed_tools(&self) -> &'static [&'static str] {
        self.current().await.allowed_tools()
    }
}

impl Default for ModeManager {
    fn default() -> Self {
        Self::new(Mode::Execute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_allowed_tools() {
        let plan = Mode::Plan;
        assert!(plan.is_tool_allowed("read"));
        assert!(plan.is_tool_allowed("glob"));
        assert!(!plan.is_tool_allowed("write"));
        assert!(!plan.is_tool_allowed("bash"));

        let execute = Mode::Execute;
        assert!(execute.is_tool_allowed("read"));
        assert!(execute.is_tool_allowed("write"));
        assert!(execute.is_tool_allowed("bash"));
    }

    #[test]
    fn test_mode_from_str() {
        assert_eq!(Mode::parse_mode("plan"), Some(Mode::Plan));
        assert_eq!(Mode::parse_mode("PLAN"), Some(Mode::Plan));
        assert_eq!(Mode::parse_mode("execute"), Some(Mode::Execute));
        assert_eq!(Mode::parse_mode("exec"), Some(Mode::Execute));
        assert_eq!(Mode::parse_mode("invalid"), None);
    }

    #[tokio::test]
    async fn test_mode_manager() {
        let manager = ModeManager::new(Mode::Execute);
        assert_eq!(manager.current().await, Mode::Execute);

        manager.to_plan().await;
        assert_eq!(manager.current().await, Mode::Plan);
        assert!(!manager.is_tool_allowed("bash").await);

        manager.to_execute().await;
        assert!(manager.is_tool_allowed("bash").await);
    }
}
