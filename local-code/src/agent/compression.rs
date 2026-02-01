//! コンテキスト圧縮機能
//!
//! 会話履歴が長くなった際に、古いメッセージを要約して
//! トークン数を削減しつつ重要なコンテキストを保持する。

use super::conversation::{Conversation, Message, Role};
use serde::{Deserialize, Serialize};

/// 圧縮設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// 圧縮を開始するトークン使用率の閾値 (0.0-1.0)
    pub threshold: f32,
    /// 最大トークン数
    pub max_tokens: usize,
    /// 保持する最新メッセージ数
    pub preserve_recent: usize,
    /// コードブロックを保持するか
    pub preserve_code_blocks: bool,
    /// ツール結果を保持するか
    pub preserve_tool_results: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            threshold: 0.5,      // 50%で圧縮開始
            max_tokens: 128000,  // 128K tokens
            preserve_recent: 10, // 直近10メッセージは保持
            preserve_code_blocks: true,
            preserve_tool_results: true,
        }
    }
}

/// 圧縮されたメッセージ情報
#[derive(Debug, Clone)]
pub struct CompressedMessage {
    /// 圧縮されたメッセージ数
    pub original_count: usize,
    /// 要約テキスト
    pub summary: String,
}

/// 圧縮された会話
#[derive(Debug, Clone)]
pub struct CompressedConversation {
    /// システムメッセージ
    pub system_message: Option<Message>,
    /// 圧縮された過去の会話要約
    pub compressed_history: Option<CompressedMessage>,
    /// 保持されたメッセージ
    pub preserved_messages: Vec<Message>,
    /// 圧縮前の総メッセージ数
    pub original_message_count: usize,
    /// 推定トークン削減数
    pub estimated_tokens_saved: usize,
}

impl CompressedConversation {
    /// 圧縮された会話をConversationに変換
    pub fn to_conversation(&self) -> Conversation {
        let mut conv = Conversation::new();

        // システムメッセージを設定
        if let Some(ref system) = self.system_message {
            conv.set_system(&system.content);
        }

        // 圧縮された履歴を追加
        if let Some(ref compressed) = self.compressed_history {
            let summary_msg = format!(
                "[Previous conversation summary ({} messages)]\n{}",
                compressed.original_count, compressed.summary
            );
            conv.add(Message::system(summary_msg));
        }

        // 保持されたメッセージを追加
        for msg in &self.preserved_messages {
            conv.add(msg.clone());
        }

        conv
    }
}

/// コンテキスト圧縮器
#[derive(Debug, Clone)]
pub struct ContextCompressor {
    config: CompressionConfig,
}

impl ContextCompressor {
    /// 新しいコンテキスト圧縮器を作成
    pub fn new() -> Self {
        Self {
            config: CompressionConfig::default(),
        }
    }

    /// 設定を指定してコンテキスト圧縮器を作成
    pub fn with_config(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// 閾値を設定
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.config.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// 最大トークン数を設定
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.config.max_tokens = max_tokens;
        self
    }

    /// 圧縮が必要かどうかを判定
    pub fn should_compress(&self, conversation: &Conversation) -> bool {
        let estimated_tokens = self.estimate_tokens(conversation);
        let threshold_tokens = (self.config.max_tokens as f32 * self.config.threshold) as usize;

        estimated_tokens > threshold_tokens
    }

    /// 会話を圧縮
    pub fn compress(&self, conversation: &Conversation) -> CompressedConversation {
        let messages = conversation.messages();
        let original_count = messages.len();

        // システムメッセージを抽出
        let system_message = messages.iter().find(|m| m.role == Role::System).cloned();

        // 非システムメッセージを取得
        let non_system: Vec<_> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .cloned()
            .collect();

        // 圧縮が不要な場合
        if non_system.len() <= self.config.preserve_recent {
            return CompressedConversation {
                system_message,
                compressed_history: None,
                preserved_messages: non_system,
                original_message_count: original_count,
                estimated_tokens_saved: 0,
            };
        }

        // 古いメッセージと保持するメッセージを分離
        let split_point = non_system.len() - self.config.preserve_recent;
        let old_messages: Vec<_> = non_system[..split_point].to_vec();
        let recent_messages: Vec<_> = non_system[split_point..].to_vec();

        // 重要なメッセージを抽出
        let important_from_old = self.extract_important_messages(&old_messages);

        // 古いメッセージを要約
        let summary = self.summarize_messages(&old_messages, &important_from_old);

        // 推定トークン削減数を計算
        let old_tokens: usize = old_messages
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();
        let summary_tokens = self.estimate_text_tokens(&summary);
        let tokens_saved = old_tokens.saturating_sub(summary_tokens);

        CompressedConversation {
            system_message,
            compressed_history: Some(CompressedMessage {
                original_count: old_messages.len(),
                summary,
            }),
            preserved_messages: recent_messages,
            original_message_count: original_count,
            estimated_tokens_saved: tokens_saved,
        }
    }

    /// 会話のトークン数を推定
    pub fn estimate_tokens(&self, conversation: &Conversation) -> usize {
        conversation
            .messages()
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum()
    }

    /// メッセージのトークン数を推定
    fn estimate_message_tokens(&self, message: &Message) -> usize {
        // 簡易推定: 4文字 = 1トークン（日本語は2文字 = 1トークン）
        self.estimate_text_tokens(&message.content) + 4 // role分のオーバーヘッド
    }

    /// テキストのトークン数を推定
    fn estimate_text_tokens(&self, text: &str) -> usize {
        let ascii_count = text.chars().filter(|c| c.is_ascii()).count();
        let non_ascii_count = text.chars().filter(|c| !c.is_ascii()).count();

        // ASCIIは4文字=1トークン、非ASCIIは2文字=1トークン
        (ascii_count / 4) + (non_ascii_count / 2) + 1
    }

    /// 重要なメッセージを抽出
    fn extract_important_messages(&self, messages: &[Message]) -> Vec<Message> {
        let mut important = Vec::new();

        for msg in messages {
            let is_important =
                // ツール結果は保持
                (self.config.preserve_tool_results && msg.role == Role::Tool) ||
                // コードブロックを含むメッセージは保持
                (self.config.preserve_code_blocks && self.contains_code_block(&msg.content));

            if is_important {
                important.push(msg.clone());
            }
        }

        important
    }

    /// コードブロックを含むかチェック
    fn contains_code_block(&self, text: &str) -> bool {
        text.contains("```") || text.contains("    ") // インデントされたコードも検出
    }

    /// メッセージを要約
    fn summarize_messages(&self, messages: &[Message], important: &[Message]) -> String {
        let mut summary = String::new();

        // 会話の流れを要約
        let mut user_topics = Vec::new();
        let mut assistant_actions = Vec::new();

        for msg in messages {
            match msg.role {
                Role::User => {
                    // ユーザーの質問/リクエストを抽出
                    if let Some(topic) = self.extract_topic(&msg.content) {
                        user_topics.push(topic);
                    }
                }
                Role::Assistant => {
                    // アシスタントのアクションを抽出
                    if let Some(action) = self.extract_action(&msg.content) {
                        assistant_actions.push(action);
                    }
                }
                _ => {}
            }
        }

        // 要約を構築
        if !user_topics.is_empty() {
            summary.push_str("User discussed: ");
            summary.push_str(&user_topics.join(", "));
            summary.push_str(".\n");
        }

        if !assistant_actions.is_empty() {
            summary.push_str("Assistant: ");
            summary.push_str(&assistant_actions.join("; "));
            summary.push_str(".\n");
        }

        // 重要なコンテンツを追加
        for msg in important {
            match msg.role {
                Role::Tool => {
                    if let Some(ref name) = msg.tool_name {
                        summary.push_str(&format!("\n[Tool: {}] ", name));
                        // ツール結果は短縮して保持
                        let truncated = self.truncate_content(&msg.content, 200);
                        summary.push_str(&truncated);
                        summary.push('\n');
                    }
                }
                Role::Assistant | Role::User if self.contains_code_block(&msg.content) => {
                    // コードブロックを抽出して保持
                    if let Some(code) = self.extract_code_blocks(&msg.content) {
                        summary.push_str("\n[Code context]:\n");
                        summary.push_str(&code);
                        summary.push('\n');
                    }
                }
                _ => {}
            }
        }

        summary
    }

    /// トピックを抽出
    fn extract_topic(&self, content: &str) -> Option<String> {
        // 最初の1文または100文字を抽出
        let first_line = content.lines().next()?;
        let truncated = if first_line.len() > 100 {
            format!("{}...", &first_line[..97])
        } else {
            first_line.to_string()
        };
        Some(truncated)
    }

    /// アクションを抽出
    fn extract_action(&self, content: &str) -> Option<String> {
        // 最初の1文を抽出
        let first_sentence = content.split('.').next()?;
        if first_sentence.len() > 100 {
            Some(format!("{}...", &first_sentence[..97]))
        } else {
            Some(first_sentence.to_string())
        }
    }

    /// コンテンツを短縮
    fn truncate_content(&self, content: &str, max_len: usize) -> String {
        if content.len() <= max_len {
            content.to_string()
        } else {
            format!("{}...", &content[..max_len.saturating_sub(3)])
        }
    }

    /// コードブロックを抽出
    fn extract_code_blocks(&self, content: &str) -> Option<String> {
        let mut blocks = Vec::new();
        let mut in_block = false;
        let mut current_block = String::new();

        for line in content.lines() {
            if line.starts_with("```") {
                if in_block {
                    // ブロック終了
                    blocks.push(current_block.clone());
                    current_block.clear();
                }
                in_block = !in_block;
            } else if in_block {
                current_block.push_str(line);
                current_block.push('\n');
            }
        }

        if blocks.is_empty() {
            None
        } else {
            Some(blocks.join("\n---\n"))
        }
    }
}

impl Default for ContextCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_compress() {
        let compressor = ContextCompressor::new()
            .with_threshold(0.5)
            .with_max_tokens(100);

        let mut conv = Conversation::new();

        // 少ないメッセージでは圧縮不要
        conv.add_user("Hello");
        assert!(!compressor.should_compress(&conv));

        // 多くのメッセージを追加
        for i in 0..50 {
            conv.add_user(&format!(
                "Message {}: This is a longer message to increase token count",
                i
            ));
            conv.add_assistant(&format!(
                "Response {}: This is a longer response to increase token count",
                i
            ));
        }

        // 圧縮が必要になる
        assert!(compressor.should_compress(&conv));
    }

    #[test]
    fn test_compress_preserves_system() {
        let compressor = ContextCompressor::new();

        let mut conv = Conversation::new();
        conv.set_system("You are a helpful assistant.");
        conv.add_user("Hello");
        conv.add_assistant("Hi!");

        let compressed = compressor.compress(&conv);

        assert!(compressed.system_message.is_some());
        assert_eq!(
            compressed.system_message.unwrap().content,
            "You are a helpful assistant."
        );
    }

    #[test]
    fn test_compress_preserves_recent() {
        let config = CompressionConfig {
            preserve_recent: 2,
            ..Default::default()
        };
        let compressor = ContextCompressor::with_config(config);

        let mut conv = Conversation::new();
        for i in 0..10 {
            conv.add_user(&format!("User message {}", i));
            conv.add_assistant(&format!("Assistant message {}", i));
        }

        let compressed = compressor.compress(&conv);

        // 直近2メッセージが保持される
        assert_eq!(compressed.preserved_messages.len(), 2);
    }

    #[test]
    fn test_estimate_tokens() {
        let compressor = ContextCompressor::new();

        let mut conv = Conversation::new();
        conv.add_user("Hello world"); // ASCII: 11文字

        let tokens = compressor.estimate_tokens(&conv);
        assert!(tokens > 0);
    }

    #[test]
    fn test_compressed_to_conversation() {
        let compressor = ContextCompressor::new();

        let mut conv = Conversation::new();
        conv.set_system("System prompt");
        for i in 0..20 {
            conv.add_user(&format!("User {}", i));
            conv.add_assistant(&format!("Assistant {}", i));
        }

        let compressed = compressor.compress(&conv);
        let restored = compressed.to_conversation();

        // 復元された会話はシステムメッセージを含む
        assert!(restored.messages().iter().any(|m| m.role == Role::System));
    }
}
