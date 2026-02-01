//! 色付き出力モジュール
//!
//! CLIの出力を色分けして表示するためのユーティリティ関数を提供
//! ストリーミング出力にも対応

use std::io::{self, Write};
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor, Attribute, SetAttribute},
};

/// Unicodeアイコンとフォールバック文字
pub struct Icons;

impl Icons {
    /// ユーザーアイコン
    pub fn user() -> &'static str {
        if Self::supports_unicode() { "󰀄 " } else { "[U]" }
    }

    /// アシスタントアイコン
    pub fn assistant() -> &'static str {
        if Self::supports_unicode() { "󰚩 " } else { "[A]" }
    }

    /// ツールアイコン
    pub fn tool() -> &'static str {
        if Self::supports_unicode() { "󰒓 " } else { "[T]" }
    }

    /// エラーアイコン
    pub fn error() -> &'static str {
        if Self::supports_unicode() { "󰅚 " } else { "[!]" }
    }

    /// 情報アイコン
    pub fn info() -> &'static str {
        if Self::supports_unicode() { "󰋽 " } else { "[i]" }
    }

    /// 成功アイコン
    pub fn success() -> &'static str {
        if Self::supports_unicode() { "󰄬 " } else { "[+]" }
    }

    /// プロンプトアイコン
    pub fn prompt() -> &'static str {
        if Self::supports_unicode() { "❯" } else { ">" }
    }

    /// Unicode対応チェック（環境変数でオーバーライド可能）
    fn supports_unicode() -> bool {
        // 環境変数でフォールバックを強制
        if std::env::var("LOCAL_CODE_NO_UNICODE").is_ok() {
            return false;
        }
        // TERM環境変数をチェック
        std::env::var("TERM").map_or(false, |term| {
            !term.contains("dumb") && !term.contains("linux")
        })
    }
}

/// ユーザーメッセージを青色+アイコンで出力
pub fn print_user_message(msg: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Blue),
        SetAttribute(Attribute::Bold),
        Print(format!("{}USER:", Icons::user())),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print(format!(" {}\n", msg))
    );
    let _ = stdout.flush();
}

/// アシスタントメッセージを緑色+アイコンで出力
pub fn print_assistant_message(msg: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Green),
        SetAttribute(Attribute::Bold),
        Print(format!("{}ASSISTANT:", Icons::assistant())),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print(format!(" {}\n", msg))
    );
    let _ = stdout.flush();
}

/// ツールメッセージをシアン色+アイコンで出力
pub fn print_tool_message(name: &str, msg: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print(format!("{}TOOL[{}]:", Icons::tool(), name)),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print(format!(" {}\n", msg))
    );
    let _ = stdout.flush();
}

/// エラーメッセージを赤色+アイコンで出力
pub fn print_error_message(msg: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Red),
        SetAttribute(Attribute::Bold),
        Print(format!("{}ERROR:", Icons::error())),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print(format!(" {}\n", msg))
    );
    let _ = stdout.flush();
}

/// エラーメッセージを赤色で出力 (レガシー互換)
pub fn print_error(msg: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Red),
        Print(format!("Error: {}\n", msg)),
        ResetColor
    );
    let _ = stdout.flush();
}

/// 成功メッセージを緑色で出力
pub fn print_success(msg: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Green),
        Print(format!("{}\n", msg)),
        ResetColor
    );
    let _ = stdout.flush();
}

/// ツール実行メッセージを出力（ツール名=シアン、メッセージ=デフォルト）
pub fn print_tool(name: &str, msg: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(format!("[{}]", name)),
        ResetColor,
        Print(format!(" {}\n", msg))
    );
    let _ = stdout.flush();
}

/// モード表示を黄色で出力
pub fn print_mode(mode: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Yellow),
        Print(format!("Mode: {}\n", mode)),
        ResetColor
    );
    let _ = stdout.flush();
}

/// 情報メッセージを青色で出力
pub fn print_info(msg: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Blue),
        Print(format!("{}\n", msg)),
        ResetColor
    );
    let _ = stdout.flush();
}

/// 起動時のバナーを表示
pub fn print_banner(version: &str, mode: &str, model: &str, project: &str, skills: usize) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print(format!("local-code v{}\n", version)),
        SetAttribute(Attribute::Reset),
        ResetColor
    );
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print(format!("Mode: {} | Model: {} | Project: {} | Skills: {}\n", mode, model, project, skills)),
        ResetColor
    );
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print("Tip: /help /skills /status\n"),
        ResetColor
    );
    let _ = stdout.flush();
}

/// Claude Code風の起動バナーを表示
pub fn print_startup_banner(version: &str, model: &str, project: &str, commands: &[String]) {
    let mut stdout = io::stdout();

    // ASCIIアートロゴ（LOCAL）
    const LOGO: &[&str] = &[
        r"  ██╗      ██████╗  ██████╗ █████╗ ██╗     ",
        r"  ██║     ██╔═══██╗██╔════╝██╔══██╗██║     ",
        r"  ██║     ██║   ██║██║     ███████║██║     ",
        r"  ██║     ██║   ██║██║     ██╔══██║██║     ",
        r"  ███████╗╚██████╔╝╚██████╗██║  ██║███████╗",
        r"  ╚══════╝ ╚═════╝  ╚═════╝╚═╝  ╚═╝╚══════╝",
    ];

    // プロジェクトパスを短縮（ホームディレクトリは~に）
    let display_project = shorten_home_path(project);

    // バージョン情報の文字列
    let version_str = format!("v{}", version);

    // ロゴ幅を計算（最長行）
    let logo_width = LOGO.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    // ロゴを表示
    let _ = execute!(stdout, SetForegroundColor(Color::Cyan), SetAttribute(Attribute::Bold));
    for (i, line) in LOGO.iter().enumerate() {
        if i == LOGO.len() - 1 {
            // 最終行にバージョン表示
            let padding = logo_width.saturating_sub(line.chars().count());
            let _ = execute!(
                stdout,
                Print(format!("{}{:>width$}", line, "", width = padding)),
                SetForegroundColor(Color::DarkGrey),
                SetAttribute(Attribute::Reset),
                Print(format!("  {}\n", version_str)),
                SetForegroundColor(Color::Cyan),
                SetAttribute(Attribute::Bold)
            );
        } else {
            let _ = execute!(stdout, Print(format!("{}\n", line)));
        }
    }
    let _ = execute!(stdout, SetAttribute(Attribute::Reset), ResetColor);

    // モデル・プロジェクト情報（右寄せ風に空白でインデント）
    let info_indent = " ".repeat(logo_width.saturating_sub(20));
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print(format!("{}  {} · local\n", info_indent, model)),
        Print(format!("{}  {}\n", info_indent, display_project)),
        ResetColor
    );

    // 空行
    let _ = execute!(stdout, Print("\n"));

    // ヒント表示
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print("  Try \"how does <filepath> work?\"\n"),
        Print("\n"),
        Print("  ? for shortcuts\n"),
        ResetColor
    );

    // superpowersコマンドがあれば表示
    if !commands.is_empty() {
        let _ = execute!(stdout, Print("\n"));
        let _ = execute!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  {} superpowers commands: ", commands.len())),
            SetForegroundColor(Color::Cyan)
        );
        // コマンドを表示（最大5つまで表示、それ以上は省略）
        let display_commands: Vec<&str> = commands.iter().take(5).map(|s| s.as_str()).collect();
        let commands_str = display_commands.join(", ");
        if commands.len() > 5 {
            let _ = execute!(
                stdout,
                Print(format!("{}, ...\n", commands_str)),
                ResetColor
            );
        } else {
            let _ = execute!(
                stdout,
                Print(format!("{}\n", commands_str)),
                ResetColor
            );
        }
    }

    let _ = execute!(stdout, Print("\n"));
    let _ = stdout.flush();
}

/// ホームディレクトリを~に短縮
fn shorten_home_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path.starts_with(home_str.as_ref()) {
            return path.replacen(home_str.as_ref(), "~", 1);
        }
    }
    path.to_string()
}

/// 出力ポストプロセッサ - 不要なブロックを除去
pub struct OutputPostProcessor;

impl OutputPostProcessor {
    /// THOUGHTブロックを除去
    pub fn remove_thought_blocks(content: &str) -> String {
        let mut result = String::new();
        let mut in_thought = false;
        let mut skip_until_newline = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // THOUGHT: で始まる行をスキップ
            if trimmed.starts_with("THOUGHT:") || trimmed.starts_with("**THOUGHT:**") {
                skip_until_newline = true;
                continue;
            }

            // 空行でTHOUGHTスキップをリセット
            if skip_until_newline {
                if trimmed.is_empty() {
                    skip_until_newline = false;
                }
                continue;
            }

            // <thought>タグ内をスキップ
            if trimmed.to_lowercase().starts_with("<thought>") {
                in_thought = true;
                continue;
            }
            if trimmed.to_lowercase().starts_with("</thought>") {
                in_thought = false;
                continue;
            }
            if in_thought {
                continue;
            }

            result.push_str(line);
            result.push('\n');
        }

        result.trim().to_string()
    }

    /// コードブロックのみを抽出（説明文を除去）
    pub fn extract_code_only(content: &str) -> String {
        let blocks = detect_code_blocks(content);
        if blocks.is_empty() {
            return content.to_string();
        }

        blocks
            .iter()
            .map(|b| {
                if let Some(lang) = &b.language {
                    format!("```{}\n{}\n```", lang, b.code)
                } else {
                    format!("```\n{}\n```", b.code)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// 完全なポストプロセス（THOUGHT除去 + コードのみ抽出オプション）
    pub fn process(content: &str, code_only: bool) -> String {
        let cleaned = Self::remove_thought_blocks(content);
        if code_only {
            Self::extract_code_only(&cleaned)
        } else {
            cleaned
        }
    }
}

/// コードブロック情報
#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub code: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// テキスト内のコードブロックを検出
/// ```language で始まり ``` で終わるブロックを検出
pub fn detect_code_blocks(content: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with("```") {
            let language = if line.len() > 3 {
                Some(line[3..].trim().to_string())
            } else {
                None
            };
            let start_line = i;
            let mut code_lines = Vec::new();
            i += 1;

            // 終了の ``` を探す
            while i < lines.len() {
                if lines[i].trim() == "```" {
                    blocks.push(CodeBlock {
                        language,
                        code: code_lines.join("\n"),
                        start_line,
                        end_line: i,
                    });
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }
        }
        i += 1;
    }

    blocks
}

/// コードブロックを枠線付きで表示
pub fn print_code_block(block: &CodeBlock) {
    let mut stdout = io::stdout();
    let lines: Vec<&str> = block.code.lines().collect();

    // 最大幅を計算
    let max_width = lines.iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(40);

    let border = "─".repeat(max_width + 2);

    // 上枠（言語名付き）
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey)
    );

    if let Some(lang) = &block.language {
        let lang_display = format!("─[ {} ]", lang);
        let remaining = max_width + 2 - lang_display.chars().count();
        let _ = execute!(
            stdout,
            Print(format!("╭{}{}\n", lang_display, "─".repeat(remaining.max(0))))
        );
    } else {
        let _ = execute!(
            stdout,
            Print(format!("╭{}╮\n", border))
        );
    }

    // コード内容
    let _ = execute!(stdout, SetForegroundColor(Color::White));
    for line in &lines {
        let _ = execute!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print("│ "),
            SetForegroundColor(Color::White),
            Print(format!("{:<width$}", line, width = max_width)),
            SetForegroundColor(Color::DarkGrey),
            Print(" │\n")
        );
    }

    // 下枠
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print(format!("╰{}╯\n", border)),
        ResetColor
    );

    let _ = stdout.flush();
}

/// フォーマットされたブロックを表示
/// タイプに応じてアイコンと色を適用し、コードブロックを検出して整形
pub fn print_formatted_block(title: &str, content: &str) {
    let mut stdout = io::stdout();

    // タイトルに応じた色とアイコンを決定
    let (color, icon) = match title.to_uppercase().as_str() {
        "USER" => (Color::Blue, Icons::user()),
        "ASSISTANT" => (Color::Green, Icons::assistant()),
        "TOOL" => (Color::Cyan, Icons::tool()),
        "ERROR" => (Color::Red, Icons::error()),
        "INFO" => (Color::Blue, Icons::info()),
        "SKILL" => (Color::Magenta, Icons::tool()),
        _ => (Color::White, ""),
    };

    // タイトル表示
    let _ = execute!(
        stdout,
        SetForegroundColor(color),
        SetAttribute(Attribute::Bold),
        Print(format!("{}{}", icon, title)),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print(":\n")
    );

    // コードブロックを検出
    let code_blocks = detect_code_blocks(content);

    if code_blocks.is_empty() {
        // コードブロックがなければそのまま表示
        let _ = execute!(stdout, Print(format!("{}\n", content)));
    } else {
        // コードブロックがある場合は整形して表示
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut block_idx = 0;

        while i < lines.len() {
            if block_idx < code_blocks.len() && i == code_blocks[block_idx].start_line {
                // コードブロックを表示
                print_code_block(&code_blocks[block_idx]);
                i = code_blocks[block_idx].end_line + 1;
                block_idx += 1;
            } else {
                // 通常のテキスト行
                let _ = execute!(stdout, Print(format!("{}\n", lines[i])));
                i += 1;
            }
        }
    }

    let _ = stdout.flush();
}

/// ストリーミング出力ライター
///
/// リアルタイムで文字単位の出力を行う
pub struct StreamingWriter {
    stdout: io::Stdout,
    color: Option<Color>,
    buffer: String,
}

impl StreamingWriter {
    /// 新しいストリーミングライターを作成
    pub fn new() -> Self {
        Self {
            stdout: io::stdout(),
            color: None,
            buffer: String::new(),
        }
    }

    /// 色付きストリーミングライターを作成
    pub fn with_color(color: Color) -> Self {
        Self {
            stdout: io::stdout(),
            color: Some(color),
            buffer: String::new(),
        }
    }

    /// ストリーミング開始（プレフィックスを表示）
    pub fn start(&mut self, prefix: Option<&str>) {
        if let Some(p) = prefix {
            let _ = execute!(
                self.stdout,
                SetForegroundColor(Color::Cyan),
                Print(format!("{} ", p)),
                ResetColor
            );
        }

        // 色を設定
        if let Some(color) = self.color {
            let _ = execute!(self.stdout, SetForegroundColor(color));
        }
        let _ = self.stdout.flush();
    }

    /// 文字単位で出力（フラッシュあり）
    pub fn write_char(&mut self, c: char) {
        self.buffer.push(c);
        print!("{}", c);
        let _ = self.stdout.flush();
    }

    /// テキストを出力（フラッシュあり）
    pub fn write(&mut self, text: &str) {
        self.buffer.push_str(text);
        print!("{}", text);
        let _ = self.stdout.flush();
    }

    /// テキストを即座に出力（バッファリングなし）
    pub fn write_immediate(&mut self, text: &str) {
        print!("{}", text);
        let _ = self.stdout.flush();
    }

    /// ストリーミング終了
    pub fn finish(&mut self) {
        if self.color.is_some() {
            let _ = execute!(self.stdout, ResetColor);
        }
        println!(); // 改行
        let _ = self.stdout.flush();
    }

    /// 統計情報を表示して終了
    pub fn finish_with_stats(&mut self, tokens_per_second: f64, total_tokens: u32) {
        if self.color.is_some() {
            let _ = execute!(self.stdout, ResetColor);
        }
        println!(); // 改行

        // 統計情報を暗い色で表示
        let _ = execute!(
            self.stdout,
            SetForegroundColor(Color::DarkGrey),
            SetAttribute(Attribute::Dim),
            Print(format!("[{} tokens, {:.1} tok/s]\n", total_tokens, tokens_per_second)),
            SetAttribute(Attribute::Reset),
            ResetColor
        );
        let _ = self.stdout.flush();
    }

    /// バッファの内容を取得
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// バッファをクリア
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }
}

impl Default for StreamingWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// ストリーミング出力を開始（シンプルなAPI）
pub fn print_streaming_start(prefix: Option<&str>) {
    let mut stdout = io::stdout();
    if let Some(p) = prefix {
        let _ = execute!(
            stdout,
            SetForegroundColor(Color::Cyan),
            Print(format!("{} ", p)),
            ResetColor
        );
    }
    let _ = stdout.flush();
}

/// ストリーミングテキストを出力（即座にフラッシュ）
pub fn print_streaming_text(text: &str) {
    let mut stdout = io::stdout();
    print!("{}", text);
    let _ = stdout.flush();
}

/// ストリーミング出力を終了
pub fn print_streaming_end() {
    println!();
    let _ = io::stdout().flush();
}

/// 統計情報付きでストリーミング出力を終了
pub fn print_streaming_end_with_stats(tokens_per_second: f64, total_tokens: u32) {
    let mut stdout = io::stdout();
    println!();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print(format!("[{} tokens, {:.1} tok/s]\n", total_tokens, tokens_per_second)),
        ResetColor
    );
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_functions_do_not_panic() {
        // 各関数がパニックしないことを確認
        print_error("test error");
        print_success("test success");
        print_tool("TestTool", "test message");
        print_mode("TestMode");
        print_info("test info");
    }

    #[test]
    fn test_streaming_writer() {
        let mut writer = StreamingWriter::new();
        writer.write("Hello");
        writer.write_char(' ');
        writer.write("World");
        assert_eq!(writer.buffer(), "Hello World");
    }

    #[test]
    fn test_streaming_writer_with_color() {
        let mut writer = StreamingWriter::with_color(Color::Green);
        writer.start(Some("AI:"));
        writer.write("Test");
        // パニックしないことを確認
    }
}
