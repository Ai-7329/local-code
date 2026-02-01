# local-code ビルド/インストール/設定ガイド

## 前提
- Rust 1.70+（`cargo` が使えること）
- Git
- OLLAMA（`http://localhost:11434` で起動していること）
- LSPを使う場合は対応サーバー（Rustなら `rust-analyzer`）

## クイックスタート
```bash
cd local-code
cargo run
```

## ビルド
```bash
cd local-code
cargo build --release
```
- 出力（macOS/Linux）: `target/release/local-code`
- 出力（Windows）: `target\\release\\local-code.exe`

## インストール
### 方法A: cargo install
```bash
cd local-code
cargo install --path .
```

### 方法B: 実行ファイルをPATHへ配置
1. `target/release/local-code(.exe)` を任意のディレクトリへコピー  
2. そのディレクトリを PATH に追加  
3. `local-code` で起動

## 設定
設定ファイル: `local-code/config/default.toml`

主な設定:
```toml
[ollama]
url = "http://localhost:11434"
model = "Rnj-1"
connect_timeout = 30
read_timeout = 300

[ollama.retry]
max_retries = 3
initial_backoff_ms = 1000
backoff_multiplier = 2.0
max_backoff_ms = 10000

[agent]
initial_mode = "execute"
max_messages = 100

[tools]
bash_timeout = 120

[skills]
# custom_path = "C:\\path\\to\\skills" # Windows例

[lsp]
# command = "rust-analyzer"
# args = []
```

環境変数:
- `LOCAL_CODE_CONFIG`：設定ファイルのパスを上書き
- `LOCAL_CODE_SUPERPOWERS`：superpowers同梱ディレクトリのパスを指定

## Superpowers（同梱）
`local-code/superpowers` を自動読み込みします。  
個人スキルがある場合はそれが優先されます。

確認コマンド:
- `/skills`
- `/brainstorm`（`superpowers:brainstorming`）
- `/execute-plan`（`superpowers:executing-plans`）
- `/write-plan`（`superpowers:writing-plans`）

Tabでコマンド候補が循環します。

## LSP
Rustプロジェクトなら `rust-analyzer` がPATHにある場合自動起動します。  
他言語は `config/default.toml` の `[lsp]` を設定してください。

## Windows 注意点
- 実行ファイルは `local-code.exe`
- パス補完で `\\` が混ざる場合があります
- TLSは `rustls` を使用（企業証明書が必要な環境では追加設定が必要な場合あり）
