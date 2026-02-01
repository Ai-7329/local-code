//! OLLAMAクライアント - エラーリトライ機能付き
//!
//! 接続エラー時の自動リトライ（エクスポネンシャルバックオフ）をサポート
//! ストリーミング出力にも対応

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

use crate::config::{OllamaConfig, RetryConfig};
use super::streaming::{generate_streaming as streaming_impl, StreamingResponse};

/// リトライ可能なエラーの種類
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RetryableError {
    /// 接続エラー（サーバーに到達できない）
    Connection,
    /// タイムアウト
    Timeout,
    /// サーバーエラー（5xx）
    ServerError,
    /// リクエストエラー（リトライ不可）
    NonRetryable,
}

impl RetryableError {
    /// reqwestエラーからリトライ可能かどうかを判定
    pub fn from_reqwest_error(error: &reqwest::Error) -> Self {
        if error.is_connect() {
            RetryableError::Connection
        } else if error.is_timeout() {
            RetryableError::Timeout
        } else if let Some(status) = error.status() {
            if status.is_server_error() {
                RetryableError::ServerError
            } else {
                RetryableError::NonRetryable
            }
        } else {
            // ネットワーク関連のエラーはリトライ可能とみなす
            if error.is_request() {
                RetryableError::Connection
            } else {
                RetryableError::NonRetryable
            }
        }
    }

    /// リトライ可能かどうか
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            RetryableError::Connection | RetryableError::Timeout | RetryableError::ServerError
        )
    }

    /// エラーの説明
    pub fn description(&self) -> &'static str {
        match self {
            RetryableError::Connection => "接続エラー",
            RetryableError::Timeout => "タイムアウト",
            RetryableError::ServerError => "サーバーエラー",
            RetryableError::NonRetryable => "リクエストエラー",
        }
    }
}

#[derive(Clone)]
pub struct OllamaClient {
    client: Client,
    base_url: String,
    model: String,
    retry_config: RetryConfig,
}

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GenerateResponse {
    pub model: String,
    pub response: String,
    pub done: bool,
}

impl OllamaClient {
    fn build_client(connect_timeout_secs: u64, read_timeout_secs: u64) -> Client {
        Client::builder()
            .connect_timeout(Duration::from_secs(connect_timeout_secs))
            .read_timeout(Duration::from_secs(read_timeout_secs))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| Client::new())
    }

    /// 基本的なクライアントを作成（デフォルトタイムアウト使用）
    pub fn new(base_url: &str, model: &str) -> Self {
        Self::with_timeout(base_url, model, 30, 300)
    }

    /// タイムアウト設定付きでクライアントを作成
    pub fn with_timeout(
        base_url: &str,
        model: &str,
        connect_timeout_secs: u64,
        read_timeout_secs: u64,
    ) -> Self {
        let client = Self::build_client(connect_timeout_secs, read_timeout_secs);

        Self {
            client,
            base_url: base_url.to_string(),
            model: model.to_string(),
            retry_config: RetryConfig::default(),
        }
    }

    /// OllamaConfigからクライアントを作成
    pub fn from_config(config: &OllamaConfig) -> Self {
        let client = Self::build_client(config.connect_timeout, config.read_timeout);

        Self {
            client,
            base_url: config.url.clone(),
            model: config.model.clone(),
            retry_config: config.retry.clone(),
        }
    }

    /// リトライ設定を更新
    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    /// モデル名を更新
    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    /// バックオフ時間を計算（エクスポネンシャルバックオフ）
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let backoff_ms = (self.retry_config.initial_backoff_ms as f64)
            * self.retry_config.backoff_multiplier.powi(attempt as i32);
        let backoff_ms = backoff_ms.min(self.retry_config.max_backoff_ms as f64) as u64;
        Duration::from_millis(backoff_ms)
    }

    /// リトライ付きでリクエストを送信
    async fn send_with_retry<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, reqwest::Error>>,
    {
        let mut last_error: Option<reqwest::Error> = None;

        for attempt in 0..=self.retry_config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    let error_type = RetryableError::from_reqwest_error(&error);

                    if !error_type.is_retryable() || attempt >= self.retry_config.max_retries {
                        // リトライ不可またはリトライ回数超過
                        return Err(error).context(format!(
                            "リクエスト失敗 ({}): {}回のリトライ後",
                            error_type.description(),
                            attempt
                        ));
                    }

                    // バックオフを計算して待機
                    let backoff = self.calculate_backoff(attempt);
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries = self.retry_config.max_retries,
                        error_type = error_type.description(),
                        backoff_ms = backoff.as_millis() as u64,
                        "リトライ待機中..."
                    );

                    sleep(backoff).await;
                    last_error = Some(error);
                }
            }
        }

        // ここには到達しないはずだが、念のため
        Err(last_error
            .map(|e| anyhow::anyhow!(e))
            .unwrap_or_else(|| anyhow::anyhow!("不明なエラー")))
    }

    /// 生成リクエストを送信（リトライ付き）
    pub async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            system: system.map(|s| s.to_string()),
        };

        let url = format!("{}/api/generate", self.base_url);
        let client = self.client.clone();
        let request_json = serde_json::to_value(&request)?;

        let response: GenerateResponse = self
            .send_with_retry(|| {
                let client = client.clone();
                let url = url.clone();
                let request_json = request_json.clone();
                async move {
                    client
                        .post(&url)
                        .json(&request_json)
                        .send()
                        .await?
                        .json::<GenerateResponse>()
                        .await
                }
            })
            .await?;

        Ok(response.response)
    }

    /// 生成リクエストを送信（リトライなし - 後方互換性のため）
    pub async fn generate_no_retry(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            system: system.map(|s| s.to_string()),
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?
            .json::<GenerateResponse>()
            .await?;

        Ok(response.response)
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 現在のリトライ設定を取得
    pub fn retry_config(&self) -> &RetryConfig {
        &self.retry_config
    }

    /// 内部のreqwestクライアントを取得
    pub fn http_client(&self) -> &Client {
        &self.client
    }

    /// ストリーミング生成リクエストを送信
    ///
    /// レスポンスはトークン単位で受信でき、リアルタイム表示に使用可能
    pub async fn generate_streaming(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> Result<StreamingResponse> {
        streaming_impl(
            &self.client,
            &self.base_url,
            &self.model,
            prompt,
            system,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_error_classification() {
        // 接続エラーはリトライ可能
        assert!(RetryableError::Connection.is_retryable());
        // タイムアウトはリトライ可能
        assert!(RetryableError::Timeout.is_retryable());
        // サーバーエラーはリトライ可能
        assert!(RetryableError::ServerError.is_retryable());
        // 非リトライエラーはリトライ不可
        assert!(!RetryableError::NonRetryable.is_retryable());
    }

    #[test]
    fn test_calculate_backoff() {
        let client = OllamaClient::new("http://localhost:11434", "test");

        // デフォルト設定: 1000ms, 倍率2.0
        let backoff_0 = client.calculate_backoff(0);
        assert_eq!(backoff_0, Duration::from_millis(1000)); // 1秒

        let backoff_1 = client.calculate_backoff(1);
        assert_eq!(backoff_1, Duration::from_millis(2000)); // 2秒

        let backoff_2 = client.calculate_backoff(2);
        assert_eq!(backoff_2, Duration::from_millis(4000)); // 4秒
    }

    #[test]
    fn test_backoff_max_limit() {
        let mut client = OllamaClient::new("http://localhost:11434", "test");
        client.retry_config.max_backoff_ms = 5000;

        // 4回目のリトライ: 1000 * 2^4 = 16000ms だが、max 5000ms に制限
        let backoff = client.calculate_backoff(4);
        assert_eq!(backoff, Duration::from_millis(5000));
    }

    #[test]
    fn test_from_config() {
        let config = OllamaConfig {
            url: "http://custom:11434".to_string(),
            model: "custom-model".to_string(),
            timeout: 300,
            connect_timeout: 60,
            read_timeout: 600,
            retry: RetryConfig {
                max_retries: 5,
                initial_backoff_ms: 2000,
                backoff_multiplier: 1.5,
                max_backoff_ms: 30000,
            },
        };

        let client = OllamaClient::from_config(&config);
        assert_eq!(client.base_url(), "http://custom:11434");
        assert_eq!(client.model(), "custom-model");
        assert_eq!(client.retry_config().max_retries, 5);
        assert_eq!(client.retry_config().initial_backoff_ms, 2000);
    }
}
