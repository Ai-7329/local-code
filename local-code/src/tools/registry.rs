use std::collections::HashMap;
use std::sync::Arc;

use super::{Tool, ToolDefinition};

/// ツールレジストリ - ツールの登録と検索
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// 新しいレジストリを作成
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// ツールを登録
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// 名前でツールを取得
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// ツール名一覧を取得
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// 全ツール定義を取得
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// ツールが存在するかチェック
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// ツール数を取得
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 空かチェック
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 指定された名前のツールのみをフィルタリング
    pub fn filter_by_names(&self, allowed_names: &[&str]) -> Vec<Arc<dyn Tool>> {
        self.tools
            .values()
            .filter(|t| allowed_names.contains(&t.name()))
            .cloned()
            .collect()
    }

    /// LLMに送信するためのツール定義JSON
    pub fn to_prompt_format(&self) -> String {
        let mut output = String::from("Available tools:\n\n");

        for tool in self.tools.values() {
            output.push_str(&format!("## {}\n", tool.name()));
            output.push_str(&format!("{}\n", tool.description()));
            output.push_str(&format!("Parameters: {}\n\n",
                serde_json::to_string_pretty(&tool.parameters_schema()).unwrap_or_default()
            ));
        }

        output
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
    }
}
