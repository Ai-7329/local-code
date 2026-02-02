//! ツール実行確認ダイアログモジュール
//!
//! 危険なツール（bash, write, edit, git_commit）の実行前に
//! ユーザー確認を求めるダイアログ機能を提供

use std::io::{self, Write};
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};

// 危険なツールの定義はmode.rsに統合（重複排除）
pub use crate::agent::mode::{DANGEROUS_TOOLS, requires_confirmation};

/// 確認ダイアログの結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmResult {
    /// ユーザーが許可
    Approved,
    /// ユーザーが拒否
    Denied,
}

/// 確認ダイアログ構造体
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    /// 実行するアクション名
    action: String,
    /// アクションの詳細説明
    details: String,
    /// 自動承認モード（テスト用）
    auto_approve: bool,
}

impl ConfirmDialog {
    /// 新しい確認ダイアログを作成
    pub fn new(action: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            details: details.into(),
            auto_approve: false,
        }
    }

    /// 自動承認モードを設定（テスト用）
    pub fn with_auto_approve(mut self, auto_approve: bool) -> Self {
        self.auto_approve = auto_approve;
        self
    }

    /// アクション名を取得
    pub fn action(&self) -> &str {
        &self.action
    }

    /// 詳細説明を取得
    pub fn details(&self) -> &str {
        &self.details
    }

    /// 確認プロンプトを表示して結果を取得
    pub fn show(&self) -> io::Result<ConfirmResult> {
        if self.auto_approve {
            return Ok(ConfirmResult::Approved);
        }

        let mut stdout = io::stdout();

        // 警告ヘッダーを黄色で表示
        execute!(
            stdout,
            SetForegroundColor(Color::Yellow),
            Print("\n--- Tool Confirmation Required ---\n"),
            ResetColor
        )?;

        // アクション名をシアンで表示
        execute!(
            stdout,
            SetForegroundColor(Color::Cyan),
            Print(format!("Action: {}\n", self.action)),
            ResetColor
        )?;

        // 詳細を表示
        if !self.details.is_empty() {
            execute!(
                stdout,
                Print(format!("Details: {}\n", self.details))
            )?;
        }

        // プロンプトを表示（デフォルトはNo）
        execute!(
            stdout,
            SetForegroundColor(Color::Yellow),
            Print("Execute? [y/N]: "),
            ResetColor
        )?;
        stdout.flush()?;

        // ユーザー入力を読み取り
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let trimmed = input.trim().to_lowercase();

        // 明示的に "y" または "yes" の場合のみ承認
        if trimmed == "y" || trimmed == "yes" {
            Ok(ConfirmResult::Approved)
        } else {
            // デフォルトは拒否（空入力含む）
            execute!(
                stdout,
                SetForegroundColor(Color::Red),
                Print("Execution denied.\n"),
                ResetColor
            )?;
            Ok(ConfirmResult::Denied)
        }
    }
}

// requires_confirmation は mode.rs から再エクスポート済み

/// 確認ダイアログを表示する便利関数
///
/// # Arguments
/// * `action` - 実行するアクション名
/// * `details` - アクションの詳細説明
///
/// # Returns
/// * `Ok(ConfirmResult)` - ユーザーの選択結果
/// * `Err` - I/Oエラー
pub fn confirm(action: impl Into<String>, details: impl Into<String>) -> io::Result<ConfirmResult> {
    let dialog = ConfirmDialog::new(action, details);
    dialog.show()
}

/// ツール実行前の確認を行う
///
/// # Arguments
/// * `tool_name` - ツール名
/// * `details` - 実行詳細
///
/// # Returns
/// * `Ok(true)` - 実行許可
/// * `Ok(false)` - 実行拒否
/// * `Err` - I/Oエラー
pub fn confirm_tool_execution(tool_name: &str, details: &str) -> io::Result<bool> {
    if !requires_confirmation(tool_name) {
        return Ok(true);
    }

    let result = confirm(
        format!("Execute tool: {}", tool_name),
        details
    )?;

    Ok(result == ConfirmResult::Approved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requires_confirmation() {
        // 危険なツールは確認が必要
        assert!(requires_confirmation("bash"));
        assert!(requires_confirmation("write"));
        assert!(requires_confirmation("edit"));
        assert!(requires_confirmation("git_commit"));

        // 安全なツールは確認不要
        assert!(!requires_confirmation("read"));
        assert!(!requires_confirmation("glob"));
        assert!(!requires_confirmation("grep"));
    }

    #[test]
    fn test_confirm_dialog_creation() {
        let dialog = ConfirmDialog::new("test_action", "test details");
        assert_eq!(dialog.action(), "test_action");
        assert_eq!(dialog.details(), "test details");
    }

    #[test]
    fn test_auto_approve() {
        let dialog = ConfirmDialog::new("test", "details")
            .with_auto_approve(true);

        let result = dialog.show().unwrap();
        assert_eq!(result, ConfirmResult::Approved);
    }

    #[test]
    fn test_confirm_result_eq() {
        assert_eq!(ConfirmResult::Approved, ConfirmResult::Approved);
        assert_eq!(ConfirmResult::Denied, ConfirmResult::Denied);
        assert_ne!(ConfirmResult::Approved, ConfirmResult::Denied);
    }
}
