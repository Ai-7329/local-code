use anyhow::Result;
use std::sync::Arc;

use crate::config::{OllamaConfig, RetryConfig};
use crate::llm::{OllamaClient, ToolCallParser};
use crate::tools::ToolRegistry;
use crate::skills::SkillRegistry;
use crate::cli::output::StreamingWriter;
use super::context::AgentContext;
use super::conversation::Conversation;
use super::mode::ModeManager;

/// エージェント設定
pub struct AgentConfig {
    pub ollama_url: String,
    pub model: String,
    pub initial_mode: super::mode::Mode,
    /// 会話履歴の最大メッセージ数
    pub max_messages: usize,
    /// 接続タイムアウト（秒）
    pub connect_timeout: u64,
    /// 読み取りタイムアウト（秒）
    pub read_timeout: u64,
    /// リトライ設定
    pub retry_config: RetryConfig,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            ollama_url: "http://localhost:11434".to_string(),
            model: "Rnj-1".to_string(),
            initial_mode: super::mode::Mode::Execute,
            max_messages: 100,
            connect_timeout: 30,
            read_timeout: 300,
            retry_config: RetryConfig::default(),
        }
    }
}

impl AgentConfig {
    /// OllamaConfigからAgentConfigを作成
    pub fn from_ollama_config(
        ollama_config: &OllamaConfig,
        initial_mode: super::mode::Mode,
        max_messages: usize,
    ) -> Self {
        Self {
            ollama_url: ollama_config.url.clone(),
            model: ollama_config.model.clone(),
            initial_mode,
            max_messages,
            connect_timeout: ollama_config.connect_timeout,
            read_timeout: ollama_config.read_timeout,
            retry_config: ollama_config.retry.clone(),
        }
    }
}

/// メインエージェント
pub struct Agent {
    /// LLMクライアント
    llm: OllamaClient,
    /// ツールレジストリ
    tools: Arc<ToolRegistry>,
    /// スキルレジストリ
    skills: Arc<SkillRegistry>,
    /// 会話履歴
    conversation: Conversation,
    /// モードマネージャー
    mode: ModeManager,
    /// プロジェクトコンテキスト
    context: AgentContext,
    /// システムプロンプト追加分
    system_extra: Option<String>,
    /// 会話履歴の最大メッセージ数
    max_messages: usize,
    /// 作業ディレクトリ（プロジェクトルート）
    project_root: Option<std::path::PathBuf>,
}

impl Agent {
    /// 新しいエージェントを作成
    pub fn new(
        config: AgentConfig,
        tools: ToolRegistry,
        skills: Arc<SkillRegistry>,
        mode: ModeManager,
    ) -> Self {
        Self {
            llm: OllamaClient::with_timeout(
                &config.ollama_url,
                &config.model,
                config.connect_timeout,
                config.read_timeout,
            )
            .with_retry_config(config.retry_config.clone()),
            tools: Arc::new(tools),
            skills,
            conversation: Conversation::with_max_messages(config.max_messages),
            mode,
            context: AgentContext::default(),
            system_extra: None,
            max_messages: config.max_messages,
            project_root: None,
        }
    }

    /// プロジェクトコンテキストを読み込み
    pub async fn load_context(&mut self, project_root: &std::path::Path) -> Result<()> {
        // 作業ディレクトリを保存
        self.project_root = Some(project_root.to_path_buf());

        self.context = AgentContext::load_from_project(project_root).await?;

        // システムプロンプトを設定
        let mut system_prompt = self.build_system_prompt();
        if let Some(extra) = &self.system_extra {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(extra);
        }
        if let Some(ctx) = self.context.as_system_prompt() {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(&ctx);
        }
        // デバッグ: システムプロンプトをログ出力
        tracing::debug!("System prompt set with working directory: {:?}", self.project_root);
        eprintln!("[DEBUG] Working directory in system prompt: {:?}", self.project_root);
        self.conversation.set_system(system_prompt);

        Ok(())
    }

    /// ユーザー入力を処理
    pub async fn process(&mut self, input: &str) -> Result<String> {
        self.conversation.add_user(input);

        // LLMに送信
        let prompt = self.conversation.to_prompt();
        let response = self.llm.generate(&prompt, None).await?;

        // ツール呼び出しをパース
        let tool_calls = ToolCallParser::parse(&response)?;

        if tool_calls.is_empty() {
            // ツール呼び出しなし - テキスト応答
            self.conversation.add_assistant(&response);
            return Ok(response);
        }

        // ツールを実行
        let mut full_response = String::new();
        let (text_part, _) = ToolCallParser::split_response(&response);
        if !text_part.is_empty() {
            full_response.push_str(&text_part);
            full_response.push_str("\n\n");
        }

        for call in tool_calls {
            // モード制限をチェック
            if !self.mode.is_tool_allowed(&call.tool).await {
                let error_msg = format!(
                    "Tool '{}' is not allowed in {} mode",
                    call.tool,
                    self.mode.current().await
                );
                self.conversation.add_tool_result(&call.tool, &error_msg);
                full_response.push_str(&format!("[{}] {}\n", call.tool, error_msg));
                continue;
            }

            // ツールを実行
            if let Some(tool) = self.tools.get(&call.tool) {
                match tool.execute(call.params).await {
                    Ok(result) => {
                        let output = if result.success {
                            result.output
                        } else {
                            result.error.unwrap_or_else(|| "Unknown error".to_string())
                        };
                        self.conversation.add_tool_result(&call.tool, &output);
                        full_response.push_str(&format!("[{}]\n{}\n\n", call.tool, output));
                    }
                    Err(e) => {
                        let error = format!("Error: {}", e);
                        self.conversation.add_tool_result(&call.tool, &error);
                        full_response.push_str(&format!("[{}] {}\n\n", call.tool, error));
                    }
                }
            } else {
                let error = format!("Unknown tool: {}", call.tool);
                self.conversation.add_tool_result(&call.tool, &error);
                full_response.push_str(&format!("{}\n\n", error));
            }
        }

        self.conversation.add_assistant(&full_response);
        Ok(full_response)
    }

    /// システムプロンプトを構築
    fn build_system_prompt(&self) -> String {
        let tools_prompt = self.tools.to_prompt_format();

        // 作業ディレクトリ情報を追加
        let working_dir_info = if let Some(ref root) = self.project_root {
            format!(
                "\n\n# Working Directory\nYou are working in: {}\nAll file operations (read, write, glob, grep, bash) are relative to this directory.\nWhen using tools, you can use relative paths from this directory, or omit the path parameter to use the current directory.",
                root.display()
            )
        } else {
            String::new()
        };

        format!(
            r#"You are a coding assistant. You can use tools to help the user.

To use a tool, output a JSON block like this:
```json
{{"tool": "tool_name", "params": {{"param1": "value1"}}}}
```

{}{}"#,
            tools_prompt,
            working_dir_info
        )
    }

    /// モードマネージャーへの参照を取得
    pub fn mode(&self) -> &ModeManager {
        &self.mode
    }

    /// スキルレジストリへの参照を取得
    pub fn skills(&self) -> &SkillRegistry {
        &self.skills
    }

    /// 会話をクリア
    pub fn clear_conversation(&mut self) {
        self.conversation.clear();
    }

    /// 会話履歴を取得
    pub fn conversation(&self) -> &Conversation {
        &self.conversation
    }

    /// 会話履歴を置き換え
    pub fn replace_conversation(&mut self, mut conversation: Conversation) {
        conversation.set_max_messages(self.max_messages);
        self.conversation = conversation;
    }

    /// 会話履歴の最大メッセージ数を更新
    pub fn set_max_messages(&mut self, max_messages: usize) {
        self.max_messages = max_messages;
        self.conversation.set_max_messages(max_messages);
    }

    /// システムプロンプトの追加分を設定
    pub fn set_system_extra(&mut self, extra: Option<String>) {
        self.system_extra = extra;
    }

    /// モデルを切り替え
    pub fn set_model(&mut self, model: impl Into<String>) {
        self.llm.set_model(model);
    }

    /// ストリーミングでユーザー入力を処理
    ///
    /// トークンを受信するたびにリアルタイムで出力する
    pub async fn process_streaming(&mut self, input: &str) -> Result<String> {
        self.conversation.add_user(input);

        // LLMにストリーミングリクエストを送信
        let prompt = self.conversation.to_prompt();
        let mut stream = self.llm.generate_streaming(&prompt, None).await?;

        // ストリーミングライターを初期化
        let mut writer = StreamingWriter::new();
        writer.start(None);

        // ストリーミングで受信
        let mut last_stats: Option<crate::llm::StreamStats> = None;

        while let Some(chunk) = stream.next().await {
            // テキストを即座に出力
            writer.write(&chunk.text);

            // 統計情報を保存
            if chunk.done {
                last_stats = chunk.stats;
            }
        }

        // 統計情報付きで終了（利用可能な場合）
        if let Some(stats) = last_stats {
            writer.finish_with_stats(stats.tokens_per_second, stats.eval_count);
        } else {
            writer.finish();
        }

        // 累積されたテキストを取得
        let response = stream.accumulated().to_string();

        // ツール呼び出しをパース
        let tool_calls = ToolCallParser::parse(&response)?;

        if tool_calls.is_empty() {
            // ツール呼び出しなし - テキスト応答
            self.conversation.add_assistant(&response);
            return Ok(response);
        }

        // ツールを実行
        let mut full_response = String::new();
        let (text_part, _) = ToolCallParser::split_response(&response);
        if !text_part.is_empty() {
            full_response.push_str(&text_part);
            full_response.push_str("\n\n");
        }

        for call in tool_calls {
            // モード制限をチェック
            if !self.mode.is_tool_allowed(&call.tool).await {
                let error_msg = format!(
                    "Tool '{}' is not allowed in {} mode",
                    call.tool,
                    self.mode.current().await
                );
                self.conversation.add_tool_result(&call.tool, &error_msg);
                full_response.push_str(&format!("[{}] {}\n", call.tool, error_msg));
                continue;
            }

            // ツールを実行
            println!(); // ツール実行前に改行
            crate::cli::output::print_tool(&call.tool, "executing...");

            if let Some(tool) = self.tools.get(&call.tool) {
                match tool.execute(call.params).await {
                    Ok(result) => {
                        let output = if result.success {
                            result.output
                        } else {
                            result.error.unwrap_or_else(|| "Unknown error".to_string())
                        };
                        self.conversation.add_tool_result(&call.tool, &output);
                        full_response.push_str(&format!("[{}]\n{}\n\n", call.tool, output));
                        // ツール結果を表示
                        crate::cli::output::print_success(&format!("[{}] completed", call.tool));
                    }
                    Err(e) => {
                        let error = format!("Error: {}", e);
                        self.conversation.add_tool_result(&call.tool, &error);
                        full_response.push_str(&format!("[{}] {}\n\n", call.tool, error));
                        crate::cli::output::print_error(&format!("[{}] {}", call.tool, error));
                    }
                }
            } else {
                let error = format!("Unknown tool: {}", call.tool);
                self.conversation.add_tool_result(&call.tool, &error);
                full_response.push_str(&format!("{}\n\n", error));
                crate::cli::output::print_error(&error);
            }
        }

        self.conversation.add_assistant(&full_response);
        Ok(full_response)
    }

    /// ストリーミングでユーザー入力を処理（コールバック版）
    ///
    /// 各トークン受信時にコールバック関数が呼ばれる
    pub async fn process_streaming_with_callback<F>(
        &mut self,
        input: &str,
        mut on_token: F,
    ) -> Result<String>
    where
        F: FnMut(&str),
    {
        self.conversation.add_user(input);

        // LLMにストリーミングリクエストを送信
        let prompt = self.conversation.to_prompt();
        let mut stream = self.llm.generate_streaming(&prompt, None).await?;

        // コールバック付きで処理
        while let Some(chunk) = stream.next().await {
            on_token(&chunk.text);
        }

        // 累積されたテキストを取得
        let response = stream.accumulated().to_string();

        // ツール呼び出しをパース（ストリーミング後に処理）
        let tool_calls = ToolCallParser::parse(&response)?;

        if tool_calls.is_empty() {
            self.conversation.add_assistant(&response);
            return Ok(response);
        }

        // ツールを実行（非ストリーミング部分）
        let mut full_response = response.clone();

        for call in tool_calls {
            if !self.mode.is_tool_allowed(&call.tool).await {
                let error_msg = format!(
                    "Tool '{}' is not allowed in {} mode",
                    call.tool,
                    self.mode.current().await
                );
                self.conversation.add_tool_result(&call.tool, &error_msg);
                full_response.push_str(&format!("\n[{}] {}", call.tool, error_msg));
                continue;
            }

            if let Some(tool) = self.tools.get(&call.tool) {
                match tool.execute(call.params).await {
                    Ok(result) => {
                        let output = if result.success {
                            result.output
                        } else {
                            result.error.unwrap_or_else(|| "Unknown error".to_string())
                        };
                        self.conversation.add_tool_result(&call.tool, &output);
                        full_response.push_str(&format!("\n[{}]\n{}", call.tool, output));
                    }
                    Err(e) => {
                        let error = format!("Error: {}", e);
                        self.conversation.add_tool_result(&call.tool, &error);
                        full_response.push_str(&format!("\n[{}] {}", call.tool, error));
                    }
                }
            } else {
                let error = format!("Unknown tool: {}", call.tool);
                self.conversation.add_tool_result(&call.tool, &error);
                full_response.push_str(&format!("\n{}", error));
            }
        }

        self.conversation.add_assistant(&full_response);
        Ok(full_response)
    }

    /// LLMクライアントへの参照を取得
    pub fn llm(&self) -> &OllamaClient {
        &self.llm
    }
}
