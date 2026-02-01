//! 会話履歴の永続化管理
//!
//! ~/.local-code/history/ に会話をJSON形式で保存・読み込みする

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

use super::conversation::{Conversation, Message, Role};

/// 永続化用の会話データ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedConversation {
    /// 会話名
    pub name: String,
    /// 保存日時（Unix timestamp）
    pub saved_at: u64,
    /// メッセージ一覧
    pub messages: Vec<PersistedMessage>,
    /// メタデータ
    #[serde(default)]
    pub metadata: ConversationMetadata,
}

/// 永続化用のメッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

/// 会話メタデータ
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationMetadata {
    /// 作成日時
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    /// モデル名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// プロジェクトパス
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
}

/// 会話履歴一覧のエントリ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// 会話名
    pub name: String,
    /// 保存日時
    pub saved_at: u64,
    /// メッセージ数
    pub message_count: usize,
    /// ファイルパス
    pub path: PathBuf,
}

/// 会話履歴マネージャー
pub struct HistoryManager {
    /// 履歴保存ディレクトリ
    history_dir: PathBuf,
}

impl HistoryManager {
    /// 新しいHistoryManagerを作成
    ///
    /// デフォルトでは ~/.local-code/history/ を使用
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir()
            .context("Failed to get home directory")?;
        let history_dir = home.join(".local-code").join("history");

        Self::with_directory(history_dir)
    }

    /// 指定されたディレクトリでHistoryManagerを作成
    pub fn with_directory(history_dir: PathBuf) -> Result<Self> {
        // ディレクトリが存在しない場合は作成
        if !history_dir.exists() {
            std::fs::create_dir_all(&history_dir)
                .context("Failed to create history directory")?;
        }

        Ok(Self { history_dir })
    }

    /// 会話を保存
    ///
    /// # Arguments
    /// * `name` - 保存名（ファイル名として使用）
    /// * `conversation` - 保存する会話
    pub fn save(&self, name: &str, conversation: &Conversation) -> Result<PathBuf> {
        let sanitized_name = Self::sanitize_filename(name);
        let file_path = self.history_dir.join(format!("{}.json", sanitized_name));

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let persisted = PersistedConversation {
            name: name.to_string(),
            saved_at: now,
            messages: conversation.messages().iter().map(Self::message_to_persisted).collect(),
            metadata: ConversationMetadata::default(),
        };

        let json = serde_json::to_string_pretty(&persisted)
            .context("Failed to serialize conversation")?;

        std::fs::write(&file_path, json)
            .context("Failed to write history file")?;

        Ok(file_path)
    }

    /// 会話を読み込み
    ///
    /// # Arguments
    /// * `name` - 読み込む会話名
    pub fn load(&self, name: &str) -> Result<Conversation> {
        let sanitized_name = Self::sanitize_filename(name);
        let file_path = self.history_dir.join(format!("{}.json", sanitized_name));

        if !file_path.exists() {
            anyhow::bail!("History '{}' not found", name);
        }

        let json = std::fs::read_to_string(&file_path)
            .context("Failed to read history file")?;

        let persisted: PersistedConversation = serde_json::from_str(&json)
            .context("Failed to parse history file")?;

        let mut conversation = Conversation::new();
        for msg in persisted.messages {
            conversation.add(Self::persisted_to_message(&msg));
        }

        Ok(conversation)
    }

    /// 保存された会話一覧を取得
    pub fn list(&self) -> Result<Vec<HistoryEntry>> {
        let mut entries = Vec::new();

        if !self.history_dir.exists() {
            return Ok(entries);
        }

        let read_dir = std::fs::read_dir(&self.history_dir)
            .context("Failed to read history directory")?;

        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            match self.read_entry(&path) {
                Ok(history_entry) => entries.push(history_entry),
                Err(e) => {
                    tracing::warn!("Failed to read history entry {:?}: {}", path, e);
                }
            }
        }

        // 保存日時で降順ソート（新しいものが先頭）
        entries.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));

        Ok(entries)
    }

    /// 会話を削除
    ///
    /// # Arguments
    /// * `name` - 削除する会話名
    pub fn delete(&self, name: &str) -> Result<()> {
        let sanitized_name = Self::sanitize_filename(name);
        let file_path = self.history_dir.join(format!("{}.json", sanitized_name));

        if !file_path.exists() {
            anyhow::bail!("History '{}' not found", name);
        }

        std::fs::remove_file(&file_path)
            .context("Failed to delete history file")?;

        Ok(())
    }

    /// 会話が存在するかチェック
    pub fn exists(&self, name: &str) -> bool {
        let sanitized_name = Self::sanitize_filename(name);
        let file_path = self.history_dir.join(format!("{}.json", sanitized_name));
        file_path.exists()
    }

    /// 履歴ディレクトリのパスを取得
    pub fn history_dir(&self) -> &PathBuf {
        &self.history_dir
    }

    // --- Private methods ---

    /// ファイル名として安全な文字列に変換
    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect()
    }

    /// MessageをPersistedMessageに変換
    fn message_to_persisted(msg: &Message) -> PersistedMessage {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };

        let timestamp = msg.timestamp
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        PersistedMessage {
            role: role.to_string(),
            content: msg.content.clone(),
            tool_name: msg.tool_name.clone(),
            timestamp,
        }
    }

    /// PersistedMessageをMessageに変換
    fn persisted_to_message(persisted: &PersistedMessage) -> Message {
        let role = match persisted.role.as_str() {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            _ => Role::User,
        };

        let timestamp = persisted.timestamp.map(|ts| {
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(ts)
        });

        Message {
            role,
            content: persisted.content.clone(),
            tool_name: persisted.tool_name.clone(),
            timestamp,
        }
    }

    /// ファイルからHistoryEntryを読み込み
    fn read_entry(&self, path: &PathBuf) -> Result<HistoryEntry> {
        let json = std::fs::read_to_string(path)
            .context("Failed to read history file")?;

        let persisted: PersistedConversation = serde_json::from_str(&json)
            .context("Failed to parse history file")?;

        Ok(HistoryEntry {
            name: persisted.name,
            saved_at: persisted.saved_at,
            message_count: persisted.messages.len(),
            path: path.clone(),
        })
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new().expect("Failed to create HistoryManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let manager = HistoryManager::with_directory(temp_dir.path().to_path_buf()).unwrap();

        let mut conversation = Conversation::new();
        conversation.set_system("You are a helpful assistant.");
        conversation.add_user("Hello");
        conversation.add_assistant("Hi there!");

        // Save
        let path = manager.save("test-conversation", &conversation).unwrap();
        assert!(path.exists());

        // Load
        let loaded = manager.load("test-conversation").unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.messages()[0].role, Role::System);
        assert_eq!(loaded.messages()[1].role, Role::User);
        assert_eq!(loaded.messages()[2].role, Role::Assistant);
    }

    #[test]
    fn test_list() {
        let temp_dir = tempdir().unwrap();
        let manager = HistoryManager::with_directory(temp_dir.path().to_path_buf()).unwrap();

        let mut conv1 = Conversation::new();
        conv1.add_user("Hello");
        manager.save("conv1", &conv1).unwrap();

        let mut conv2 = Conversation::new();
        conv2.add_user("Hi");
        conv2.add_assistant("Hello!");
        manager.save("conv2", &conv2).unwrap();

        let entries = manager.list().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_delete() {
        let temp_dir = tempdir().unwrap();
        let manager = HistoryManager::with_directory(temp_dir.path().to_path_buf()).unwrap();

        let mut conversation = Conversation::new();
        conversation.add_user("Hello");
        manager.save("to-delete", &conversation).unwrap();

        assert!(manager.exists("to-delete"));
        manager.delete("to-delete").unwrap();
        assert!(!manager.exists("to-delete"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(HistoryManager::sanitize_filename("normal"), "normal");
        assert_eq!(HistoryManager::sanitize_filename("with/slash"), "with_slash");
        assert_eq!(HistoryManager::sanitize_filename("with:colon"), "with_colon");
        assert_eq!(HistoryManager::sanitize_filename("multi<>chars"), "multi__chars");
    }

    #[test]
    fn test_load_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let manager = HistoryManager::with_directory(temp_dir.path().to_path_buf()).unwrap();

        let result = manager.load("nonexistent");
        assert!(result.is_err());
    }
}
