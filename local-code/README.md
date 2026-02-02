# local-code

OLLAMA連携コーディングエージェント - Claude Codeライクなプランモード/実行モード切り替え、Skills、agent.md参照、LSP連携を実装。

## 特徴

- **モードシステム**: Plan モード（読み取り専用）と Execute モード（全ツール利用可能）
- **ツールシステム**: ファイル操作、検索、Git、Bash、LSP連携
- **スキルシステム**: Claude Code互換のSKILL.md形式をサポート
- **コンテキスト**: プロジェクトのagent.md/CLAUDE.mdを自動読み込み

## インストール

```bash
# ビルド
cargo build --release

# インストール（オプション）
cargo install --path .
```

## 使い方

```bash
# 基本的な起動
local-code

# オプション付き
local-code --ollama-url http://localhost:11434 --model Rnj-1 --mode plan
```

## コマンド

| コマンド | 説明 |
|---------|------|
| `/help` | ヘルプを表示 |
| `/quit` | 終了 |
| `/plan` | Planモードに切り替え（読み取り専用） |
| `/execute` | Executeモードに切り替え（全ツール利用可能） |
| `/status` | 現在の状態を表示 |
| `/skills` | 利用可能なスキル一覧 |
| `/clear` | 画面をクリア |
| `/<skill-name>` | スキルを実行 |
| `/brainstorm` | superpowers:brainstorming を実行 |
| `/execute-plan` | superpowers:executing-plans を実行 |
| `/write-plan` | superpowers:writing-plans を実行 |

## ツール一覧

### ファイル操作
- `read` - ファイル読み込み
- `write` - ファイル書き込み
- `edit` - 部分編集（old_string → new_string）

### 検索
- `glob` - ファイルパターン検索
- `grep` - 内容検索

### 実行
- `bash` - Bashコマンド実行

### Git
- `git_status` - ステータス表示
- `git_diff` - 差分表示
- `git_add` - ステージング
- `git_commit` - コミット
- `git_log` - ログ表示

### LSP
- `lsp_definition` - 定義ジャンプ
- `lsp_references` - 参照検索
- `lsp_diagnostics` - 診断情報

## スキル

スキルは `~/.claude/skills/` または `~/.claude/plugins/cache/` から読み込まれます。
Superpowers同梱時は `superpowers/skills` も自動読み込みされます（`LOCAL_CODE_SUPERPOWERS`でパス指定可）。

### スキルの形式

```markdown
---
name: my-skill
description: My custom skill
triggers:
  - keyword1
  - keyword2
auto: false
---

# My Skill

スキルの内容...
```

## 設定

設定ファイル: `config/default.toml`

```toml
[ollama]
url = "http://localhost:11434"
model = "Rnj-1"

[agent]
initial_mode = "execute"

[tools]
bash_timeout = 120

[skills]
# custom_path = "/path/to/skills"

[lsp]
# command = "rust-analyzer"
# args = []
```

## プロジェクトコンテキスト

プロジェクトルートに以下のファイルがあれば自動的に読み込まれます:
- `agent.md`
- `AGENT.md`
- `CLAUDE.md`
- `claude.md`

## 依存関係

- Rust 1.70+
- OLLAMA（ローカルで起動）

## 更新履歴

### 2026-02-02
- **キー二重入力バグの修正**: キーを1回押すと2文字入力される問題を修正
  - crossterm を 0.27 から 0.28 にアップグレード
  - `KeyEventKind::Press` イベントのみを処理するようにフィルタリングを追加
  - 一部のターミナルで Press/Release 両方のイベントが送信される問題に対応

## ライセンス

MIT
