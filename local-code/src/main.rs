use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

use local_code::{
    config::Config,
    Mode, ModeManager,
    Command, CommandHandler, CommandResult, Repl,
    ToolRegistry,
    SkillRegistry, SkillExecutor,
    Agent, AgentConfig, CodeVerifier,
    tools::file::{ReadTool, WriteTool, EditTool},
    tools::search::{GlobTool, GrepTool},
    tools::bash::BashTool,
    tools::git::{GitStatusTool, GitDiffTool, GitAddTool, GitCommitTool, GitLogTool},
    tools::lsp::{LspClient, LspDefinitionTool, LspReferencesTool, LspDiagnosticsTool},
    skills::{SkillContext, TriggerDetector, load_superpowers_commands, EmbeddedSuperpowers},
    cli::{print_startup_banner, print_formatted_block, print_processing, print_separator, OutputPostProcessor},
};

#[derive(Parser, Debug)]
#[command(name = "local-code")]
#[command(about = "OLLAMA連携コーディングエージェント")]
#[command(version)]
struct Args {
    /// 設定ファイルパス
    #[arg(short, long, default_value = "config/default.toml")]
    config: PathBuf,

    /// OLLAMAサーバーURL
    #[arg(long)]
    ollama_url: Option<String>,

    /// 使用するモデル名
    #[arg(short, long)]
    model: Option<String>,

    /// 初期モード (plan/execute)
    #[arg(long)]
    mode: Option<String>,

    /// プロジェクトルートディレクトリ
    #[arg(short, long)]
    project: Option<PathBuf>,

    /// 詳細ログを表示 (INFO level)
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // トレーシング初期化（デフォルトはWARN、--verboseでINFO）
    let args = Args::parse();
    let default_level = if args.verbose { "info" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level))
        )
        .init();

    // 設定ファイルを読み込み
    let config = if args.config.exists() {
        Config::load_from_file(&args.config).unwrap_or_else(|e| {
            tracing::warn!("Failed to load config file: {}, using defaults", e);
            Config::default()
        })
    } else {
        Config::load_default().unwrap_or_else(|e| {
            tracing::warn!("Failed to load default config: {}, using defaults", e);
            Config::default()
        })
    };

    // コマンドライン引数で設定を上書き
    let ollama_url = args.ollama_url.clone().unwrap_or_else(|| config.ollama.url.clone());
    let model = args.model.clone().unwrap_or_else(|| config.ollama.model.clone());

    tracing::info!("local-code v{} starting...", local_code::VERSION);
    tracing::info!("OLLAMA URL: {}", ollama_url);
    tracing::info!("Model: {}", model);
    let mode_str = args.mode.clone().unwrap_or_else(|| config.agent.initial_mode.clone());
    tracing::info!("Mode: {}", mode_str);
    tracing::info!("Connect timeout: {}s", config.ollama.connect_timeout);
    tracing::info!("Read timeout: {}s", config.ollama.read_timeout);

    // 初期モードをパース
    let initial_mode = Mode::parse_mode(&mode_str).unwrap_or_else(|| {
        tracing::warn!("Invalid mode '{}', using execute", mode_str);
        Mode::Execute
    });

    // モードマネージャーを初期化
    let mode_manager = ModeManager::new(initial_mode);

    // ツールレジストリを初期化
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Arc::new(ReadTool::new()));
    tool_registry.register(Arc::new(WriteTool::new()));
    tool_registry.register(Arc::new(EditTool::new()));
    tool_registry.register(Arc::new(GlobTool::new()));
    tool_registry.register(Arc::new(GrepTool::new()));
    tool_registry.register(Arc::new(BashTool::with_timeout(config.tools.bash_timeout)));
    tool_registry.register(Arc::new(GitStatusTool::new()));
    tool_registry.register(Arc::new(GitDiffTool::new()));
    tool_registry.register(Arc::new(GitAddTool::new()));
    tool_registry.register(Arc::new(GitCommitTool::new()));
    tool_registry.register(Arc::new(GitLogTool::new()));
    // LSPツール（クライアントは後で初期化）
    let lsp_client = Arc::new(Mutex::new(None));
    tool_registry.register(Arc::new(LspDefinitionTool::new(Arc::clone(&lsp_client))));
    tool_registry.register(Arc::new(LspReferencesTool::new(Arc::clone(&lsp_client))));
    tool_registry.register(Arc::new(LspDiagnosticsTool::new(Arc::clone(&lsp_client))));

    tracing::info!("Registered {} tools", tool_registry.len());

    // スキルレジストリを初期化
    let mut skill_registry = SkillRegistry::new();
    if let Some(custom_path) = &config.skills.custom_path {
        skill_registry.add_search_path(PathBuf::from(custom_path));
    }

    // Superpowersスキルをロード
    let superpowers_dir = find_superpowers_dir();
    if let Some(dir) = &superpowers_dir {
        skill_registry.add_superpowers_path(dir.join("skills"));
    }

    skill_registry.load_all().await?;
    tracing::info!("Loaded {} skills", skill_registry.len());
    let skill_registry = Arc::new(skill_registry);

    // モードマネージャーを初期化
    // Superpowersコマンドエイリアスをロード（埋め込み + ファイルシステム）
    let mut command_aliases: HashMap<String, String> = HashMap::new();
    let mut superpowers_commands: Vec<String> = Vec::new();
    let commands_dir = superpowers_dir.as_ref().map(|d| d.join("commands")).unwrap_or_default();
    match load_superpowers_commands(&commands_dir).await {
        Ok(commands) => {
            for command in commands {
                command_aliases.insert(command.name.clone(), command.skill.clone());
                superpowers_commands.push(command.name);
            }
        }
        Err(e) => tracing::warn!("Failed to load superpowers commands: {}", e),
    }

    // コマンドハンドラーを初期化
    let command_handler = CommandHandler::new(mode_manager.clone())
        .with_skill_aliases(command_aliases);

    // エージェントを初期化（設定ファイルからタイムアウトを取得）
    let agent_config = AgentConfig {
        ollama_url: ollama_url.clone(),
        model: model.clone(),
        initial_mode,
        max_messages: config.agent.max_messages,
        connect_timeout: config.ollama.connect_timeout,
        read_timeout: config.ollama.read_timeout,
        retry_config: config.ollama.retry.clone(),
    };
    let mut agent = Agent::new(
        agent_config,
        tool_registry,
        Arc::clone(&skill_registry),
        mode_manager.clone(),
    );

    // Superpowersブートストラップをシステムプロンプトに追加
    // 優先順位: ファイルシステム > 埋め込み
    let bootstrap_content = if let Some(dir) = &superpowers_dir {
        let local_bootstrap = dir.join("superpowers-bootstrap.local.md");
        let codex_bootstrap = dir.join("superpowers-bootstrap.md");
        let bootstrap_path = if local_bootstrap.exists() {
            Some(local_bootstrap)
        } else if codex_bootstrap.exists() {
            Some(codex_bootstrap)
        } else {
            None
        };
        if let Some(path) = bootstrap_path {
            match fs::read_to_string(&path).await {
                Ok(content) => Some(content),
                Err(e) => {
                    tracing::warn!("Failed to read superpowers bootstrap: {}", e);
                    EmbeddedSuperpowers::bootstrap()
                }
            }
        } else {
            EmbeddedSuperpowers::bootstrap()
        }
    } else {
        // ファイルシステムにsuperpowersがない場合は埋め込み版を使用
        EmbeddedSuperpowers::bootstrap()
    };
    if let Some(content) = bootstrap_content {
        agent.set_system_extra(Some(content));
    }

    // プロジェクトコンテキストを読み込み
    let project_root = args.project
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    // LSPクライアントを初期化（設定またはCargoプロジェクトの場合のみ）
    let lsp_command = config
        .lsp
        .command
        .clone()
        .or_else(|| {
            if project_root.join("Cargo.toml").exists() {
                Some("rust-analyzer".to_string())
            } else {
                None
            }
        });
    if let Some(command) = lsp_command {
        let args = config.lsp.args.clone();
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        match LspClient::start(&command, &arg_refs).await {
            Ok(client) => {
                match client.initialize(&project_root).await {
                    Ok(_) => {
                        *lsp_client.lock().await = Some(client);
                        tracing::info!("LSP initialized: {}", command);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to initialize LSP: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to start LSP server '{}': {}", command, e);
            }
        }
    }

    if let Err(e) = agent.load_context(&project_root).await {
        tracing::warn!("Failed to load project context: {}", e);
    } else {
        tracing::info!("Loaded project context from: {}", project_root.display());
    }

    let mut repl = Repl::new();
    repl.set_skills(skill_registry.names());
    repl.set_superpowers_commands(superpowers_commands.clone());
    repl.set_working_dir(project_root.clone());
    repl.set_mode(mode_str.clone());
    repl.set_model(model.clone());

    // Claude Code風の起動バナーを表示
    print_startup_banner(
        local_code::VERSION,
        &model,
        &project_root.display().to_string(),
        &superpowers_commands,
    );

    println!("Type /help for commands, /quit to exit\n");

    loop {
        let mode = mode_manager.current().await;
        // モードとモデルを更新してプロンプトを自動生成
        repl.set_mode(mode.to_string());
        repl.set_model(agent.llm().model().to_string());
        // モードアイコン付きプロンプトを表示
        repl.print_prompt_with_icon(Some(mode.icon()))?;

        let input = repl.read_line_with_history()?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        let command = Command::parse(input);
        let result = command_handler.handle(&command, &*skill_registry).await;

        match result {
            CommandResult::Exit => {
                print_separator();
                println!("Goodbye!");
                break;
            }
            CommandResult::Clear => {
                // シンプルモードでは画面クリアは行わない（スクロール式のため）
                println!("\n--- cleared ---\n");
            }
            CommandResult::Output(msg) => {
                print_formatted_block("INFO", &msg);
            }
            CommandResult::SendToLLM(msg) => {
                print_formatted_block("USER", &msg);
                let detector = TriggerDetector::new(&skill_registry);
                let matches = detector.detect(&msg);

                // 自動実行スキルがあれば実行
                if let Some(skill) = matches.iter().find(|s| s.metadata.auto) {
                    print_formatted_block("SKILL", &format!("Auto: {}", skill.metadata.name));
                    let skill_executor = SkillExecutor::new(Arc::clone(&skill_registry));
                    let context = SkillContext::new(Some(msg.clone()));
                    match skill_executor.execute(skill, &context).await {
                        Ok(skill_prompt) => {
                            print_processing("Processing skill prompt...");
                            match agent.process(&skill_prompt).await {
                                Ok(response) => print_formatted_block("ASSISTANT", &response),
                                Err(e) => {
                                    tracing::error!("Agent error while processing skill: {}", e);
                                    print_formatted_block("ERROR", &format!("Failed to process skill: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Skill execution error: {}", e);
                            print_formatted_block(
                                "ERROR",
                                &format!("Failed to execute skill '{}': {}", skill.metadata.name, e),
                            );
                        }
                    }
                    continue;
                }

                // 関連スキルがあればモデルに通知（強制適用）
                let enhanced_msg = if !matches.is_empty() {
                    let skill_names: Vec<_> = matches.iter().map(|s| s.metadata.name.as_str()).collect();
                    print_formatted_block("SKILL", &format!("Related: {}", skill_names.join(", ")));
                    format!(
                        "<skill_hint>\nRelevant skills detected: {}. Consider using `/{}` if applicable.\n</skill_hint>\n\n{}",
                        skill_names.join(", "),
                        skill_names.first().unwrap_or(&""),
                        msg
                    )
                } else {
                    msg.clone()
                };

                // 「only code」キーワードを検出
                let code_only = {
                    let lower = msg.to_lowercase();
                    lower.contains("only code") || lower.contains("code only") || lower.contains("コードのみ")
                };

                // エージェントに処理を委譲
                print_processing("Processing...");
                match agent.process(&enhanced_msg).await {
                    Ok(response) => {
                        // ポストプロセス（THOUGHT除去、オプションでコードのみ抽出）
                        let mut processed = OutputPostProcessor::process(&response, code_only);

                        // 自己検証ループ
                        let verifier = CodeVerifier::new();
                        let code_blocks = CodeVerifier::extract_code_blocks(&processed);

                        for (lang, code) in &code_blocks {
                            if lang.is_empty() {
                                continue;
                            }

                            match verifier.verify(lang, code) {
                                Ok(result) => {
                                    if !result.success {
                                        print_formatted_block("VERIFY", &format!("❌ {} error detected, attempting fix...", lang));

                                        // 修正ループ
                                        let mut attempts = 0;
                                        let mut current_code = code.clone();
                                        let mut last_error = result.error.clone();

                                        while attempts < verifier.max_attempts() {
                                            let fix_prompt = verifier.create_fix_prompt(&local_code::VerificationResult {
                                                success: false,
                                                output: String::new(),
                                                error: last_error.clone(),
                                                language: lang.clone(),
                                                code: current_code.clone(),
                                            });

                                            print_processing(&format!("Fix attempt {}/{}...", attempts + 1, verifier.max_attempts()));

                                            match agent.process(&fix_prompt).await {
                                                Ok(fix_response) => {
                                                    let fixed = OutputPostProcessor::process(&fix_response, true);
                                                    let fixed_blocks = CodeVerifier::extract_code_blocks(&fixed);

                                                    // フェンスなし応答対応
                                                    let fixed_code = if let Some((_, code)) = fixed_blocks.first() {
                                                        code.clone()
                                                    } else {
                                                        // フェンスがない場合は応答全体をコードとして扱う
                                                        let trimmed = fixed.trim();
                                                        if !trimmed.is_empty() {
                                                            trimmed.to_string()
                                                        } else {
                                                            continue; // 空の応答はスキップ
                                                        }
                                                    };

                                                    if !fixed_code.is_empty() {
                                                        current_code = fixed_code;

                                                        match verifier.verify(lang, &current_code) {
                                                            Ok(verify_result) => {
                                                                if verify_result.success {
                                                                    print_formatted_block("VERIFY", &format!("✅ {} code fixed successfully!", lang));
                                                                    processed = replace_code_block(&processed, code, &current_code, lang);
                                                                    break;
                                                                } else {
                                                                    last_error = verify_result.error;
                                                                }
                                                            }
                                                            Err(e) => {
                                                                tracing::warn!("Verification error: {}", e);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!("Fix attempt failed: {}", e);
                                                    break;
                                                }
                                            }
                                            attempts += 1;
                                        }

                                        if attempts >= verifier.max_attempts() {
                                            print_formatted_block("VERIFY", &format!("⚠️ Could not fix {} code after {} attempts", lang, verifier.max_attempts()));
                                        }
                                    } else {
                                        print_formatted_block("VERIFY", &format!("✅ {} code verified", lang));
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!("Verification skipped: {}", e);
                                }
                            }
                        }

                        print_formatted_block("ASSISTANT", &processed);
                    }
                    Err(e) => {
                        tracing::error!("Agent error: {}", e);
                        print_formatted_block("ERROR", &format!("Failed to process request: {}", e));
                    }
                }
            }
            CommandResult::Skill { name, args } => {
                print_formatted_block("SKILL", &format!("Manual: {}", name));

                // SkillExecutorを使用してスキルを実行
                let skill_executor = SkillExecutor::new(Arc::clone(&skill_registry));
                let context = SkillContext::new(args);

                match skill_executor.execute_by_name(&name, &context).await {
                    Ok(skill_prompt) => {
                        // 生成されたプロンプトをLLMに送信
                        print_processing("Processing skill prompt...");
                        match agent.process(&skill_prompt).await {
                            Ok(response) => {
                                print_formatted_block("ASSISTANT", &response);
                            }
                            Err(e) => {
                                tracing::error!("Agent error while processing skill: {}", e);
                                print_formatted_block("ERROR", &format!("Failed to process skill: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Skill execution error: {}", e);
                        print_formatted_block("ERROR", &format!("Failed to execute skill '{}': {}", name, e));
                    }
                }
            }
            CommandResult::SaveConversation { name } => {
                match command_handler.history_manager() {
                    Some(manager) => match manager.save(&name, agent.conversation()) {
                        Ok(path) => print_formatted_block("INFO", &format!("Saved conversation: {}", path.display())),
                        Err(e) => print_formatted_block("ERROR", &format!("Failed to save conversation: {}", e)),
                    },
                    None => print_formatted_block("ERROR", "History manager is not available."),
                }
            }
            CommandResult::LoadConversation { name } => {
                match command_handler.history_manager() {
                    Some(manager) => match manager.load(&name) {
                        Ok(conversation) => {
                            agent.replace_conversation(conversation);
                            print_formatted_block("INFO", &format!("Loaded conversation: {}", name));
                        }
                        Err(e) => print_formatted_block("ERROR", &format!("Failed to load conversation: {}", e)),
                    },
                    None => print_formatted_block("ERROR", "History manager is not available."),
                }
            }
            CommandResult::ChangeModel { name } => {
                agent.set_model(name.clone());
                print_formatted_block("INFO", &format!("Model changed to: {}", name));
            }
        }
        println!(); // 出力後に空行を追加
    }

    // LSPサーバーをシャットダウン
    if let Some(client) = lsp_client.lock().await.take() {
        if let Err(e) = client.shutdown().await {
            tracing::warn!("Failed to shutdown LSP server: {}", e);
        }
    }

    Ok(())
}

fn find_superpowers_dir() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("LOCAL_CODE_SUPERPOWERS") {
        let dir = PathBuf::from(path);
        if dir.join("skills").exists() {
            return Some(dir);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("superpowers"));
        candidates.push(cwd.join("local-code").join("superpowers"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("superpowers"));
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("superpowers"));
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local-code").join("superpowers"));
    }

    candidates.into_iter().find(|dir| dir.join("skills").exists())
}

/// 特定のコードブロックを置換
fn replace_code_block(content: &str, old_code: &str, new_code: &str, lang: &str) -> String {
    // 元のブロック（言語タグあり/なし両方をカバー）
    let old_block_with_lang = format!("```{}\n{}\n```", lang, old_code);
    let old_block_no_lang = format!("```\n{}\n```", old_code);

    if content.contains(&old_block_with_lang) {
        content.replace(&old_block_with_lang, &format!("```{}\n{}\n```", lang, new_code))
    } else if content.contains(&old_block_no_lang) {
        content.replace(&old_block_no_lang, &format!("```{}\n{}\n```", lang, new_code))
    } else {
        // マッチしない場合は変更なし
        content.to_string()
    }
}
