use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::AsyncReadExt;

use crate::tools::{Tool, ToolResult};

/// Git コマンド実行ヘルパー
async fn run_git_command(args: &[&str], working_dir: Option<&str>) -> Result<(bool, String)> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

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

    let output = if stderr.is_empty() {
        stdout
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    Ok((status.success(), output.trim().to_string()))
}

/// Git status ツール
pub struct GitStatusTool;

impl GitStatusTool {
    pub fn new() -> Self { Self }
}

impl Default for GitStatusTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str { "git_status" }
    fn description(&self) -> &str { "Show the working tree status" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Repository path" }
            }
        })
    }
    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let path = params.get("path").and_then(|v| v.as_str());
        let (success, output) = run_git_command(&["status", "--short"], path).await?;
        if success {
            Ok(ToolResult::success(if output.is_empty() { "Working tree clean".to_string() } else { output }))
        } else {
            Ok(ToolResult::failure(output))
        }
    }
}

/// Git diff ツール
pub struct GitDiffTool;

impl GitDiffTool {
    pub fn new() -> Self { Self }
}

impl Default for GitDiffTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str { "git_diff" }
    fn description(&self) -> &str { "Show changes between commits, commit and working tree, etc" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Repository path" },
                "staged": { "type": "boolean", "description": "Show staged changes" },
                "file": { "type": "string", "description": "Specific file to diff" }
            }
        })
    }
    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let path = params.get("path").and_then(|v| v.as_str());
        let staged = params.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
        let file = params.get("file").and_then(|v| v.as_str());

        let mut args = vec!["diff"];
        if staged { args.push("--staged"); }
        if let Some(f) = file { args.push(f); }

        let (success, output) = run_git_command(&args, path).await?;
        if success {
            Ok(ToolResult::success(if output.is_empty() { "No changes".to_string() } else { output }))
        } else {
            Ok(ToolResult::failure(output))
        }
    }
}

/// Git add ツール
pub struct GitAddTool;

impl GitAddTool {
    pub fn new() -> Self { Self }
}

impl Default for GitAddTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for GitAddTool {
    fn name(&self) -> &str { "git_add" }
    fn description(&self) -> &str { "Add file contents to the staging area" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Repository path" },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Files to add"
                }
            },
            "required": ["files"]
        })
    }
    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let path = params.get("path").and_then(|v| v.as_str());
        let files = params.get("files")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing files parameter"))?;

        let file_strs: Vec<&str> = files.iter()
            .filter_map(|v| v.as_str())
            .collect();

        let mut args = vec!["add"];
        args.extend(file_strs.iter());

        let (success, output) = run_git_command(&args, path).await?;
        if success {
            Ok(ToolResult::success(format!("Added {} file(s)", file_strs.len())))
        } else {
            Ok(ToolResult::failure(output))
        }
    }
}

/// Git commit ツール
pub struct GitCommitTool;

impl GitCommitTool {
    pub fn new() -> Self { Self }
}

impl Default for GitCommitTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str { "git_commit" }
    fn description(&self) -> &str { "Record changes to the repository" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Repository path" },
                "message": { "type": "string", "description": "Commit message" }
            },
            "required": ["message"]
        })
    }
    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let path = params.get("path").and_then(|v| v.as_str());
        let message = params.get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing message parameter"))?;

        let (success, output) = run_git_command(&["commit", "-m", message], path).await?;
        if success {
            Ok(ToolResult::success(output))
        } else {
            Ok(ToolResult::failure(output))
        }
    }
}

/// Git log ツール
pub struct GitLogTool;

impl GitLogTool {
    pub fn new() -> Self { Self }
}

impl Default for GitLogTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str { "git_log" }
    fn description(&self) -> &str { "Show commit logs" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Repository path" },
                "count": { "type": "integer", "description": "Number of commits to show (default: 10)" },
                "oneline": { "type": "boolean", "description": "Show one line per commit" }
            }
        })
    }
    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let path = params.get("path").and_then(|v| v.as_str());
        let count = params.get("count").and_then(|v| v.as_u64()).unwrap_or(10);
        let oneline = params.get("oneline").and_then(|v| v.as_bool()).unwrap_or(true);

        let count_str = format!("-{}", count);
        let mut args = vec!["log", &count_str];
        if oneline { args.push("--oneline"); }

        let (success, output) = run_git_command(&args, path).await?;
        if success {
            Ok(ToolResult::success(if output.is_empty() { "No commits".to_string() } else { output }))
        } else {
            Ok(ToolResult::failure(output))
        }
    }
}
