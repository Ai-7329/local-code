use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::AsyncReadExt;

use crate::tools::{Tool, ToolResult};

/// Bashコマンド実行ツール
pub struct BashTool {
    /// タイムアウト（秒）
    timeout_secs: u64,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            timeout_secs: 120,
        }
    }

    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for the command"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 120)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let command = params.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing command parameter"))?;

        let working_dir = params.get("working_dir")
            .and_then(|v| v.as_str());

        let timeout_secs = params.get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.timeout_secs);

        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            async {
                let mut child = cmd.spawn()?;

                let mut stdout = String::new();
                let mut stderr = String::new();

                if let Some(mut out) = child.stdout.take() {
                    out.read_to_string(&mut stdout).await?;
                }
                if let Some(mut err) = child.stderr.take() {
                    err.read_to_string(&mut stderr).await?;
                }

                let status = child.wait().await?;
                Ok::<_, anyhow::Error>((status, stdout, stderr))
            }
        ).await;

        match result {
            Ok(Ok((status, stdout, stderr))) => {
                let mut output = String::new();
                if !stdout.is_empty() {
                    output.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str("[stderr]\n");
                    output.push_str(&stderr);
                }

                if status.success() {
                    Ok(ToolResult::success(output))
                } else {
                    Ok(ToolResult::failure(format!(
                        "Command exited with code {}\n{}",
                        status.code().unwrap_or(-1),
                        output
                    )))
                }
            }
            Ok(Err(e)) => Ok(ToolResult::failure(format!("Failed to execute command: {}", e))),
            Err(_) => Ok(ToolResult::failure(format!("Command timed out after {} seconds", timeout_secs))),
        }
    }
}
