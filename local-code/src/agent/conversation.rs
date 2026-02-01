use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// 会話のロール
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// 会話メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip)]
    pub timestamp: Option<SystemTime>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_name: None,
            timestamp: Some(SystemTime::now()),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_name: None,
            timestamp: Some(SystemTime::now()),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_name: None,
            timestamp: Some(SystemTime::now()),
        }
    }

    pub fn tool(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_name: Some(name.into()),
            timestamp: Some(SystemTime::now()),
        }
    }
}

/// 会話履歴
#[derive(Debug, Clone, Default)]
pub struct Conversation {
    messages: Vec<Message>,
    max_messages: usize,
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_messages: 100,
        }
    }

    pub fn with_max_messages(max: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages: max,
        }
    }

    /// 最大メッセージ数を設定
    pub fn set_max_messages(&mut self, max: usize) {
        self.max_messages = max;
        self.truncate_if_needed();
    }

    /// システムメッセージを設定（会話の最初に配置）
    pub fn set_system(&mut self, content: impl Into<String>) {
        // 既存のシステムメッセージを削除
        self.messages.retain(|m| m.role != Role::System);
        // 先頭に追加
        self.messages.insert(0, Message::system(content));
    }

    /// メッセージを追加
    pub fn add(&mut self, message: Message) {
        self.messages.push(message);
        self.truncate_if_needed();
    }

    /// ユーザーメッセージを追加
    pub fn add_user(&mut self, content: impl Into<String>) {
        self.add(Message::user(content));
    }

    /// アシスタントメッセージを追加
    pub fn add_assistant(&mut self, content: impl Into<String>) {
        self.add(Message::assistant(content));
    }

    /// ツール結果を追加
    pub fn add_tool_result(&mut self, tool_name: impl Into<String>, content: impl Into<String>) {
        self.add(Message::tool(tool_name, content));
    }

    /// 全メッセージを取得
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// 最後のメッセージを取得
    pub fn last(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// メッセージ数を取得
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// 空かチェック
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// 会話をクリア（システムメッセージは保持）
    pub fn clear(&mut self) {
        let system_msg = self.messages.iter()
            .find(|m| m.role == Role::System)
            .cloned();
        self.messages.clear();
        if let Some(msg) = system_msg {
            self.messages.push(msg);
        }
    }

    /// プロンプト形式に変換（OLLAMA用）
    pub fn to_prompt(&self) -> String {
        let mut prompt = String::new();

        for msg in &self.messages {
            match msg.role {
                Role::System => {
                    prompt.push_str(&format!("System: {}\n\n", msg.content));
                }
                Role::User => {
                    prompt.push_str(&format!("User: {}\n\n", msg.content));
                }
                Role::Assistant => {
                    prompt.push_str(&format!("Assistant: {}\n\n", msg.content));
                }
                Role::Tool => {
                    let tool_name = msg.tool_name.as_deref().unwrap_or("unknown");
                    prompt.push_str(&format!("Tool ({}): {}\n\n", tool_name, msg.content));
                }
            }
        }

        prompt.push_str("Assistant: ");
        prompt
    }

    /// 必要に応じて古いメッセージを削除
    fn truncate_if_needed(&mut self) {
        if self.messages.len() > self.max_messages {
            // システムメッセージは保持
            let system_msgs: Vec<_> = self.messages.iter()
                .filter(|m| m.role == Role::System)
                .cloned()
                .collect();

            let non_system: Vec<_> = self.messages.iter()
                .filter(|m| m.role != Role::System)
                .cloned()
                .collect();

            let keep_count = self.max_messages - system_msgs.len();
            let skip = non_system.len().saturating_sub(keep_count);

            self.messages = system_msgs;
            self.messages.extend(non_system.into_iter().skip(skip));
        }
    }

    /// コンテキスト圧縮を適用して新しいConversationを返す
    pub fn compress(&self) -> Self {
        use super::compression::ContextCompressor;

        let compressor = ContextCompressor::new();
        if compressor.should_compress(self) {
            compressor.compress(self).to_conversation()
        } else {
            self.clone()
        }
    }

    /// カスタム設定でコンテキスト圧縮を適用
    pub fn compress_with_config(&self, config: super::compression::CompressionConfig) -> Self {
        use super::compression::ContextCompressor;

        let compressor = ContextCompressor::with_config(config);
        if compressor.should_compress(self) {
            compressor.compress(self).to_conversation()
        } else {
            self.clone()
        }
    }

    /// 圧縮が必要かどうかをチェック
    pub fn needs_compression(&self, threshold: f32, max_tokens: usize) -> bool {
        use super::compression::ContextCompressor;

        let compressor = ContextCompressor::new()
            .with_threshold(threshold)
            .with_max_tokens(max_tokens);
        compressor.should_compress(self)
    }

    /// 推定トークン数を取得
    pub fn estimated_tokens(&self) -> usize {
        use super::compression::ContextCompressor;

        let compressor = ContextCompressor::new();
        compressor.estimate_tokens(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation() {
        let mut conv = Conversation::new();
        conv.set_system("You are a helpful assistant.");
        conv.add_user("Hello");
        conv.add_assistant("Hi there!");

        assert_eq!(conv.len(), 3);
        assert_eq!(conv.messages()[0].role, Role::System);
        assert_eq!(conv.messages()[1].role, Role::User);
    }

    #[test]
    fn test_to_prompt() {
        let mut conv = Conversation::new();
        conv.add_user("Hello");
        let prompt = conv.to_prompt();
        assert!(prompt.contains("User: Hello"));
        assert!(prompt.ends_with("Assistant: "));
    }
}
