use anyhow::{Result, anyhow};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// ツール呼び出しリクエスト
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// ツール名
    pub tool: String,
    /// パラメータ
    pub params: Value,
}

/// LLMレスポンスからツール呼び出しを抽出
pub struct ToolCallParser;

impl ToolCallParser {
    /// レスポンステキストからツール呼び出しを抽出
    pub fn parse(response: &str) -> Result<Vec<ToolCall>> {
        let mut tool_calls = Vec::new();

        // ```json ... ``` ブロックを抽出
        let json_blocks = Self::extract_json_blocks(response);

        for block in json_blocks {
            if let Ok(call) = Self::parse_tool_call(&block) {
                tool_calls.push(call);
            }
        }

        Ok(tool_calls)
    }

    /// 最初のツール呼び出しのみを取得
    pub fn parse_first(response: &str) -> Result<Option<ToolCall>> {
        let calls = Self::parse(response)?;
        Ok(calls.into_iter().next())
    }

    /// JSONブロックを抽出
    fn extract_json_blocks(text: &str) -> Vec<String> {
        let re = Regex::new(r"```(?:json)?\s*\n?([\s\S]*?)```").unwrap();
        let mut blocks = Vec::new();

        for cap in re.captures_iter(text) {
            if let Some(content) = cap.get(1) {
                blocks.push(content.as_str().trim().to_string());
            }
        }

        // ```なしの生JSONも検出
        if blocks.is_empty() {
            if let Some(json) = Self::find_raw_json(text) {
                blocks.push(json);
            }
        }

        blocks
    }

    /// 生のJSONオブジェクトを検出
    fn find_raw_json(text: &str) -> Option<String> {
        let text = text.trim();

        // { で始まり } で終わる部分を探す
        if let Some(start) = text.find('{') {
            let mut depth = 0;
            let chars: Vec<char> = text.chars().collect();

            for (i, &c) in chars.iter().enumerate().skip(start) {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(text[start..=i].to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// JSONをToolCallにパース
    fn parse_tool_call(json_str: &str) -> Result<ToolCall> {
        let value: Value = serde_json::from_str(json_str)?;

        let tool = value.get("tool")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'tool' field"))?
            .to_string();

        let params = value.get("params")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        Ok(ToolCall { tool, params })
    }

    /// レスポンスにツール呼び出しが含まれるかチェック
    pub fn has_tool_call(response: &str) -> bool {
        let re = Regex::new(r#"\{\s*"tool"\s*:"#).unwrap();
        re.is_match(response)
    }

    /// ツール呼び出し部分とテキスト部分を分離
    pub fn split_response(response: &str) -> (String, Vec<ToolCall>) {
        let re = Regex::new(r"```(?:json)?\s*\n?[\s\S]*?```").unwrap();
        let text_only = re.replace_all(response, "").trim().to_string();
        let tool_calls = Self::parse(response).unwrap_or_default();
        (text_only, tool_calls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_block() {
        let response = r#"
I'll read the file for you.

```json
{"tool": "read", "params": {"file_path": "/path/to/file.rs"}}
```
"#;
        let calls = ToolCallParser::parse(response).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read");
    }

    #[test]
    fn test_parse_multiple_calls() {
        let response = r#"
```json
{"tool": "glob", "params": {"pattern": "*.rs"}}
```

Let me also check this:

```json
{"tool": "read", "params": {"file_path": "/src/main.rs"}}
```
"#;
        let calls = ToolCallParser::parse(response).unwrap();
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn test_has_tool_call() {
        assert!(ToolCallParser::has_tool_call(r#"{"tool": "read"}"#));
        assert!(!ToolCallParser::has_tool_call("Just a regular message"));
    }
}
