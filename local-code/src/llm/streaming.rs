//! ストリーミングレスポンス処理モジュール
//!
//! OLLAMAのストリーミングAPIを使用してリアルタイムにトークンを受信

use anyhow::Result;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct StreamChunk {
    response: String,
    done: bool,
    #[serde(default)]
    #[allow(dead_code)]
    context: Option<Vec<i64>>,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    load_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    prompt_eval_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u32>,
    #[serde(default)]
    eval_duration: Option<u64>,
}

/// ストリーミングレスポンスのチャンク
#[derive(Debug, Clone)]
pub struct StreamChunkData {
    /// テキストコンテンツ
    pub text: String,
    /// ストリームが完了したかどうか
    pub done: bool,
    /// 統計情報（完了時のみ）
    pub stats: Option<StreamStats>,
}

/// ストリーミング完了時の統計情報
#[derive(Debug, Clone)]
pub struct StreamStats {
    /// 総処理時間（ナノ秒）
    pub total_duration: u64,
    /// プロンプト評価トークン数
    pub prompt_eval_count: u32,
    /// 生成トークン数
    pub eval_count: u32,
    /// トークン/秒
    pub tokens_per_second: f64,
}

/// ストリーミングレスポンス
///
/// トークン単位でレスポンスを受信するためのイテレータ風インターフェース
pub struct StreamingResponse {
    receiver: mpsc::Receiver<StreamChunkData>,
    /// 累積されたテキスト
    accumulated_text: String,
}

impl StreamingResponse {
    /// 次のチャンクを取得
    ///
    /// ストリームが終了した場合はNoneを返す
    pub async fn next(&mut self) -> Option<StreamChunkData> {
        if let Some(chunk) = self.receiver.recv().await {
            self.accumulated_text.push_str(&chunk.text);
            Some(chunk)
        } else {
            None
        }
    }

    /// 次のテキストチャンクのみを取得（簡易版）
    pub async fn next_text(&mut self) -> Option<String> {
        self.next().await.map(|chunk| chunk.text)
    }

    /// 累積テキストを取得
    pub fn accumulated(&self) -> &str {
        &self.accumulated_text
    }

    /// 全テキストを収集（ストリーム完了まで待機）
    pub async fn collect_all(&mut self) -> String {
        while self.next().await.is_some() {
            // 全てのチャンクを受信
        }
        self.accumulated_text.clone()
    }

    /// コールバック付きで全テキストを処理
    ///
    /// 各チャンク受信時にコールバックが呼ばれる
    pub async fn process_with_callback<F>(&mut self, mut callback: F) -> String
    where
        F: FnMut(&str),
    {
        while let Some(chunk) = self.next().await {
            callback(&chunk.text);
        }
        self.accumulated_text.clone()
    }
}

/// ストリーミング生成リクエストを送信
pub async fn generate_streaming(
    client: &Client,
    base_url: &str,
    model: &str,
    prompt: &str,
    system: Option<&str>,
) -> Result<StreamingResponse> {
    let (tx, rx) = mpsc::channel(100);

    let request = GenerateRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        stream: true,
        system: system.map(|s| s.to_string()),
    };

    let response = client
        .post(format!("{}/api/generate", base_url))
        .json(&request)
        .send()
        .await?;

    // エラーレスポンスをチェック
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OLLAMAサーバーエラー: {} - {}",
            status,
            body
        ));
    }

    let mut stream = response.bytes_stream();

    tokio::spawn(async move {
        let mut buffer = Vec::new();

        while let Some(chunk) = stream.next().await {
            if let Ok(bytes) = chunk {
                buffer.extend_from_slice(&bytes);

                // 改行で分割してJSONをパース
                while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = buffer.drain(..=pos).collect();
                    if let Ok(text) = std::str::from_utf8(&line) {
                        let trimmed = text.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(trimmed) {
                            let stats = if chunk.done {
                                // 完了時に統計情報を計算
                                let eval_count = chunk.eval_count.unwrap_or(0);
                                let eval_duration = chunk.eval_duration.unwrap_or(1); // 0除算防止
                                let tokens_per_second = if eval_duration > 0 {
                                    (eval_count as f64) / (eval_duration as f64 / 1_000_000_000.0)
                                } else {
                                    0.0
                                };

                                Some(StreamStats {
                                    total_duration: chunk.total_duration.unwrap_or(0),
                                    prompt_eval_count: chunk.prompt_eval_count.unwrap_or(0),
                                    eval_count,
                                    tokens_per_second,
                                })
                            } else {
                                None
                            };

                            let chunk_data = StreamChunkData {
                                text: chunk.response,
                                done: chunk.done,
                                stats,
                            };

                            if tx.send(chunk_data).await.is_err() {
                                return; // レシーバーがドロップされた
                            }

                            if chunk.done {
                                return; // ストリーム完了
                            }
                        }
                    }
                }
            } else {
                // エラーが発生した場合はストリームを終了
                tracing::warn!("ストリーミング中にエラーが発生しました");
                break;
            }
        }
    });

    Ok(StreamingResponse {
        receiver: rx,
        accumulated_text: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_chunk_data() {
        let chunk = StreamChunkData {
            text: "Hello".to_string(),
            done: false,
            stats: None,
        };
        assert_eq!(chunk.text, "Hello");
        assert!(!chunk.done);
        assert!(chunk.stats.is_none());
    }

    #[test]
    fn test_stream_stats() {
        let stats = StreamStats {
            total_duration: 1_000_000_000,
            prompt_eval_count: 10,
            eval_count: 100,
            tokens_per_second: 50.0,
        };
        assert_eq!(stats.total_duration, 1_000_000_000);
        assert_eq!(stats.eval_count, 100);
    }
}
