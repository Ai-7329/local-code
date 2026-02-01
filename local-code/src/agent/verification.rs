//! コード検証エンジン
//!
//! 生成されたコードを実行して検証し、エラーがあれば修正を促す

use anyhow::Result;
use std::process::Command;
use std::io::Write;
use tempfile::NamedTempFile;
use std::time::Duration;
use std::process::Stdio;
use tokio::time::timeout;
use tokio::process::Command as TokioCommand;

/// 検証結果
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// 成功したか
    pub success: bool,
    /// 出力（stdout）
    pub output: String,
    /// エラー出力（stderr）
    pub error: String,
    /// 言語
    pub language: String,
    /// 元のコード
    pub code: String,
}

/// コード検証エンジン
pub struct CodeVerifier {
    /// 最大試行回数
    max_attempts: usize,
}

const EXECUTION_TIMEOUT: Duration = Duration::from_secs(10);

impl CodeVerifier {
    pub fn new() -> Self {
        Self { max_attempts: 3 }
    }

    /// コードブロックを検出して検証
    pub fn extract_code_blocks(content: &str) -> Vec<(String, String)> {
        let mut blocks = Vec::new();
        let mut in_block = false;
        let mut current_lang = String::new();
        let mut current_code = Vec::new();

        for line in content.lines() {
            if line.trim().starts_with("```") {
                if in_block {
                    // ブロック終了
                    let lang = if current_lang.is_empty() {
                        Self::infer_language(&current_code.join("\n")).unwrap_or_default()
                    } else {
                        current_lang.clone()
                    };
                    blocks.push((lang, current_code.join("\n")));
                    current_code.clear();
                    in_block = false;
                } else {
                    // ブロック開始
                    current_lang = line.trim()[3..].trim().to_string();
                    in_block = true;
                }
            } else if in_block {
                current_code.push(line);
            }
        }

        blocks
    }

    /// 言語を正規化
    fn normalize_language(lang: &str) -> &str {
        match lang.to_lowercase().as_str() {
            "python" | "py" | "python3" => "python",
            "rust" | "rs" => "rust",
            "javascript" | "js" | "node" => "javascript",
            "typescript" | "ts" => "typescript",
            "bash" | "sh" | "shell" => "bash",
            _ => lang,
        }
    }

    /// コードから言語を推論
    pub fn infer_language(code: &str) -> Option<String> {
        let first_lines: String = code.lines().take(5).collect::<Vec<_>>().join("\n");

        if first_lines.contains("def ") || first_lines.contains("import ") || first_lines.contains("print(") {
            Some("python".to_string())
        } else if first_lines.contains("fn ") || first_lines.contains("let ") || first_lines.contains("use ") {
            Some("rust".to_string())
        } else if first_lines.contains("function ") || first_lines.contains("const ") || first_lines.contains("=>") {
            Some("javascript".to_string())
        } else if first_lines.starts_with("#!/bin/bash") || first_lines.starts_with("#!/bin/sh") {
            Some("bash".to_string())
        } else {
            None
        }
    }

    /// コードを実行して検証
    pub fn verify(&self, language: &str, code: &str) -> Result<VerificationResult> {
        let lang = Self::normalize_language(language);

        match lang {
            "python" => self.verify_python(code),
            "rust" => self.verify_rust(code),
            "javascript" => self.verify_javascript(code),
            "bash" => self.verify_bash(code),
            _ => Ok(VerificationResult {
                success: true,
                output: format!("Verification not supported for language: {}", language),
                error: String::new(),
                language: language.to_string(),
                code: code.to_string(),
            }),
        }
    }

    /// Python コードを検証
    fn verify_python(&self, code: &str) -> Result<VerificationResult> {
        let mut temp_file = NamedTempFile::with_suffix(".py")?;
        temp_file.write_all(code.as_bytes())?;
        temp_file.flush()?;

        let output = Command::new("python3")
            .arg(temp_file.path())
            .output()?;

        Ok(VerificationResult {
            success: output.status.success(),
            output: String::from_utf8_lossy(&output.stdout).to_string(),
            error: String::from_utf8_lossy(&output.stderr).to_string(),
            language: "python".to_string(),
            code: code.to_string(),
        })
    }

    /// Python コードを検証（非同期、タイムアウト付き）
    pub async fn verify_python_async(&self, code: &str) -> Result<VerificationResult> {
        let mut temp_file = NamedTempFile::with_suffix(".py")?;
        temp_file.write_all(code.as_bytes())?;
        temp_file.flush()?;

        let path = temp_file.path().to_path_buf();

        let result = timeout(EXECUTION_TIMEOUT, async {
            TokioCommand::new("python3")
                .arg(&path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
        }).await;

        match result {
            Ok(Ok(output)) => Ok(VerificationResult {
                success: output.status.success(),
                output: String::from_utf8_lossy(&output.stdout).to_string(),
                error: String::from_utf8_lossy(&output.stderr).to_string(),
                language: "python".to_string(),
                code: code.to_string(),
            }),
            Ok(Err(e)) => Err(anyhow::anyhow!("Execution error: {}", e)),
            Err(_) => Ok(VerificationResult {
                success: false,
                output: String::new(),
                error: "Execution timed out after 10 seconds".to_string(),
                language: "python".to_string(),
                code: code.to_string(),
            }),
        }
    }

    /// コードを非同期で検証（タイムアウト付き）
    pub async fn verify_async(&self, language: &str, code: &str) -> Result<VerificationResult> {
        let lang = Self::normalize_language(language);

        match lang {
            "python" => self.verify_python_async(code).await,
            // 他の言語は同期版にフォールバック
            _ => self.verify(language, code),
        }
    }

    /// Rust コードを検証（コンパイルのみ）
    fn verify_rust(&self, code: &str) -> Result<VerificationResult> {
        let mut temp_file = NamedTempFile::with_suffix(".rs")?;
        temp_file.write_all(code.as_bytes())?;
        temp_file.flush()?;

        // rustc でコンパイルチェックのみ
        let output = Command::new("rustc")
            .arg("--emit=metadata")
            .arg("-o")
            .arg("/dev/null")
            .arg(temp_file.path())
            .output()?;

        Ok(VerificationResult {
            success: output.status.success(),
            output: String::from_utf8_lossy(&output.stdout).to_string(),
            error: String::from_utf8_lossy(&output.stderr).to_string(),
            language: "rust".to_string(),
            code: code.to_string(),
        })
    }

    /// JavaScript コードを検証（構文チェック）
    fn verify_javascript(&self, code: &str) -> Result<VerificationResult> {
        let mut temp_file = NamedTempFile::with_suffix(".js")?;
        temp_file.write_all(code.as_bytes())?;
        temp_file.flush()?;

        let output = Command::new("node")
            .arg("--check")
            .arg(temp_file.path())
            .output()?;

        Ok(VerificationResult {
            success: output.status.success(),
            output: String::from_utf8_lossy(&output.stdout).to_string(),
            error: String::from_utf8_lossy(&output.stderr).to_string(),
            language: "javascript".to_string(),
            code: code.to_string(),
        })
    }

    /// Bash コードを検証（構文チェック）
    fn verify_bash(&self, code: &str) -> Result<VerificationResult> {
        let mut temp_file = NamedTempFile::with_suffix(".sh")?;
        temp_file.write_all(code.as_bytes())?;
        temp_file.flush()?;

        let output = Command::new("bash")
            .arg("-n")
            .arg(temp_file.path())
            .output()?;

        Ok(VerificationResult {
            success: output.status.success(),
            output: String::from_utf8_lossy(&output.stdout).to_string(),
            error: String::from_utf8_lossy(&output.stderr).to_string(),
            language: "bash".to_string(),
            code: code.to_string(),
        })
    }

    /// 修正プロンプトを生成
    pub fn create_fix_prompt(&self, result: &VerificationResult) -> String {
        format!(
            r#"The following {} code has an error. Please fix it.

**Original Code:**
```{}
{}
```

**Error:**
```
{}
```

Please provide the corrected code. Only output the fixed code block, no explanation."#,
            result.language, result.language, result.code, result.error
        )
    }

    /// 最大試行回数を取得
    pub fn max_attempts(&self) -> usize {
        self.max_attempts
    }
}

impl Default for CodeVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_code_blocks() {
        let content = r#"
Here is some code:
```python
def hello():
    print("Hello")
```
And more text.
"#;
        let blocks = CodeVerifier::extract_code_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "python");
        assert!(blocks[0].1.contains("def hello()"));
    }

    #[test]
    fn test_normalize_language() {
        assert_eq!(CodeVerifier::normalize_language("py"), "python");
        assert_eq!(CodeVerifier::normalize_language("Python"), "python");
        assert_eq!(CodeVerifier::normalize_language("rs"), "rust");
        assert_eq!(CodeVerifier::normalize_language("js"), "javascript");
    }

    #[test]
    fn test_infer_language() {
        assert_eq!(CodeVerifier::infer_language("def foo(): pass"), Some("python".to_string()));
        assert_eq!(CodeVerifier::infer_language("fn main() {}"), Some("rust".to_string()));
        assert_eq!(CodeVerifier::infer_language("const x = 1;"), Some("javascript".to_string()));
        assert_eq!(CodeVerifier::infer_language("#!/bin/bash\necho hi"), Some("bash".to_string()));
        assert_eq!(CodeVerifier::infer_language("some random text"), None);
    }

    #[test]
    fn test_extract_code_blocks_without_lang_tag() {
        let content = "```\ndef hello():\n    print('hi')\n```";
        let blocks = CodeVerifier::extract_code_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "python");
    }
}
