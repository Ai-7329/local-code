//! 埋め込みSuperpowersアセット
//!
//! ビルド時にsuperpowersディレクトリをバイナリに埋め込み、
//! どこからでも利用可能にする

use rust_embed::Embed;

/// 埋め込みSuperpowersアセット
#[derive(Embed)]
#[folder = "superpowers/"]
#[prefix = ""]
pub struct EmbeddedSuperpowers;

impl EmbeddedSuperpowers {
    /// スキルファイル一覧を取得
    pub fn skill_files() -> Vec<String> {
        Self::iter()
            .filter(|path| path.starts_with("skills/") && path.ends_with("/SKILL.md"))
            .map(|s| s.to_string())
            .collect()
    }

    /// コマンドファイル一覧を取得
    pub fn command_files() -> Vec<String> {
        Self::iter()
            .filter(|path| path.starts_with("commands/") && path.ends_with(".md"))
            .map(|s| s.to_string())
            .collect()
    }

    /// ブートストラップファイルを取得
    pub fn bootstrap() -> Option<String> {
        // ローカル版を優先
        Self::get("superpowers-bootstrap.local.md")
            .or_else(|| Self::get("superpowers-bootstrap.md"))
            .map(|f| String::from_utf8_lossy(&f.data).to_string())
    }

    /// ファイル内容を取得
    pub fn get_content(path: &str) -> Option<String> {
        Self::get(path).map(|f| String::from_utf8_lossy(&f.data).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_files_exist() {
        let skills = EmbeddedSuperpowers::skill_files();
        assert!(!skills.is_empty(), "Embedded skills should exist");
        
        let commands = EmbeddedSuperpowers::command_files();
        assert!(!commands.is_empty(), "Embedded commands should exist");
    }
}
