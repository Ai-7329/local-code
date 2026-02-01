//! プログレス表示（スピナー）モジュール
//!
//! LLM応答待ちやツール実行中にスピナーアニメーションを表示する

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use crossterm::{
    cursor::{Hide, MoveToColumn, Show},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};

/// スピナーのフレーム（Brailleパターン）
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// スピナーのフレーム間隔（ミリ秒）
const FRAME_INTERVAL_MS: u64 = 80;

/// 非同期スピナー構造体
///
/// LLM応答待ちやツール実行中にアニメーション付きのプログレス表示を行う
pub struct Spinner {
    /// スピナーが動作中かどうか
    running: Arc<AtomicBool>,
    /// 現在のメッセージ
    message: Arc<Mutex<String>>,
    /// スピナータスクのハンドル
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Spinner {
    /// 新しいSpinnerインスタンスを作成
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            message: Arc::new(Mutex::new(String::new())),
            handle: None,
        }
    }

    /// スピナーを開始
    ///
    /// # Arguments
    /// * `msg` - 表示するメッセージ
    pub fn start(&mut self, msg: &str) {
        // 既に動作中なら何もしない
        if self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(true, Ordering::SeqCst);

        let running = Arc::clone(&self.running);
        let message = Arc::clone(&self.message);

        // メッセージを設定
        {
            let mut msg_guard = futures::executor::block_on(message.lock());
            *msg_guard = msg.to_string();
        }

        // スピナータスクを起動
        self.handle = Some(tokio::spawn(async move {
            let mut frame_idx = 0;
            let mut stdout = io::stdout();

            // カーソルを非表示
            let _ = execute!(stdout, Hide);

            while running.load(Ordering::SeqCst) {
                let current_msg = {
                    let guard = message.lock().await;
                    guard.clone()
                };

                // 行をクリアしてスピナーとメッセージを表示
                let _ = execute!(
                    stdout,
                    MoveToColumn(0),
                    Clear(ClearType::CurrentLine),
                    SetForegroundColor(Color::Cyan),
                    Print(SPINNER_FRAMES[frame_idx]),
                    ResetColor,
                    Print(format!(" {}", current_msg))
                );
                let _ = stdout.flush();

                frame_idx = (frame_idx + 1) % SPINNER_FRAMES.len();
                tokio::time::sleep(Duration::from_millis(FRAME_INTERVAL_MS)).await;
            }

            // 終了時に行をクリアしてカーソルを表示
            let _ = execute!(
                stdout,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Show
            );
            let _ = stdout.flush();
        }));
    }

    /// スピナーを停止
    pub async fn stop(&mut self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(false, Ordering::SeqCst);

        // タスクの完了を待機
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
    }

    /// 表示メッセージを更新
    ///
    /// # Arguments
    /// * `msg` - 新しいメッセージ
    pub async fn update(&self, msg: &str) {
        let mut guard = self.message.lock().await;
        *guard = msg.to_string();
    }

    /// スピナーが動作中かどうかを返す
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// スピナーを停止してメッセージを表示（成功時）
    pub async fn stop_with_success(&mut self, msg: &str) {
        self.stop().await;
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            SetForegroundColor(Color::Green),
            Print("✓"),
            ResetColor,
            Print(format!(" {}\n", msg))
        );
        let _ = stdout.flush();
    }

    /// スピナーを停止してメッセージを表示（エラー時）
    pub async fn stop_with_error(&mut self, msg: &str) {
        self.stop().await;
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            SetForegroundColor(Color::Red),
            Print("✗"),
            ResetColor,
            Print(format!(" {}\n", msg))
        );
        let _ = stdout.flush();
    }

    /// スピナーを停止してメッセージを表示（情報）
    pub async fn stop_with_info(&mut self, msg: &str) {
        self.stop().await;
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            SetForegroundColor(Color::Blue),
            Print("ℹ"),
            ResetColor,
            Print(format!(" {}\n", msg))
        );
        let _ = stdout.flush();
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        // Dropで確実に停止
        self.running.store(false, Ordering::SeqCst);
        // カーソルを表示状態に戻す
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spinner_creation() {
        let spinner = Spinner::new();
        assert!(!spinner.is_running());
    }

    #[tokio::test]
    async fn test_spinner_start_stop() {
        let mut spinner = Spinner::new();
        spinner.start("Testing...");
        assert!(spinner.is_running());

        // 少し待機
        tokio::time::sleep(Duration::from_millis(100)).await;

        spinner.stop().await;
        assert!(!spinner.is_running());
    }

    #[tokio::test]
    async fn test_spinner_update() {
        let mut spinner = Spinner::new();
        spinner.start("Initial message");

        spinner.update("Updated message").await;

        let msg = spinner.message.lock().await;
        assert_eq!(*msg, "Updated message");

        drop(msg);
        spinner.stop().await;
    }

    #[tokio::test]
    async fn test_spinner_stop_with_variants() {
        let mut spinner = Spinner::new();

        spinner.start("Processing...");
        spinner.stop_with_success("Done!").await;
        assert!(!spinner.is_running());

        spinner.start("Processing...");
        spinner.stop_with_error("Failed!").await;
        assert!(!spinner.is_running());

        spinner.start("Processing...");
        spinner.stop_with_info("Info").await;
        assert!(!spinner.is_running());
    }
}
