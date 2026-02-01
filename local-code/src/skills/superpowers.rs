use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;

use super::embedded::EmbeddedSuperpowers;

#[derive(Debug, Clone)]
pub struct SuperpowersCommand {
    pub name: String,
    pub skill: String,
    pub path: PathBuf,
}

/// 埋め込みSuperpowersコマンドを読み込み
pub fn load_embedded_commands() -> Vec<SuperpowersCommand> {
    let mut commands = Vec::new();
    let re = match Regex::new(r"superpowers:([a-zA-Z0-9\-]+)") {
        Ok(r) => r,
        Err(_) => return commands,
    };

    for path in EmbeddedSuperpowers::command_files() {
        if let Some(content) = EmbeddedSuperpowers::get_content(&path) {
            // ファイル名からコマンド名を取得
            let name = Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }

            if let Some(captures) = re.captures(&content) {
                let skill = format!("superpowers:{}", &captures[1]);
                commands.push(SuperpowersCommand {
                    name,
                    skill,
                    path: PathBuf::from(format!("embedded://{}", path)),
                });
            }
        }
    }

    commands
}

/// Superpowersコマンド定義を読み込み（埋め込み + ファイルシステム）
pub async fn load_superpowers_commands(commands_dir: &Path) -> Result<Vec<SuperpowersCommand>> {
    // 1. 埋め込みコマンドを最初にロード
    let mut commands = load_embedded_commands();
    let embedded_names: std::collections::HashSet<_> = commands.iter().map(|c| c.name.clone()).collect();

    // 2. ファイルシステムからコマンドをロード（オーバーライド可能）
    if !commands_dir.exists() {
        return Ok(commands);
    }

    let mut entries = fs::read_dir(commands_dir).await?;
    let re = Regex::new(r"superpowers:([a-zA-Z0-9\-]+)")?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }

        let content = fs::read_to_string(&path).await?;
        if let Some(captures) = re.captures(&content) {
            let skill = format!("superpowers:{}", &captures[1]);

            // ファイルシステム版で埋め込み版をオーバーライド
            if embedded_names.contains(&name) {
                if let Some(idx) = commands.iter().position(|c| c.name == name) {
                    commands[idx] = SuperpowersCommand { name, skill, path };
                }
            } else {
                commands.push(SuperpowersCommand { name, skill, path });
            }
        } else {
            tracing::warn!("Superpowers command missing skill reference: {}", path.display());
        }
    }

    Ok(commands)
}
