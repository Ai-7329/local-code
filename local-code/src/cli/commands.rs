use crate::agent::mode::ModeManager;
use crate::agent::history::HistoryManager;
use crate::skills::SkillRegistry;
use std::collections::HashMap;

/// Unix timestampを人間が読める形式に変換
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let datetime = UNIX_EPOCH + Duration::from_secs(timestamp);
    if let Ok(duration) = SystemTime::now().duration_since(datetime) {
        let secs = duration.as_secs();
        if secs < 60 {
            return "just now".to_string();
        } else if secs < 3600 {
            let mins = secs / 60;
            return format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" });
        } else if secs < 86400 {
            let hours = secs / 3600;
            return format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" });
        } else {
            let days = secs / 86400;
            return format!("{} day{} ago", days, if days == 1 { "" } else { "s" });
        }
    }
    "unknown".to_string()
}

/// CLIコマンド
#[derive(Debug, Clone)]
pub enum Command {
    /// ヘルプ表示
    Help,
    /// 終了
    Quit,
    /// Planモードに切り替え
    Plan,
    /// Executeモードに切り替え
    Execute,
    /// 画面クリア
    Clear,
    /// スキル実行
    Skill { name: String, args: Option<String> },
    /// モデル変更
    Model { name: String },
    /// 現在の状態を表示
    Status,
    /// スキル一覧表示
    Skills,
    /// 会話を保存
    Save { name: String },
    /// 会話を読み込み
    Load { name: String },
    /// 保存された会話一覧を表示
    History,
    /// 不明なコマンド
    Unknown(String),
    /// 通常のメッセージ（コマンドではない）
    Message(String),
}

impl Command {
    /// 入力テキストをコマンドにパース
    pub fn parse(input: &str) -> Self {
        let input = input.trim();

        if !input.starts_with('/') {
            return Command::Message(input.to_string());
        }

        let parts: Vec<&str> = input[1..].splitn(2, char::is_whitespace).collect();
        let cmd = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let args = parts.get(1).map(|s| s.trim().to_string());

        match cmd.as_str() {
            "help" | "h" | "?" => Command::Help,
            "quit" | "q" | "exit" => Command::Quit,
            "plan" => Command::Plan,
            "execute" | "exec" => Command::Execute,
            "clear" | "cls" => Command::Clear,
            "model" => {
                if let Some(name) = args {
                    Command::Model { name }
                } else {
                    Command::Unknown("/model requires a model name".to_string())
                }
            }
            "status" => Command::Status,
            "skills" => Command::Skills,
            "save" => {
                if let Some(name) = args {
                    Command::Save { name }
                } else {
                    Command::Unknown("/save requires a conversation name".to_string())
                }
            }
            "load" => {
                if let Some(name) = args {
                    Command::Load { name }
                } else {
                    Command::Unknown("/load requires a conversation name".to_string())
                }
            }
            "history" | "hist" => Command::History,
            _ => {
                // 未知のコマンドはスキルとして扱う
                Command::Skill {
                    name: cmd,
                    args,
                }
            }
        }
    }
}

/// コマンドハンドラー
pub struct CommandHandler {
    mode_manager: ModeManager,
    history_manager: Option<HistoryManager>,
    skill_aliases: HashMap<String, String>,
}

impl CommandHandler {
    pub fn new(mode_manager: ModeManager) -> Self {
        let history_manager = HistoryManager::new().ok();
        Self {
            mode_manager,
            history_manager,
            skill_aliases: HashMap::new(),
        }
    }

    /// HistoryManagerを指定してCommandHandlerを作成
    pub fn with_history_manager(mode_manager: ModeManager, history_manager: HistoryManager) -> Self {
        Self {
            mode_manager,
            history_manager: Some(history_manager),
            skill_aliases: HashMap::new(),
        }
    }

    /// スキルエイリアスを設定
    pub fn with_skill_aliases(mut self, aliases: HashMap<String, String>) -> Self {
        self.skill_aliases = aliases;
        self
    }

    /// HistoryManagerへの参照を取得
    pub fn history_manager(&self) -> Option<&HistoryManager> {
        self.history_manager.as_ref()
    }

    /// コマンドを処理
    pub async fn handle(&self, command: &Command, skill_registry: &SkillRegistry) -> CommandResult {
        match command {
            Command::Help => {
                CommandResult::Output(self.help_text())
            }
            Command::Quit => {
                CommandResult::Exit
            }
            Command::Plan => {
                self.mode_manager.to_plan().await;
                CommandResult::Output("Switched to Plan mode (read-only tools)".to_string())
            }
            Command::Execute => {
                self.mode_manager.to_execute().await;
                CommandResult::Output("Switched to Execute mode (all tools available)".to_string())
            }
            Command::Clear => {
                CommandResult::Clear
            }
            Command::Status => {
                let mode = self.mode_manager.current().await;
                let tools = self.mode_manager.allowed_tools().await;
                CommandResult::Output(format!(
                    "Mode: {}\nAllowed tools: {}",
                    mode,
                    tools.join(", ")
                ))
            }
            Command::Skills => {
                let names = skill_registry.names();
                if names.is_empty() {
                    CommandResult::Output("No skills loaded".to_string())
                } else {
                    CommandResult::Output(format!(
                        "Available skills:\n{}",
                        names.iter().map(|n| format!("  /{}", n)).collect::<Vec<_>>().join("\n")
                    ))
                }
            }
            Command::Skill { name, args } => {
                let effective_name = self
                    .skill_aliases
                    .get(name)
                    .map(|s| s.as_str())
                    .unwrap_or(name);

                if let Some(_skill) = skill_registry.get(effective_name) {
                    CommandResult::Skill {
                        name: effective_name.to_string(),
                        args: args.clone(),
                    }
                } else {
                    CommandResult::Output(format!(
                        "Unknown skill: {}. Use /skills to list available skills.",
                        name
                    ))
                }
            }
            Command::Model { name } => {
                CommandResult::ChangeModel { name: name.clone() }
            }
            Command::Unknown(msg) => {
                CommandResult::Output(format!("Unknown command: {}", msg))
            }
            Command::Message(msg) => {
                CommandResult::SendToLLM(msg.clone())
            }
            Command::Save { name } => {
                CommandResult::SaveConversation { name: name.clone() }
            }
            Command::Load { name } => {
                CommandResult::LoadConversation { name: name.clone() }
            }
            Command::History => {
                self.list_history()
            }
        }
    }

    /// 保存された会話履歴の一覧を表示
    fn list_history(&self) -> CommandResult {
        match &self.history_manager {
            Some(manager) => {
                match manager.list() {
                    Ok(entries) => {
                        if entries.is_empty() {
                            CommandResult::Output("No saved conversations found.".to_string())
                        } else {
                            let mut output = String::from("Saved conversations:\n");
                            for entry in entries {
                                let datetime = format_timestamp(entry.saved_at);
                                output.push_str(&format!(
                                    "  {} ({} messages) - {}\n",
                                    entry.name,
                                    entry.message_count,
                                    datetime
                                ));
                            }
                            output.push_str("\nUse /load <name> to restore a conversation.");
                            CommandResult::Output(output)
                        }
                    }
                    Err(e) => CommandResult::Output(format!("Failed to list history: {}", e))
                }
            }
            None => CommandResult::Output("History manager is not available.".to_string())
        }
    }

    fn help_text(&self) -> String {
        r#"
Commands:
  /help, /h, /?   - Show this help message
  /quit, /q       - Exit the REPL
  /plan           - Switch to Plan mode (read-only tools)
  /execute, /exec - Switch to Execute mode (all tools)
  /clear, /cls    - Clear the screen
  /status         - Show current mode and available tools
  /skills         - List available skills
  /model <name>   - Change the model
  /save <name>    - Save current conversation
  /load <name>    - Load a saved conversation
  /history, /hist - List saved conversations
  /<skill-name>   - Run a skill

Enter text to chat with the AI.
"#.to_string()
    }
}

/// コマンド実行結果
#[derive(Debug)]
pub enum CommandResult {
    /// テキスト出力
    Output(String),
    /// 終了
    Exit,
    /// 画面クリア
    Clear,
    /// LLMにメッセージ送信
    SendToLLM(String),
    /// モデル変更
    ChangeModel { name: String },
    /// スキル実行
    Skill { name: String, args: Option<String> },
    /// 会話を保存
    SaveConversation { name: String },
    /// 会話を読み込み
    LoadConversation { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commands() {
        assert!(matches!(Command::parse("/help"), Command::Help));
        assert!(matches!(Command::parse("/quit"), Command::Quit));
        assert!(matches!(Command::parse("/plan"), Command::Plan));
        assert!(matches!(Command::parse("/execute"), Command::Execute));

        if let Command::Model { name } = Command::parse("/model gpt-4") {
            assert_eq!(name, "gpt-4");
        } else {
            panic!("Expected Model command");
        }

        if let Command::Skill { name, args } = Command::parse("/commit fix bug") {
            assert_eq!(name, "commit");
            assert_eq!(args, Some("fix bug".to_string()));
        } else {
            panic!("Expected Skill command");
        }

        if let Command::Message(msg) = Command::parse("hello world") {
            assert_eq!(msg, "hello world");
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn test_parse_help_aliases() {
        assert!(matches!(Command::parse("/h"), Command::Help));
        assert!(matches!(Command::parse("/?"), Command::Help));
    }

    #[test]
    fn test_parse_quit_aliases() {
        assert!(matches!(Command::parse("/q"), Command::Quit));
        assert!(matches!(Command::parse("/exit"), Command::Quit));
    }

    #[test]
    fn test_parse_execute_aliases() {
        assert!(matches!(Command::parse("/exec"), Command::Execute));
    }

    #[test]
    fn test_parse_clear_aliases() {
        assert!(matches!(Command::parse("/cls"), Command::Clear));
        assert!(matches!(Command::parse("/clear"), Command::Clear));
    }

    #[test]
    fn test_model_without_name() {
        if let Command::Unknown(msg) = Command::parse("/model") {
            assert!(msg.contains("requires"));
        } else {
            panic!("Expected Unknown command for /model without name");
        }
    }

    #[test]
    fn test_parse_skill_without_args() {
        if let Command::Skill { name, args } = Command::parse("/commit") {
            assert_eq!(name, "commit");
            assert!(args.is_none());
        } else {
            panic!("Expected Skill command");
        }
    }

    #[test]
    fn test_case_insensitive_commands() {
        assert!(matches!(Command::parse("/HELP"), Command::Help));
        assert!(matches!(Command::parse("/Plan"), Command::Plan));
        assert!(matches!(Command::parse("/QUIT"), Command::Quit));
    }

    #[test]
    fn test_whitespace_handling() {
        assert!(matches!(Command::parse("  /help  "), Command::Help));

        if let Command::Model { name } = Command::parse("/model   gpt-4  ") {
            assert_eq!(name, "gpt-4");
        } else {
            panic!("Expected Model command");
        }
    }

    #[test]
    fn test_empty_input() {
        if let Command::Message(msg) = Command::parse("") {
            assert_eq!(msg, "");
        } else {
            panic!("Expected empty Message");
        }
    }

    #[test]
    fn test_message_with_slash_in_middle() {
        // スラッシュで始まらない場合はメッセージとして扱う
        if let Command::Message(msg) = Command::parse("hello/world") {
            assert_eq!(msg, "hello/world");
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn test_parse_save_command() {
        if let Command::Save { name } = Command::parse("/save my-conversation") {
            assert_eq!(name, "my-conversation");
        } else {
            panic!("Expected Save command");
        }
    }

    #[test]
    fn test_parse_save_without_name() {
        if let Command::Unknown(msg) = Command::parse("/save") {
            assert!(msg.contains("requires"));
        } else {
            panic!("Expected Unknown command for /save without name");
        }
    }

    #[test]
    fn test_parse_load_command() {
        if let Command::Load { name } = Command::parse("/load my-conversation") {
            assert_eq!(name, "my-conversation");
        } else {
            panic!("Expected Load command");
        }
    }

    #[test]
    fn test_parse_load_without_name() {
        if let Command::Unknown(msg) = Command::parse("/load") {
            assert!(msg.contains("requires"));
        } else {
            panic!("Expected Unknown command for /load without name");
        }
    }

    #[test]
    fn test_parse_history_command() {
        assert!(matches!(Command::parse("/history"), Command::History));
        assert!(matches!(Command::parse("/hist"), Command::History));
    }
}
