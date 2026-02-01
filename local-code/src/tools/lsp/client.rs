use anyhow::Result;
use lsp_types::{
    InitializeParams, InitializeResult, InitializedParams,
    ClientCapabilities, Url, TextDocumentIdentifier,
    Position, GotoDefinitionParams, GotoDefinitionResponse,
    ReferenceParams, ReferenceContext, Location,
    TextDocumentPositionParams,
};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// LSPクライアント
pub struct LspClient {
    process: Mutex<Child>,
    request_id: Mutex<i64>,
    #[allow(dead_code)]
    pending_responses: Mutex<HashMap<i64, tokio::sync::oneshot::Sender<Value>>>,
}

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: i64,
    method: String,
    params: Value,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    id: Option<i64>,
    result: Option<Value>,
    error: Option<Value>,
}

impl LspClient {
    /// 言語サーバープロセスを起動してクライアントを作成
    pub async fn start(command: &str, args: &[&str]) -> Result<Self> {
        let process = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self {
            process: Mutex::new(process),
            request_id: Mutex::new(0),
            pending_responses: Mutex::new(HashMap::new()),
        })
    }

    /// LSPサーバーを初期化
    pub async fn initialize(&self, root_path: &Path) -> Result<InitializeResult> {
        let root_uri = Url::from_file_path(root_path)
            .map_err(|_| anyhow::anyhow!("Invalid path"))?;

        #[allow(deprecated)]
        let params = InitializeParams {
            root_uri: Some(root_uri),
            capabilities: ClientCapabilities::default(),
            ..Default::default()
        };

        let result: InitializeResult = self.request("initialize", serde_json::to_value(params)?).await?;

        // initialized通知を送信
        self.notify("initialized", serde_json::to_value(InitializedParams {})?).await?;

        Ok(result)
    }

    /// ドキュメントを開く（didOpen通知）
    pub async fn did_open(&self, file_path: &Path) -> Result<()> {
        let uri = Url::from_file_path(file_path)
            .map_err(|_| anyhow::anyhow!("Invalid path"))?;
        let text = fs::read_to_string(file_path).await?;
        let language_id = Self::language_id_for_path(file_path);

        let params = json!({
            "textDocument": {
                "uri": uri,
                "languageId": language_id,
                "version": 1,
                "text": text,
            }
        });

        self.notify("textDocument/didOpen", params).await
    }

    /// 定義ジャンプ
    pub async fn goto_definition(&self, file_path: &Path, line: u32, character: u32) -> Result<Option<GotoDefinitionResponse>> {
        let uri = Url::from_file_path(file_path)
            .map_err(|_| anyhow::anyhow!("Invalid path"))?;

        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        self.request("textDocument/definition", serde_json::to_value(params)?).await
    }

    /// 参照検索
    pub async fn find_references(&self, file_path: &Path, line: u32, character: u32) -> Result<Option<Vec<Location>>> {
        let uri = Url::from_file_path(file_path)
            .map_err(|_| anyhow::anyhow!("Invalid path"))?;

        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            context: ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        self.request("textDocument/references", serde_json::to_value(params)?).await
    }

    /// 診断情報を取得（pull diagnostics）
    pub async fn document_diagnostics(&self, file_path: &Path) -> Result<Value> {
        let uri = Url::from_file_path(file_path)
            .map_err(|_| anyhow::anyhow!("Invalid path"))?;

        let params = json!({
            "textDocument": { "uri": uri },
            "identifier": null,
            "previousResultId": null,
        });

        self.request("textDocument/diagnostic", params).await
    }

    /// LSPサーバーをシャットダウン
    pub async fn shutdown(&self) -> Result<()> {
        let _: Value = self.request("shutdown", Value::Null).await?;
        self.notify("exit", Value::Null).await?;
        Ok(())
    }

    async fn request<T: for<'de> Deserialize<'de>>(&self, method: &str, params: Value) -> Result<T> {
        let id = {
            let mut id_guard = self.request_id.lock().await;
            *id_guard += 1;
            *id_guard
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let content = serde_json::to_string(&request)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);

        {
            let mut process = self.process.lock().await;
            if let Some(stdin) = process.stdin.as_mut() {
                stdin.write_all(message.as_bytes()).await?;
                stdin.flush().await?;
            }
        }

        // レスポンス読み取り（簡略化版）
        let response = self.read_response().await?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("LSP error: {:?}", error));
        }

        match response.result {
            Some(result) => Ok(serde_json::from_value(result)?),
            None => Err(anyhow::anyhow!("No result in response")),
        }
    }

    async fn notify(&self, method: &str, params: Value) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let content = serde_json::to_string(&notification)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);

        let mut process = self.process.lock().await;
        if let Some(stdin) = process.stdin.as_mut() {
            stdin.write_all(message.as_bytes()).await?;
            stdin.flush().await?;
        }

        Ok(())
    }

    async fn read_response(&self) -> Result<JsonRpcResponse> {
        let mut process = self.process.lock().await;
        let stdout = process.stdout.as_mut().ok_or_else(|| anyhow::anyhow!("No stdout"))?;
        let mut reader = BufReader::new(stdout);

        // ヘッダー読み取り（ログ行は無視してContent-Lengthを待つ）
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                return Err(anyhow::anyhow!("LSP response EOF"));
            }

            let line = line.trim();
            if line.is_empty() {
                if content_length.is_some() {
                    break;
                }
                continue;
            }
            if let Some(length_str) = line.strip_prefix("Content-Length:") {
                content_length = Some(length_str.trim().parse()?);
            }
        }

        // ボディ読み取り
        let content_length = content_length.ok_or_else(|| anyhow::anyhow!("Missing Content-Length header"))?;
        let mut body = vec![0u8; content_length];
        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut body).await?;

        let response: JsonRpcResponse = serde_json::from_slice(&body)?;
        Ok(response)
    }

    fn language_id_for_path(path: &Path) -> &'static str {
        match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
            "rs" => "rust",
            "ts" => "typescript",
            "tsx" => "typescriptreact",
            "js" => "javascript",
            "jsx" => "javascriptreact",
            "py" => "python",
            "go" => "go",
            "java" => "java",
            "c" => "c",
            "cc" | "cpp" | "cxx" => "cpp",
            "h" | "hpp" => "cpp",
            "json" => "json",
            "toml" => "toml",
            "md" => "markdown",
            "yml" | "yaml" => "yaml",
            _ => "plaintext",
        }
    }
}
