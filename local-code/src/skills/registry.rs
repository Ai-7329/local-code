use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

use super::loader::Skill;
use super::embedded::EmbeddedSuperpowers;

/// スキルレジストリ - スキルの探索と管理
pub struct SkillRegistry {
    /// 登録されたスキル（名前 -> スキル）
    skills: HashMap<String, Skill>,
    /// Superpowersスキル（名前 -> スキル）
    superpowers_skills: HashMap<String, Skill>,
    /// スキル探索パス
    search_paths: Vec<SkillSearchPath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillSource {
    User,
    Superpowers,
}

#[derive(Debug, Clone)]
struct SkillSearchPath {
    path: PathBuf,
    source: SkillSource,
}

impl SkillRegistry {
    /// 新しいレジストリを作成
    pub fn new() -> Self {
        let mut search_paths = Vec::new();

        // ~/.claude/skills/
        if let Some(home) = dirs::home_dir() {
            search_paths.push(SkillSearchPath {
                path: home.join(".claude").join("skills"),
                source: SkillSource::User,
            });
            search_paths.push(SkillSearchPath {
                path: home.join(".claude").join("plugins").join("cache"),
                source: SkillSource::User,
            });
        }

        Self {
            skills: HashMap::new(),
            superpowers_skills: HashMap::new(),
            search_paths,
        }
    }

    /// カスタム探索パスを追加
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(SkillSearchPath {
            path,
            source: SkillSource::User,
        });
    }

    /// Superpowers探索パスを追加
    pub fn add_superpowers_path(&mut self, path: PathBuf) {
        self.search_paths.push(SkillSearchPath {
            path,
            source: SkillSource::Superpowers,
        });
    }

    /// 全探索パスからスキルを読み込み
    pub async fn load_all(&mut self) -> Result<()> {
        // 1. 埋め込みSuperpowersスキルを最初にロード
        self.load_embedded_skills();

        // 2. ファイルシステムからスキルをロード（オーバーライド可能）
        for entry in &self.search_paths.clone() {
            if entry.path.exists() {
                self.load_from_directory(&entry.path, entry.source).await?;
            }
        }
        Ok(())
    }

    /// 埋め込みスキルを読み込み
    fn load_embedded_skills(&mut self) {
        for path in EmbeddedSuperpowers::skill_files() {
            if let Some(content) = EmbeddedSuperpowers::get_content(&path) {
                match Skill::load_from_string(&content, &format!("embedded://{}", path)) {
                    Ok(skill) => {
                        tracing::debug!("Loaded embedded skill: {}", skill.metadata.name);
                        self.insert_skill(skill, SkillSource::Superpowers);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse embedded skill {}: {}", path, e);
                    }
                }
            }
        }
    }

    /// 指定ディレクトリからスキルを読み込み
    fn load_from_directory<'a>(
        &'a mut self,
        dir: &'a Path,
        source: SkillSource,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = fs::read_dir(dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    // ディレクトリの場合、SKILL.mdを探す
                    let skill_file = path.join("SKILL.md");
                    if skill_file.exists() {
                        if let Ok(skill) = Skill::load_from_file(&skill_file).await {
                            tracing::info!("Loaded skill: {} from {}", skill.metadata.name, skill_file.display());
                            self.insert_skill(skill, source);
                        }
                    }

                    // プラグインキャッシュの場合はさらに深くスキャン
                    if path.to_string_lossy().contains("plugins/cache") {
                        self.scan_plugin_directory(&path, source).await?;
                    }
                }
            }

            Ok(())
        })
    }

    /// プラグインディレクトリをスキャン（バージョンディレクトリ内のskills/）
    fn scan_plugin_directory<'a>(
        &'a mut self,
        plugin_dir: &'a Path,
        source: SkillSource,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = fs::read_dir(plugin_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let version_dir = entry.path();
                if version_dir.is_dir() {
                    let skills_dir = version_dir.join("skills");
                    if skills_dir.exists() {
                        self.load_from_directory(&skills_dir, source).await?;
                    }
                }
            }

            Ok(())
        })
    }

    /// 名前でスキルを取得
    pub fn get(&self, name: &str) -> Option<&Skill> {
        if let Some(stripped) = name.strip_prefix("superpowers:") {
            return self.superpowers_skills.get(stripped);
        }

        self.skills
            .get(name)
            .or_else(|| self.superpowers_skills.get(name))
    }

    /// 全スキルのリストを取得
    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// スキル名一覧を取得
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.skills.keys().cloned().collect();
        for name in self.superpowers_skills.keys() {
            names.push(format!("superpowers:{}", name));
        }
        names.sort();
        names.dedup();
        names
    }

    /// トリガーにマッチするスキルを検索
    pub fn find_by_trigger(&self, input: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|skill| skill.matches_trigger(input))
            .collect()
    }

    /// スキル数を取得
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// スキルが空かチェック
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    fn insert_skill(&mut self, skill: Skill, source: SkillSource) {
        let name = skill.metadata.name.clone();
        match source {
            SkillSource::Superpowers => {
                self.superpowers_skills.insert(name.clone(), skill.clone());
                self.skills.entry(name).or_insert(skill);
            }
            SkillSource::User => {
                self.skills.insert(name, skill);
            }
        }
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_creation() {
        let registry = SkillRegistry::new();
        assert!(registry.is_empty());
    }
}
