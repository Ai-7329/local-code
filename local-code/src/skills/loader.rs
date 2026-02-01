use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

/// スキルのメタデータ（YAML frontmatter）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// スキル名
    pub name: String,
    /// 説明
    #[serde(default)]
    pub description: String,
    /// トリガーフレーズ
    #[serde(default)]
    pub triggers: Vec<String>,
    /// 自動実行するか
    #[serde(default)]
    pub auto: bool,
    /// 親スキル名（階層構造用）
    #[serde(default)]
    pub parent: Option<String>,
}

/// スキル定義
#[derive(Debug, Clone)]
pub struct Skill {
    /// メタデータ
    pub metadata: SkillMetadata,
    /// スキル本文（Markdown）
    pub content: String,
    /// ファイルパス
    pub path: std::path::PathBuf,
}

impl Skill {
    /// SKILL.mdファイルからスキルを読み込み
    pub async fn load_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).await?;
        Self::parse(&content, path.to_path_buf())
    }

    /// 文字列からスキルを読み込み（埋め込みリソース用）
    pub fn load_from_string(content: &str, virtual_path: &str) -> Result<Self> {
        Self::parse(content, std::path::PathBuf::from(virtual_path))
    }

    /// ファイル内容をパース
    fn parse(content: &str, path: std::path::PathBuf) -> Result<Self> {
        // YAML frontmatterを抽出
        let (metadata, body) = Self::extract_frontmatter(content)?;

        Ok(Self {
            metadata,
            content: body,
            path,
        })
    }

    /// frontmatter（---で囲まれた部分）を抽出
    fn extract_frontmatter(content: &str) -> Result<(SkillMetadata, String)> {
        let content = content.trim();

        if !content.starts_with("---") {
            // frontmatterがない場合はデフォルトメタデータ
            return Ok((
                SkillMetadata {
                    name: "unnamed".to_string(),
                    description: String::new(),
                    triggers: Vec::new(),
                    auto: false,
                    parent: None,
                },
                content.to_string(),
            ));
        }

        // 2つ目の---を探す
        let rest = &content[3..];
        if let Some(end_pos) = rest.find("---") {
            let yaml_content = &rest[..end_pos].trim();
            let body = &rest[end_pos + 3..].trim();

            let metadata: SkillMetadata = serde_yaml::from_str(yaml_content)?;
            Ok((metadata, body.to_string()))
        } else {
            Err(anyhow::anyhow!("Invalid frontmatter: missing closing ---"))
        }
    }

    /// トリガーフレーズにマッチするか確認
    pub fn matches_trigger(&self, input: &str) -> bool {
        let input_lower = input.to_lowercase();
        self.metadata.triggers.iter().any(|trigger| {
            input_lower.contains(&trigger.to_lowercase())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
triggers:
  - test
  - example
auto: true
---

# Test Skill

This is the skill content.
"#;

        let (metadata, body) = Skill::extract_frontmatter(content).unwrap();
        assert_eq!(metadata.name, "test-skill");
        assert_eq!(metadata.triggers, vec!["test", "example"]);
        assert!(metadata.auto);
        assert!(body.contains("# Test Skill"));
    }
}
