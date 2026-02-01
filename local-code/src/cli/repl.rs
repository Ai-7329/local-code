use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor, Attribute, SetAttribute},
    terminal::{self, ClearType},
};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::future::Future;

use super::completion::{Completer, CompletionResult};
use super::output::Icons;

/// コマンド履歴を管理する構造体
pub struct CommandHistory {
    history: Vec<String>,
    position: usize,
    history_file: PathBuf,
    max_history: usize,
}

impl CommandHistory {
    pub fn new() -> Self {
        let history_file = Self::get_history_file_path();
        let history = Self::load_from_file(&history_file);
        let position = history.len();

        Self {
            history,
            position,
            history_file,
            max_history: 1000,
        }
    }

    fn get_history_file_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = home.join(".local-code");

        // ディレクトリが存在しない場合は作成
        if !config_dir.exists() {
            let _ = fs::create_dir_all(&config_dir);
        }

        config_dir.join("command_history")
    }

    fn load_from_file(path: &PathBuf) -> Vec<String> {
        if !path.exists() {
            return Vec::new();
        }

        match File::open(path) {
            Ok(file) => {
                BufReader::new(file)
                    .lines()
                    .filter_map(|line| line.ok())
                    .filter(|line| !line.is_empty())
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    fn save_to_file(&self) -> Result<()> {
        let mut file = File::create(&self.history_file)?;

        // 最大履歴数を超えた場合は古いものを削除
        let start = if self.history.len() > self.max_history {
            self.history.len() - self.max_history
        } else {
            0
        };

        for cmd in &self.history[start..] {
            writeln!(file, "{}", cmd)?;
        }

        Ok(())
    }

    /// コマンドを履歴に追加
    pub fn add(&mut self, cmd: String) {
        // 空のコマンドは追加しない
        if cmd.trim().is_empty() {
            return;
        }

        // 直前と同じコマンドは追加しない
        if self.history.last().map_or(false, |last| last == &cmd) {
            self.position = self.history.len();
            return;
        }

        self.history.push(cmd);
        self.position = self.history.len();

        // ファイルに保存
        let _ = self.save_to_file();
    }

    /// 前の履歴を取得
    pub fn prev(&mut self) -> Option<&String> {
        if self.history.is_empty() {
            return None;
        }

        if self.position > 0 {
            self.position -= 1;
        }

        self.history.get(self.position)
    }

    /// 次の履歴を取得
    pub fn next(&mut self) -> Option<&String> {
        if self.history.is_empty() {
            return None;
        }

        if self.position < self.history.len() {
            self.position += 1;
        }

        if self.position >= self.history.len() {
            None // 最新位置では空を返す（新規入力用）
        } else {
            self.history.get(self.position)
        }
    }

    /// 位置をリセット（最新位置に戻す）
    pub fn reset_position(&mut self) {
        self.position = self.history.len();
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Repl {
    command_history: CommandHistory,
    prompt: String,
    mode: String,
    model: String,
    completer: Completer,
    completion_state: Option<CompletionState>,
    superpowers_commands: Vec<String>,
    superpowers_cycle: Option<SuperpowersCycleState>,
    workflow_next_index: usize,  // 次回の初期インデックス
}

struct CompletionState {
    seed: String,
    candidates: Vec<String>,
    index: usize,
    from_empty: bool,  // 空入力から開始したか
}

/// Superpowersコマンドサイクル状態
struct SuperpowersCycleState {
    index: usize,
    workflow_index: usize,  // ワークフロー進行位置を保持
}

impl Repl {
    pub fn new() -> Self {
        Self {
            command_history: CommandHistory::new(),
            prompt: "> ".to_string(),
            mode: "Plan".to_string(),
            model: "ollama".to_string(),
            completer: Completer::new(),
            completion_state: None,
            superpowers_commands: Vec::new(),
            superpowers_cycle: None,
            workflow_next_index: 0,
        }
    }

    /// モードを設定
    pub fn set_mode(&mut self, mode: String) {
        self.mode = mode;
        self.update_prompt();
    }

    /// モデルを設定
    pub fn set_model(&mut self, model: String) {
        self.model = model;
        self.update_prompt();
    }

    /// プロンプトを更新（内部用）
    fn update_prompt(&mut self) {
        let prompt_icon = Icons::prompt();
        self.prompt = format!("[{}|{}] {} ", self.mode, self.model, prompt_icon);
    }

    /// superpowersコマンドを設定
    pub fn set_superpowers_commands(&mut self, commands: Vec<String>) {
        self.superpowers_commands = commands.clone();
        // Completerにも設定
        self.completer.set_extra_commands(commands);
    }

    /// スキル名を設定（補完用）
    pub fn set_skills(&mut self, skills: Vec<String>) {
        self.completer.set_skills(skills);
    }

    /// 追加コマンドを設定（補完用）
    pub fn set_commands(&mut self, commands: Vec<String>) {
        self.completer.set_extra_commands(commands);
    }

    /// プロンプトを設定
    pub fn set_prompt(&mut self, prompt: String) {
        self.prompt = prompt;
    }

    /// プロンプトを表示（色付き）
    pub fn print_prompt(&self) -> Result<()> {
        self.print_prompt_with_icon(None)
    }

    /// モードアイコン付きプロンプトを表示
    pub fn print_prompt_with_icon(&self, mode_icon: Option<&str>) -> Result<()> {
        let mut stdout = io::stdout();
        let icon = mode_icon.unwrap_or(if self.mode.to_lowercase() == "plan" { "📋" } else { "⏵⏵" });

        // アイコン Mode (shift+tab) ❯ 形式で表示
        let _ = execute!(
            stdout,
            SetForegroundColor(Color::Magenta),
            Print(format!("{} ", icon)),
            ResetColor,
            SetForegroundColor(Color::Yellow),
            Print(format!("{}", self.mode)),
            ResetColor,
            SetForegroundColor(Color::DarkGrey),
            Print(" (shift+tab)"),
            ResetColor,
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            Print(format!(" {} ", Icons::prompt())),
            SetAttribute(Attribute::Reset),
            ResetColor
        );
        stdout.flush()?;
        Ok(())
    }

    /// サイクル状態付きプロンプトを表示
    pub fn print_prompt_with_cycle(&self) -> Result<()> {
        let mut stdout = io::stdout();

        if let Some(state) = &self.superpowers_cycle {
            let total = self.superpowers_commands.len();
            let idx = state.index + 1;
            execute!(
                stdout,
                SetForegroundColor(Color::Magenta),
                Print("⏵⏵ "),
                SetForegroundColor(Color::DarkGrey),
                Print(format!("[{}/{}] ", idx, total)),
                ResetColor
            )?;
        } else {
            execute!(
                stdout,
                SetForegroundColor(Color::Cyan),
                SetAttribute(Attribute::Bold),
                Print(format!("{} ", Icons::prompt())),
                SetAttribute(Attribute::Reset),
                ResetColor
            )?;
        }
        stdout.flush()?;
        Ok(())
    }

    /// プロンプト文字列を取得（履歴表示用）
    pub fn prompt_str(&self) -> &str {
        &self.prompt
    }

    /// 作業ディレクトリを設定（補完用）
    pub fn set_working_dir(&mut self, path: PathBuf) {
        self.completer.set_working_dir(path);
    }

    pub async fn run<F, Fut>(&mut self, mut on_message: F) -> Result<()>
    where
        F: FnMut(&str) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        println!("local-code REPL (type /help for commands, /quit to exit)\n");

        loop {
            print!("{}", self.prompt);
            io::stdout().flush()?;

            let input = match self.read_line_with_history() {
                Ok(line) => line,
                Err(e) => {
                    // Ctrl+C などの場合はスキップ
                    if e.to_string().contains("interrupted") {
                        println!();
                        continue;
                    }
                    return Err(e);
                }
            };

            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            self.command_history.add(input.to_string());

            // コマンド処理
            if input.starts_with('/') {
                match self.handle_command(input).await {
                    Ok(should_quit) => {
                        if should_quit {
                            println!("Goodbye!");
                            break;
                        }
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
            } else {
                // 通常の入力（LLMに送信）
                on_message(input).await?;
            }
        }

        Ok(())
    }

    /// crosstermを使用して履歴対応の行読み取り
    pub fn read_line_with_history(&mut self) -> Result<String> {
        terminal::enable_raw_mode()?;

        let result = self.read_line_internal();

        terminal::disable_raw_mode()?;
        println!(); // 改行を追加

        result
    }

    fn read_line_internal(&mut self) -> Result<String> {
        let mut input = String::new();
        let mut cursor_pos: usize = 0; // char index
        let mut stdout = io::stdout();

        self.command_history.reset_position();

        loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.code != KeyCode::Tab {
                        self.completion_state = None;
                    }
                    match key_event {
                        KeyEvent {
                            code: KeyCode::Enter,
                            ..
                        } => {
                            // ペースト検出: 短時間内に次の入力があれば改行として扱う
                            if event::poll(std::time::Duration::from_millis(30))? {
                                // ペースト中 - 改行を挿入して継続
                                let byte_idx = byte_index(&input, cursor_pos);
                                input.insert(byte_idx, '\n');
                                cursor_pos += 1;
                                // 改行を表示
                                write!(stdout, "\r\n")?;
                                // 残りの文字を再描画
                                let remaining = &input[byte_index(&input, cursor_pos)..];
                                if !remaining.is_empty() {
                                    write!(stdout, "{}", remaining)?;
                                    let remaining_chars = char_len(remaining);
                                    if remaining_chars > 0 {
                                        execute!(stdout, cursor::MoveLeft(remaining_chars as u16))?;
                                    }
                                }
                                stdout.flush()?;
                                continue;
                            }
                            // 通常のEnter - 入力確定
                            // 実行したコマンドがsuperpowersの場合、次のインデックスを記録
                            if let Some(idx) = self.superpowers_commands.iter().position(|c| c == &input) {
                                self.workflow_next_index = (idx + 1) % self.superpowers_commands.len();
                            }
                            self.superpowers_cycle = None;  // サイクルをリセット
                            break;
                        }
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            return Err(anyhow::anyhow!("interrupted"));
                        }
                        KeyEvent {
                            code: KeyCode::Char('d'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            if input.is_empty() {
                                return Ok("/quit".to_string());
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Esc,
                            ..
                        } => {
                            if self.superpowers_cycle.is_some() {
                                self.superpowers_cycle = None;
                                Self::clear_line_static(&mut stdout, cursor_pos)?;
                                input.clear();
                                cursor_pos = 0;
                                continue;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Up,
                            ..
                        } => {
                            let prev_cmd = self.command_history.prev().cloned();
                            if let Some(cmd) = prev_cmd {
                                // 現在の入力をクリアして履歴を表示
                                Self::clear_line_static(&mut stdout, cursor_pos)?;
                                input = cmd;
                                cursor_pos = input.len();
                                write!(stdout, "{}", input)?;
                                stdout.flush()?;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Down,
                            ..
                        } => {
                            let next_cmd = self.command_history.next().cloned();
                            Self::clear_line_static(&mut stdout, cursor_pos)?;
                            if let Some(cmd) = next_cmd {
                                input = cmd;
                            } else {
                                input.clear();
                            }
                            cursor_pos = input.len();
                            write!(stdout, "{}", input)?;
                            stdout.flush()?;
                        }
                        KeyEvent {
                            code: KeyCode::Left,
                            ..
                        } => {
                            if cursor_pos > 0 {
                                cursor_pos -= 1;
                                execute!(stdout, cursor::MoveLeft(1))?;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Right,
                            ..
                        } => {
                            if cursor_pos < char_len(&input) {
                                cursor_pos += 1;
                                execute!(stdout, cursor::MoveRight(1))?;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Home,
                            ..
                        } => {
                            if cursor_pos > 0 {
                                execute!(stdout, cursor::MoveLeft(cursor_pos as u16))?;
                                cursor_pos = 0;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::End,
                            ..
                        } => {
                            let total = char_len(&input);
                            if cursor_pos < total {
                                let move_right = total - cursor_pos;
                                execute!(stdout, cursor::MoveRight(move_right as u16))?;
                                cursor_pos = total;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Backspace,
                            ..
                        } => {
                            if cursor_pos > 0 {
                                cursor_pos -= 1;
                                let byte_idx = byte_index(&input, cursor_pos);
                                input.remove(byte_idx);

                                // カーソルを左に移動して、残りの文字を再描画
                                execute!(stdout, cursor::MoveLeft(1))?;
                                let remaining = &input[byte_index(&input, cursor_pos)..];
                                write!(stdout, "{} ", remaining)?;
                                // カーソルを正しい位置に戻す
                                let move_back = char_len(remaining) + 1;
                                execute!(stdout, cursor::MoveLeft(move_back as u16))?;
                                stdout.flush()?;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Delete,
                            ..
                        } => {
                            if cursor_pos < char_len(&input) {
                                let byte_idx = byte_index(&input, cursor_pos);
                                input.remove(byte_idx);

                                // 残りの文字を再描画
                                let remaining = &input[byte_index(&input, cursor_pos)..];
                                write!(stdout, "{} ", remaining)?;
                                let move_back = char_len(remaining) + 1;
                                execute!(stdout, cursor::MoveLeft(move_back as u16))?;
                                stdout.flush()?;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::BackTab,
                            ..
                        } => {
                            // Shift+Tab: Superpowersコマンドをサイクル
                            if self.superpowers_commands.is_empty() {
                                continue;
                            }

                            // サイクル状態を更新
                            let next_index = match &mut self.superpowers_cycle {
                                Some(state) => {
                                    state.index = (state.index + 1) % self.superpowers_commands.len();
                                    state.index
                                }
                                None => {
                                    // ワークフロー位置から開始
                                    let start_idx = self.workflow_next_index;
                                    self.superpowers_cycle = Some(SuperpowersCycleState {
                                        index: start_idx,
                                        workflow_index: start_idx,
                                    });
                                    start_idx
                                }
                            };

                            // 入力を選択中コマンドに置換
                            let cmd = self.superpowers_commands[next_index].clone();
                            Self::clear_line_static(&mut stdout, cursor_pos)?;
                            input = cmd;
                            cursor_pos = input.len();
                            write!(stdout, "{}", input)?;
                            stdout.flush()?;
                        }
                        KeyEvent {
                            code: KeyCode::Tab,
                            ..
                        } => {
                            // 前回の補完状態を継続するか判定（現在のインデックスの候補と一致するか）
                            let continue_empty_cycle = self.completion_state.as_ref()
                                .map(|s| s.from_empty && s.candidates.get(s.index).map(|c| c == &input).unwrap_or(false))
                                .unwrap_or(false);

                            if input.is_empty() || continue_empty_cycle {
                                // 空入力 → Superpowersコマンドのみをサイクル
                                let candidates = self.completer.get_superpowers_commands();
                                if candidates.is_empty() {
                                    continue;
                                }

                                if !continue_empty_cycle {
                                    self.completion_state = Some(CompletionState {
                                        seed: String::new(),
                                        candidates,
                                        index: 0,
                                        from_empty: true,
                                    });
                                } else if let Some(state) = &mut self.completion_state {
                                    state.index = (state.index + 1) % state.candidates.len();
                                }

                                if let Some(state) = &self.completion_state {
                                    Self::clear_line_static(&mut stdout, cursor_pos)?;
                                    input = state.candidates[state.index].clone();
                                    cursor_pos = input.len();
                                    write!(stdout, "{}", input)?;
                                    stdout.flush()?;
                                }
                            } else if input.starts_with('/') {
                                // "/" で始まる → 従来のコマンド補完（全コマンド対象）
                                let seed = self
                                    .completion_state
                                    .as_ref()
                                    .map(|state| state.seed.clone())
                                    .unwrap_or_else(|| input.clone());
                                let candidates = self.completer.complete(&seed);
                                if candidates.is_empty() {
                                    continue;
                                }

                                let needs_reset = self
                                    .completion_state
                                    .as_ref()
                                    .map(|state| state.seed != seed || state.candidates != candidates)
                                    .unwrap_or(true);

                                if needs_reset {
                                    self.completion_state = Some(CompletionState {
                                        seed,
                                        candidates,
                                        index: 0,
                                        from_empty: false,
                                    });
                                } else if let Some(state) = &mut self.completion_state {
                                    state.index = (state.index + 1) % state.candidates.len();
                                }

                                if let Some(state) = &self.completion_state {
                                    Self::clear_line_static(&mut stdout, cursor_pos)?;
                                    input = state.candidates[state.index].clone();
                                    cursor_pos = input.len();
                                    write!(stdout, "{}", input)?;
                                    stdout.flush()?;
                                }
                            } else {
                                // パス補完は従来通り
                                match self.completer.complete_with_result(&input) {
                                    CompletionResult::Single(completion) => {
                                        Self::clear_line_static(&mut stdout, cursor_pos)?;
                                        input = completion;
                                        cursor_pos = input.len();
                                        write!(stdout, "{}", input)?;
                                        stdout.flush()?;
                                    }
                                    CompletionResult::Multiple { common_prefix, candidates } => {
                                        if common_prefix.len() > input.len() {
                                            Self::clear_line_static(&mut stdout, cursor_pos)?;
                                            input = common_prefix;
                                            cursor_pos = input.len();
                                            write!(stdout, "{}", input)?;
                                            stdout.flush()?;
                                        } else {
                                            write!(stdout, "\r\n")?;
                                            for (i, candidate) in candidates.iter().enumerate() {
                                                if i > 0 && i % 4 == 0 {
                                                    write!(stdout, "\r\n")?;
                                                }
                                                write!(stdout, "{:<20}", candidate)?;
                                            }
                                            write!(stdout, "\r\n{}{}", self.prompt, input)?;
                                            stdout.flush()?;
                                        }
                                    }
                                    CompletionResult::None => {}
                                }
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char(c),
                            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                            ..
                        } => {
                            self.superpowers_cycle = None;  // 通常入力でサイクルをリセット
                            let byte_idx = byte_index(&input, cursor_pos);
                            input.insert(byte_idx, c);
                            cursor_pos += 1;

                            // カーソル位置に文字を挿入
                            let remaining = &input[byte_index(&input, cursor_pos - 1)..];
                            write!(stdout, "{}", remaining)?;

                            // カーソルを正しい位置に戻す
                            let remaining_chars = char_len(remaining);
                            if remaining_chars > 1 {
                                execute!(stdout, cursor::MoveLeft((remaining_chars - 1) as u16))?;
                            }
                            stdout.flush()?;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(input)
    }

    /// 現在の行をクリア（静的メソッド）
    fn clear_line_static(stdout: &mut io::Stdout, cursor_pos: usize) -> Result<()> {
        // カーソルを行頭に移動
        if cursor_pos > 0 {
            execute!(stdout, cursor::MoveLeft(cursor_pos as u16))?;
        }
        // 行末までクリア
        execute!(stdout, terminal::Clear(ClearType::UntilNewLine))?;
        Ok(())
    }

    pub fn read_line(&self) -> Result<String> {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input)
    }

    async fn handle_command(&self, input: &str) -> Result<bool> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts.first().unwrap_or(&"");

        match *command {
            "/quit" | "/q" | "/exit" => Ok(true),
            "/help" | "/h" => {
                self.print_help();
                Ok(false)
            }
            "/plan" => {
                println!("Switched to Plan mode (read-only tools)");
                Ok(false)
            }
            "/execute" | "/exec" => {
                println!("Switched to Execute mode (all tools available)");
                Ok(false)
            }
            "/clear" => {
                print!("\x1B[2J\x1B[1;1H");
                Ok(false)
            }
            "/history" => {
                self.print_history();
                Ok(false)
            }
            _ => {
                println!("Unknown command: {}. Type /help for available commands.", command);
                Ok(false)
            }
        }
    }

    fn print_help(&self) {
        println!("
Commands:
  /help, /h       - Show this help message
  /quit, /q       - Exit the REPL
  /plan           - Switch to Plan mode (read-only tools)
  /execute, /exec - Switch to Execute mode (all tools)
  /clear          - Clear the screen
  /history        - Show command history
  /<skill-name>   - Run a skill

Navigation:
  Up/Down arrows  - Navigate command history
  Left/Right      - Move cursor
  Home/End        - Jump to start/end of line
  Ctrl+C          - Cancel current input
  Ctrl+D          - Exit (when input is empty)

Enter text to chat with the AI.
");
    }

    fn print_history(&self) {
        println!("\nCommand History:");
        println!("----------------");
        for (i, cmd) in self.command_history.history.iter().enumerate() {
            println!("{:4}: {}", i + 1, cmd);
        }
        println!();
    }
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

fn byte_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| text.len())
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}
