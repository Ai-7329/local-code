//! オートコンプリート機能
//!
//! REPLでのTabキーによる補完機能を提供する:
//! - `/`で始まるコマンド補完
//! - ファイルパス補完

use std::path::{Path, PathBuf};
use std::fs;

/// 利用可能なスラッシュコマンド一覧
const COMMANDS: &[&str] = &[
    "/help",
    "/h",
    "/?",
    "/quit",
    "/q",
    "/exit",
    "/plan",
    "/execute",
    "/exec",
    "/clear",
    "/cls",
    "/status",
    "/skills",
    "/model",
    "/save",
    "/load",
    "/history",
    "/hist",
];

/// オートコンプリーター
pub struct Completer {
    /// スキル名のリスト（動的に更新可能）
    skill_names: Vec<String>,
    /// 追加コマンド（動的に更新可能）
    extra_commands: Vec<String>,
    /// 現在の作業ディレクトリ
    working_dir: PathBuf,
}

impl Completer {
    /// 新しいCompleterを作成
    pub fn new() -> Self {
        Self {
            skill_names: Vec::new(),
            extra_commands: Vec::new(),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// スキル名を設定
    pub fn set_skills(&mut self, skills: Vec<String>) {
        self.skill_names = skills;
    }

    /// 追加コマンドを設定
    pub fn set_extra_commands(&mut self, commands: Vec<String>) {
        self.extra_commands = commands;
    }

    /// 作業ディレクトリを設定
    pub fn set_working_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }

    /// Superpowersコマンドのみを取得（空入力Tab用）
    pub fn get_superpowers_commands(&self) -> Vec<String> {
        self.extra_commands
            .iter()
            .map(|cmd| {
                if cmd.starts_with('/') { cmd.clone() } else { format!("/{}", cmd) }
            })
            .collect()
    }

    /// 入力に対する補完候補を取得
    ///
    /// # Arguments
    /// * `input` - 現在の入力文字列
    ///
    /// # Returns
    /// 補完候補のリスト
    pub fn complete(&self, input: &str) -> Vec<String> {
        if input.is_empty() {
            return Vec::new();
        }

        // スラッシュコマンドの補完
        if input.starts_with('/') {
            return self.complete_command(input);
        }

        // ファイルパス補完（パスセパレータを含む場合）
        if input.contains('/') || input.contains('\\') || input.starts_with('.') || input.starts_with('~') {
            return self.complete_path(input);
        }

        Vec::new()
    }

    /// コマンド補完
    fn complete_command(&self, input: &str) -> Vec<String> {
        let input_lower = input.to_lowercase();
        let mut candidates = Vec::new();

        // 組み込みコマンドの補完
        for cmd in COMMANDS {
            if cmd.to_lowercase().starts_with(&input_lower) {
                candidates.push(cmd.to_string());
            }
        }

        // スキル名の補完
        for skill in &self.skill_names {
            let skill_cmd = format!("/{}", skill);
            if skill_cmd.to_lowercase().starts_with(&input_lower) {
                candidates.push(skill_cmd);
            }
        }

        // 追加コマンドの補完
        for command in &self.extra_commands {
            let cmd = if command.starts_with('/') {
                command.to_string()
            } else {
                format!("/{}", command)
            };
            if cmd.to_lowercase().starts_with(&input_lower) {
                candidates.push(cmd);
            }
        }

        // 重複を除去してソート
        candidates.sort();
        candidates.dedup();
        candidates
    }

    /// ファイルパス補完
    fn complete_path(&self, input: &str) -> Vec<String> {
        let expanded = self.expand_tilde(input);
        let path = Path::new(&expanded);

        // 親ディレクトリと入力されたファイル名のプレフィックスを取得
        let (parent, prefix) = if path.is_dir() && expanded.ends_with('/') {
            (path.to_path_buf(), String::new())
        } else {
            let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| {
                if expanded.starts_with('/') {
                    PathBuf::from("/")
                } else {
                    self.working_dir.clone()
                }
            });
            let file_name = path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent, file_name)
        };

        // 親ディレクトリが存在しない場合は空を返す
        if !parent.exists() {
            return Vec::new();
        }

        // ディレクトリの内容を読み取り
        let entries = match fs::read_dir(&parent) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };

        let mut candidates = Vec::new();
        let prefix_lower = prefix.to_lowercase();

        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // 隠しファイルは.で始まる入力の場合のみ表示
            if file_name.starts_with('.') && !prefix.starts_with('.') {
                continue;
            }

            if file_name.to_lowercase().starts_with(&prefix_lower) {
                let full_path = if expanded.ends_with('/') || prefix.is_empty() {
                    format!("{}{}", expanded, file_name)
                } else {
                    let parent_str = path.parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if parent_str.is_empty() {
                        file_name.clone()
                    } else {
                        format!("{}/{}", parent_str, file_name)
                    }
                };

                // ディレクトリの場合は末尾に/を追加
                let is_dir = entry.path().is_dir();
                let candidate = if is_dir {
                    format!("{}/", full_path)
                } else {
                    full_path
                };

                candidates.push(candidate);
            }
        }

        candidates.sort();
        candidates
    }

    /// チルダをホームディレクトリに展開
    fn expand_tilde(&self, path: &str) -> String {
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                return path.replacen('~', &home.to_string_lossy(), 1);
            }
        }
        path.to_string()
    }

    /// 補完候補から共通プレフィックスを取得
    ///
    /// 複数の候補がある場合、共通する部分までを返す
    pub fn common_prefix(candidates: &[String]) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }

        if candidates.len() == 1 {
            return Some(candidates[0].clone());
        }

        let first = &candidates[0];
        let mut prefix_len = first.len();

        for candidate in &candidates[1..] {
            let mut common = 0;
            for (c1, c2) in first.chars().zip(candidate.chars()) {
                if c1.to_lowercase().next() == c2.to_lowercase().next() {
                    common += c1.len_utf8();
                } else {
                    break;
                }
            }
            prefix_len = prefix_len.min(common);
        }

        if prefix_len > 0 {
            Some(first[..prefix_len].to_string())
        } else {
            None
        }
    }
}

impl Default for Completer {
    fn default() -> Self {
        Self::new()
    }
}

/// 補完結果
#[derive(Debug, Clone)]
pub enum CompletionResult {
    /// 単一の補完候補（そのまま適用）
    Single(String),
    /// 複数の候補（共通プレフィックスと候補リスト）
    Multiple {
        common_prefix: String,
        candidates: Vec<String>,
    },
    /// 補完候補なし
    None,
}

impl Completer {
    /// 補完を実行し、結果を返す
    pub fn complete_with_result(&self, input: &str) -> CompletionResult {
        let candidates = self.complete(input);

        match candidates.len() {
            0 => CompletionResult::None,
            1 => CompletionResult::Single(candidates[0].clone()),
            _ => {
                let common = Self::common_prefix(&candidates)
                    .unwrap_or_else(|| input.to_string());
                CompletionResult::Multiple {
                    common_prefix: common,
                    candidates,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_completion() {
        let completer = Completer::new();

        // /heで始まるコマンド
        let candidates = completer.complete("/he");
        assert!(candidates.contains(&"/help".to_string()));

        // /exで始まるコマンド
        let candidates = completer.complete("/ex");
        assert!(candidates.contains(&"/execute".to_string()));
        assert!(candidates.contains(&"/exec".to_string()));
        assert!(candidates.contains(&"/exit".to_string()));
    }

    #[test]
    fn test_command_completion_with_skills() {
        let mut completer = Completer::new();
        completer.set_skills(vec!["commit".to_string(), "review".to_string()]);

        let candidates = completer.complete("/co");
        assert!(candidates.contains(&"/commit".to_string()));

        let candidates = completer.complete("/re");
        assert!(candidates.contains(&"/review".to_string()));
    }

    #[test]
    fn test_empty_input() {
        let completer = Completer::new();
        let candidates = completer.complete("");
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_no_match() {
        let completer = Completer::new();
        let candidates = completer.complete("/xyz");
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_common_prefix() {
        let candidates = vec![
            "/execute".to_string(),
            "/exec".to_string(),
            "/exit".to_string(),
        ];
        let prefix = Completer::common_prefix(&candidates);
        assert_eq!(prefix, Some("/ex".to_string()));
    }

    #[test]
    fn test_common_prefix_single() {
        let candidates = vec!["/help".to_string()];
        let prefix = Completer::common_prefix(&candidates);
        assert_eq!(prefix, Some("/help".to_string()));
    }

    #[test]
    fn test_common_prefix_empty() {
        let candidates: Vec<String> = vec![];
        let prefix = Completer::common_prefix(&candidates);
        assert_eq!(prefix, None);
    }

    #[test]
    fn test_completion_result_single() {
        let completer = Completer::new();
        let result = completer.complete_with_result("/hel");

        match result {
            CompletionResult::Single(s) => assert_eq!(s, "/help"),
            _ => panic!("Expected Single result"),
        }
    }

    #[test]
    fn test_completion_result_multiple() {
        let completer = Completer::new();
        let result = completer.complete_with_result("/ex");

        match result {
            CompletionResult::Multiple { common_prefix, candidates } => {
                assert_eq!(common_prefix, "/ex");
                assert!(candidates.len() > 1);
            }
            _ => panic!("Expected Multiple result"),
        }
    }

    #[test]
    fn test_completion_result_none() {
        let completer = Completer::new();
        let result = completer.complete_with_result("/xyz");

        match result {
            CompletionResult::None => {}
            _ => panic!("Expected None result"),
        }
    }

    #[test]
    fn test_case_insensitive() {
        let completer = Completer::new();

        let candidates_lower = completer.complete("/he");
        let candidates_upper = completer.complete("/HE");

        assert_eq!(candidates_lower, candidates_upper);
    }
}
