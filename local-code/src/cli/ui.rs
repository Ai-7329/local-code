use anyhow::Result;
use crossterm::{
    cursor,
    execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Write};

const SEPARATOR_MARK: &str = "__LOCAL_CODE_SEPARATOR__";

#[derive(Debug, Clone, Default)]
pub struct StatusLine {
    pub mode: String,
    pub model: String,
    pub project: String,
    pub skills: usize,
    pub commands: Vec<String>,
}

pub struct Ui {
    title: String,
    log: Vec<String>,
    status: StatusLine,
}

impl Ui {
    pub fn new(title: String) -> Self {
        Self {
            title,
            log: Vec::new(),
            status: StatusLine::default(),
        }
    }

    pub fn set_status(&mut self, status: StatusLine) {
        self.status = status;
    }

    pub fn clear(&mut self) {
        self.log.clear();
    }

    pub fn push_line(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
    }

    pub fn push_separator(&mut self) {
        self.log.push(SEPARATOR_MARK.to_string());
    }

    pub fn push_block(&mut self, title: &str, text: &str) {
        self.push_separator();
        self.push_line(format!("{}:", title));
        self.push_text(text);
    }

    pub fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            self.log.push(String::new());
            return;
        }
        for line in text.lines() {
            self.log.push(line.to_string());
        }
    }

    pub fn render(&self, prompt: &str) -> Result<()> {
        let mut stdout = io::stdout();
        let (cols, rows) = terminal::size().unwrap_or((120, 40));
        let cols = cols as usize;
        let rows = rows as usize;

        let header_lines = vec![
            self.title.clone(),
            format!(
                "Model: {} | Project: {} | Skills: {}",
                self.status.model,
                shorten_home_path(&self.status.project),
                self.status.skills
            ),
            "-".repeat(cols.max(1)),
        ];

        let commands_count = self.status.commands.len();
        let footer_lines = vec![
            "-".repeat(cols.max(1)),
            format!(
                "Mode: {} | Model: {} | Commands: {} | Skills: {}",
                self.status.mode, self.status.model, commands_count, self.status.skills
            ),
        ];

        let reserved = header_lines.len() + footer_lines.len() + 1;
        let log_height = rows.saturating_sub(reserved);

        let mut wrapped_log: Vec<String> = Vec::new();
        for line in &self.log {
            if line == SEPARATOR_MARK {
                wrapped_log.push("-".repeat(cols.max(1)));
                continue;
            }
            wrapped_log.extend(wrap_line(line, cols));
        }

        let start = wrapped_log.len().saturating_sub(log_height);
        let visible_log = &wrapped_log[start..];

        execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        execute!(
            stdout,
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            Print(format!("{}\n", header_lines[0])),
            SetAttribute(Attribute::Reset),
            ResetColor
        )?;
        execute!(stdout, Print(format!("{}\n", header_lines[1])))?;
        execute!(stdout, Print(format!("{}\n", header_lines[2])))?;

        for line in visible_log {
            execute!(stdout, Print(format!("{}\n", line)))?;
        }

        for _ in visible_log.len()..log_height {
            execute!(stdout, Print("\n"))?;
        }

        execute!(stdout, Print(format!("{}\n", footer_lines[0])))?;
        execute!(stdout, Print(format!("{}\n", footer_lines[1])))?;
        execute!(stdout, Print(prompt))?;
        stdout.flush()?;

        Ok(())
    }
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }

    let mut out = Vec::new();
    let mut current = String::new();
    let mut count = 0usize;

    for ch in line.chars() {
        if count >= width {
            out.push(current);
            current = String::new();
            count = 0;
        }
        current.push(ch);
        count += 1;
    }

    if current.is_empty() && !line.is_empty() {
        out.push(String::new());
    } else {
        out.push(current);
    }

    out
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

// ============================================================
// シンプル出力関数（フルスクリーンUIの代わりに使用）
// ============================================================

/// セパレータを出力
pub fn print_separator() {
    let (cols, _) = terminal::size().unwrap_or((80, 24));
    println!("{}", "-".repeat(cols as usize));
}

/// 情報メッセージを出力
pub fn print_info(message: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(message),
        Print("\n"),
        ResetColor
    );
}

/// エラーメッセージを出力
pub fn print_error(message: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Red),
        SetAttribute(Attribute::Bold),
        Print("ERROR: "),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print(message),
        Print("\n")
    );
}

/// フォーマット済みブロックを出力（タイトル付き）
pub fn print_formatted_block(title: &str, content: &str) {
    let mut stdout = io::stdout();

    // タイトルに応じて色を設定
    let color = match title.to_uppercase().as_str() {
        "USER" => Color::Green,
        "ASSISTANT" => Color::Blue,
        "ERROR" => Color::Red,
        "INFO" => Color::Cyan,
        "SKILL" => Color::Magenta,
        _ => Color::Yellow,
    };

    print_separator();
    let _ = execute!(
        stdout,
        SetForegroundColor(color),
        SetAttribute(Attribute::Bold),
        Print(format!("{}:\n", title)),
        SetAttribute(Attribute::Reset),
        ResetColor
    );

    if !content.is_empty() {
        println!("{}", content);
    }
}

/// 処理中メッセージを出力
pub fn print_processing(message: &str) {
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetForegroundColor(Color::Yellow),
        Print(format!("{}\n", message)),
        ResetColor
    );
}
