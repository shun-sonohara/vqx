# vqx - Vantiq CLI ラッパー

Vantiq CLI を安全かつ高機能にラップする Rust 製 CLI ツール。

## 概要

vqx は、underlying Vantiq CLI に対して、ワークフロー自動化、安全ガード、開発者フレンドリーな機能を提供します。

**準拠ドキュメント**: Vantiq CLI Reference Guide (PDF)

## 機能

### Phase 1（実装済み）

| コマンド | 説明 | PDF参照箇所 |
|----------|------|-------------|
| `doctor` | 環境前提条件のチェック | Prerequisites, Installation セクション |
| `profile` | 接続プロファイル管理 | Profile, Command Line Options セクション |
| `passthrough` | CLI直接実行 | 全コマンド |

### Phase 2以降（計画中）

- `export` / `import` - JSON正規化による git diff しやすい出力
- `diff` / `sync` - 環境間の比較・同期
- `safe-delete` - 確認とバックアップ付きの破壊的操作
- `promote` - ワークフロー: export → diff → confirm → import → test
- `run` - テストスイートとプロシージャ実行

## 前提条件

CLI Reference Guide PDF「Prerequisites」セクションより:

> The Vantiq CLI is a Java (Groovy) application and requires an installation of Java 11.

- Java 11 以降
- Vantiq CLI がインストール済みで PATH に設定されていること

## インストール

```bash
cargo install --path .
```

または、ソースからビルド:

```bash
cargo build --release
./target/release/vqx --help
```

## クイックスタート

### 1. 環境チェック

```bash
# Java と CLI が正しくインストールされているか確認
vqx doctor

# 詳細出力（PDF参照箇所付き）
vqx doctor --verbose
```

### 2. プロファイル作成

```bash
# 対話形式でセットアップ
vqx profile init

# または手動設定
vqx profile set myprofile \
    --url https://dev.vantiq.com \
    --token YOUR_ACCESS_TOKEN \
    --description "開発環境"
```

**PDF注記**: 「public clouds and any server using keycloak access require use of the token option」（パブリッククラウドおよびkeycloak認証を使用するサーバーではtokenオプションが必須）

### 3. CLIの使用

```bash
# プロファイルを指定してtypes一覧を取得
vqx --profile myprofile passthrough list types

# メタデータをエクスポート
vqx --profile myprofile passthrough export metadata -d ./export
```

## 設定

### プロファイル保存場所

プロファイルは TOML 形式で以下に保存:
- macOS/Linux: `~/.config/vqx/profiles.toml`
- Windows: `%APPDATA%\vqx\profiles.toml`

### グローバル設定

設定ファイルの場所:
- macOS/Linux: `~/.config/vqx/config.toml`
- Windows: `%APPDATA%\vqx\config.toml`

サンプル設定は [examples/config.toml](examples/config.toml) を参照。

## CLI 使用方法

### グローバルオプション

```
--cli <path>      Vantiq CLI 実行ファイルのパス（デフォルト: vantiq）
--profile <name>  接続に使用するプロファイル
--config <path>   vqx 設定ファイルのパス
--verbose         詳細出力を有効化
--quiet           非重要な出力を抑制
--output <fmt>    出力形式: text, json, csv
```

### コマンド

#### doctor

環境と CLI 前提条件をチェック。

```bash
vqx doctor                    # 全チェック
vqx doctor --java-only        # Java のみチェック
vqx doctor --cli-only         # CLI のみチェック
vqx doctor --test-connection  # サーバー接続もテスト
```

#### profile

接続プロファイルを管理。

```bash
vqx profile list              # 全プロファイル一覧
vqx profile show myprofile    # プロファイル詳細表示
vqx profile init              # 対話形式でプロファイル作成
vqx profile set <name> ...    # プロファイル作成/更新
vqx profile delete <name>     # プロファイル削除
vqx profile default <name>    # デフォルトプロファイル設定
vqx profile export <file>     # プロファイルをファイルにエクスポート
vqx profile import <file>     # ファイルからプロファイルをインポート
```

プロファイルオプション（PDF「Command Line Options」に基づく）:

| オプション | PDF フラグ | 説明 |
|-----------|-----------|------|
| `--url` | `-b` | ベースURL（デフォルト: https://dev.vantiq.com） |
| `--username` | `-u` | ユーザー名（Edgeサーバーのみ） |
| `--password` | `-p` | パスワード |
| `--token` | `-t` | アクセストークン（パブリッククラウド推奨） |
| `--namespace` | `-n` | ターゲット名前空間（username/passwordのみ） |
| `--trust-ssl` | `-trust` | SSL証明書を信頼 |

#### passthrough

underlying CLI に直接コマンドを渡す。

```bash
vqx passthrough list types
vqx passthrough find procedures MyProc
vqx passthrough export metadata -d ./export
vqx --profile prod passthrough run procedure Utils.getNamespaceAndProfiles
```

## PDF マッピング

### 接続オプション

| vqx | PDF CLI フラグ | 説明 |
|-----|---------------|------|
| `--profile` | `-s` | プロファイル名 |
| Profile.url | `-b` | ベースURL |
| Profile.username | `-u` | ユーザー名 |
| Profile.password | `-p` | パスワード |
| Profile.token | `-t` | アクセストークン |
| Profile.namespace | `-n` | ターゲット名前空間 |
| Profile.trust_ssl | `-trust` | SSL信頼 |

### 重要な PDF 注記

1. **Token vs Password**: 「If a password is specified, it is used instead of the token.」（パスワードが指定されている場合、トークンより優先される）

2. **Token 要件**: 「public clouds and any server using keycloak access require use of the token option」（パブリッククラウドとkeycloak認証サーバーではtokenオプション必須）

3. **Username/Password 制限**: 「username/password can only be used for Edge servers」（username/passwordはEdgeサーバーのみで使用可能）

4. **Namespace 制限**: 「the namespace option can only be used with username/password; it cannot be used with long-lived access tokens」（namespaceオプションはusername/passwordでのみ使用可能、長期アクセストークンでは使用不可）

5. **非推奨コマンド**: 「the execute command is deprecated in favor of the run procedure command as of release 1.37」（executeコマンドはrelease 1.37でrun procedureに置き換え、非推奨）

## セキュリティ

### 認証情報の保存

vqx は安全な認証情報保存をサポート:

1. **キーリング**（デフォルト）: システムキーチェーンを使用（macOS Keychain, Windows Credential Manager, Linux Secret Service）

2. **暗号化ファイル**: キーリングが利用できない場合のフォールバック（`age` 暗号化を使用）

3. **プレーンテキスト**: 非推奨、開発用途のみ

### 機密データの取り扱い

- パスワードとトークンはログに記録されない
- CLIの引数は詳細出力でマスクされる
- 利用可能な場合はデフォルトで安全なストレージを使用

## 開発

### ビルド

```bash
cargo build
cargo test
```

### プロジェクト構造

```
src/
  main.rs           # エントリーポイント
  cli.rs            # CLIコマンド定義（clap）
  config.rs         # グローバル設定
  error.rs          # エラー型
  profile.rs        # プロファイル管理
  underlying.rs     # CLI実行層（PDFベース）
  commands/
    mod.rs
    doctor.rs       # 環境チェック
    profile.rs      # プロファイル管理
    passthrough.rs  # CLI直接実行
```

### 新しいコマンドの追加

1. `src/cli.rs` でコマンドを定義
2. `src/commands/` に実装を作成
3. `src/main.rs` のディスパッチに追加
4. コードコメントに PDF マッピングを文書化

## ライセンス

MIT

## コントリビューション

ガイドラインは [CONTRIBUTING.md](CONTRIBUTING.md) を参照。
