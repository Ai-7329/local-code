use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;

use super::loader::Skill;
use super::registry::SkillRegistry;
use super::embedded::EmbeddedSuperpowers;

/// スキル実行コンテキスト
pub struct SkillContext {
    /// スキルに渡された引数
    pub args: Option<String>,
    /// 現在の作業ディレクトリ
    pub working_dir: std::path::PathBuf,
}

impl SkillContext {
    /// 新しいスキルコンテキストを作成
    pub fn new(args: Option<String>) -> Self {
        Self {
            args,
            working_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        }
    }
}

/// スキル実行器
pub struct SkillExecutor {
    registry: Arc<SkillRegistry>,
}

impl SkillExecutor {
    /// Arc<SkillRegistry>から新しいSkillExecutorを作成
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }

    /// スキル名から実行し、プロンプトを生成
    pub async fn execute_by_name(&self, name: &str, context: &SkillContext) -> Result<String> {
        let skill = self.registry.get(name)
            .ok_or_else(|| anyhow::anyhow!("Skill not found: {}", name))?;

        self.execute(skill, context).await
    }

    /// スキルを実行し、プロンプトを生成
    pub async fn execute(&self, skill: &Skill, context: &SkillContext) -> Result<String> {
        let mut prompt = String::new();

        // 親スキルがあれば先に読み込み
        if let Some(parent_name) = &skill.metadata.parent {
            if let Some(parent) = self.registry.get(parent_name) {
                prompt.push_str(&parent.content);
                prompt.push_str("\n\n---\n\n");
            }
        }

        // スキル本文を追加
        prompt.push_str(&skill.content);

        // 子スキル（doc.md等）を探索して追加
        let child_docs = self.find_child_docs(&skill.path).await?;
        for doc in child_docs {
            prompt.push_str("\n\n---\n\n");
            prompt.push_str(&doc);
        }

        // 引数があれば追加
        if let Some(args) = &context.args {
            prompt.push_str("\n\n---\n\n");
            prompt.push_str(&format!("User input: {}", args));
        }

        Ok(prompt)
    }

    /// 子スキル（同じディレクトリ内のdoc.md等）を探索
    async fn find_child_docs(&self, skill_path: &Path) -> Result<Vec<String>> {
        let path_str = skill_path.to_string_lossy();

        // 埋め込みリソースの場合
        if path_str.starts_with("embedded://") {
            return Ok(self.find_embedded_child_docs(&path_str));
        }

        // ファイルシステムの場合
        let mut docs = Vec::new();

        if let Some(parent_dir) = skill_path.parent() {
            if !parent_dir.exists() {
                return Ok(docs);
            }

            let mut entries = fs::read_dir(parent_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");

                    // SKILL.md以外のmdファイルを読み込み
                    if filename.ends_with(".md") && filename != "SKILL.md" {
                        if let Ok(content) = fs::read_to_string(&path).await {
                            docs.push(content);
                        }
                    }
                }
            }
        }

        Ok(docs)
    }

    /// 埋め込みリソースから子ドキュメントを探索
    fn find_embedded_child_docs(&self, embedded_path: &str) -> Vec<String> {
        let mut docs = Vec::new();

        // "embedded://skills/xxx/SKILL.md" から "skills/xxx/" を取得
        let path = embedded_path.strip_prefix("embedded://").unwrap_or(embedded_path);
        let parent_dir = Path::new(path).parent().map(|p| p.to_string_lossy().to_string());

        if let Some(dir) = parent_dir {
            // 埋め込みファイルを列挙して、同じディレクトリ内のmdファイルを探す
            for file_path in EmbeddedSuperpowers::iter() {
                let file_str = file_path.as_ref();
                if file_str.starts_with(&dir) && file_str.ends_with(".md") && !file_str.ends_with("SKILL.md") {
                    if let Some(content) = EmbeddedSuperpowers::get_content(file_str) {
                        docs.push(content);
                    }
                }
            }
        }

        docs
    }

    /// スキルをシステムプロンプト形式に変換
    pub fn to_system_prompt(&self, skill: &Skill) -> String {
        format!(
            "<skill name=\"{}\">\n{}\n</skill>",
            skill.metadata.name,
            skill.content
        )
    }
}

/// スキル実行結果
pub struct SkillResult {
    /// 生成されたプロンプト
    pub prompt: String,
    /// 使用されたスキル名
    pub skill_name: String,
    /// 子スキルが含まれるか
    pub has_children: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_context() {
        let ctx = SkillContext {
            args: Some("test args".to_string()),
            working_dir: std::path::PathBuf::from("/test"),
        };
        assert_eq!(ctx.args.as_deref(), Some("test args"));
    }
}
