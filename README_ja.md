# vqx - Vantiq CLI ラッパー

Vantiq CLI を安全かつ高機能にラップする Rust 製 CLI ツール。

## 概要

vqx は、underlying Vantiq CLI に対して、ワークフロー自動化、安全ガード、開発者フレンドリーな機能を提供します。

**準拠ドキュメント**: [Vantiq CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/)

## 機能

### Phase 1（実装済み）

| コマンド | 説明 | 参照箇所 |
|----------|------|----------|
| `doctor` | 環境前提条件のチェック | Prerequisites, Installation セクション |
| `profile` | 接続プロファイル管理 | Profile, Command Line Options セクション |
| `passthrough` | CLI直接実行 | 全コマンド |

### Phase 2（実装済み）

| コマンド | 説明 | 参照箇所 |
|----------|------|----------|
| `export` | JSON正規化付きリソースエクスポート | Export セクション |
| `import` | 安全確認付きリソースインポート | Import セクション |

**主な機能:**
- git diff しやすい JSON 正規化（キーソート、配列安定化、タイムスタンプ除去）
- 破壊的操作前の確認プロンプト
- 進捗インジケーターと詳細出力

### Phase 3（実装済み）

| コマンド | 説明 | 参照箇所 |
|----------|------|----------|
| `diff` | 環境またはディレクトリ間のリソース比較 | - |
| `sync pull` | リモートからローカルへエクスポート（正規化付き） | Export セクション |
| `sync push` | 差分プレビューと確認付きでリモートへインポート | Import セクション |

**主な機能:**
- プロファイル（リモート）またはディレクトリ（ローカル）の比較
- 比較のための自動エクスポートと正規化
- push操作前の差分プレビュー
- 安全のための確認プロンプト

### Phase 4（実装済み）

| コマンド | 説明 | 参照箇所 |
|----------|------|----------|
| `run test` | 単体テスト実行 | Run セクション |
| `run testsuite` | テストスイート実行 | Run セクション |
| `run procedure` | パラメータ付きプロシージャ実行 | Run セクション |
| `safe-delete` | バックアップと確認付き削除 | Delete セクション |
| `promote` | 環境間リソース移行 | - |

**主な機能:**
- 削除前の自動バックアップ（`~/.local/share/vqx/backups` に保存）
- safe-delete の dry-run モード
- 大量削除の制限保護（デフォルト100件）
- 環境移行ワークフロー: export → diff → confirm → import → test

## 前提条件

[Vantiq CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/)「Prerequisites」セクションより:

> The Vantiq CLI is a Java (Groovy) application and requires an installation of Java 11.

- Java 11 以降
- Vantiq CLI がインストール済みで PATH に設定されていること

## インストール

### GitHub Releases からダウンロード（推奨）

[GitHub Releases](https://github.com/shun-sonohara/vqx/releases) からお使いのプラットフォーム用のバイナリをダウンロードしてください。

| プラットフォーム | ファイル |
|-----------------|----------|
| Linux x86_64 | `vqx-linux-x86_64.tar.gz` |
| macOS Intel | `vqx-macos-x86_64.tar.gz` |
| macOS Apple Silicon | `vqx-macos-aarch64.tar.gz` |
| Windows x86_64 | `vqx-windows-x86_64.zip` |

#### Linux / macOS

```bash
# ダウンロードと展開（macOS Apple Silicon の例）
curl -LO https://github.com/shun-sonohara/vqx/releases/latest/download/vqx-macos-aarch64.tar.gz
tar xzf vqx-macos-aarch64.tar.gz

# PATH に含まれるディレクトリに移動
sudo mv vqx /usr/local/bin/

# インストール確認
vqx --version
```

#### macOS セキュリティ（Gatekeeper）

macOS では以下のようなメッセージでバイナリがブロックされることがあります：
> 「開発元を検証できないため開けません」

または

> 「Macに損害を与えたり、プライバシーを侵害する可能性のあるマルウェアが含まれていないことを検証できませんでした」

この問題を解決するには、quarantine 属性を削除します：

```bash
# quarantine 属性を削除
xattr -d com.apple.quarantine /usr/local/bin/vqx

# カレントディレクトリに展開した場合
xattr -d com.apple.quarantine ./vqx
```

または、Finder でバイナリを右クリックし、「開く」を選択してから、ダイアログで「開く」をクリックすることもできます。

#### Windows

1. [Releases](https://github.com/shun-sonohara/vqx/releases) から `vqx-windows-x86_64.zip` をダウンロード
2. ZIP ファイルを展開
3. `vqx.exe` を PATH に含まれるディレクトリに移動するか、展開したディレクトリを PATH に追加

**PATH への追加（PowerShell）:**

```powershell
# ユーザー PATH に追加（ターミナルの再起動が必要）
$path = [Environment]::GetEnvironmentVariable("Path", "User")
[Environment]::SetEnvironmentVariable("Path", "$path;C:\path\to\vqx", "User")
```

**Windows セキュリティ（SmartScreen）:**

Windows で SmartScreen の警告が表示される場合があります：
> 「Windows によって PC が保護されました」

「詳細情報」をクリックし、「実行」をクリックしてアプリケーションを許可してください。

### ソースからビルド

```bash
cargo install --path .
```

または手動でビルド：

```bash
cargo build --release
./target/release/vqx --help
```

## クイックスタート

### 1. 環境チェック

```bash
# Java と CLI が正しくインストールされているか確認
vqx doctor

# 詳細出力（リファレンス参照付き）
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

**注記**: 「public clouds and any server using keycloak access require use of the token option」（パブリッククラウドおよびkeycloak認証を使用するサーバーではtokenオプションが必須）

### 3. CLIの使用

```bash
# プロファイルを指定してtypes一覧を取得
vqx --profile myprofile passthrough list types

# メタデータをエクスポート
vqx --profile myprofile passthrough export metadata -d ./export
```

## 設定

### プロファイル保存場所 (profiles.toml)

プロファイルは TOML 形式で以下に保存:
- macOS/Linux: `~/.config/vqx/profiles.toml`
- Windows: `%APPDATA%\vqx\profiles.toml`

サンプル設定は [examples/profiles.toml](examples/profiles.toml) を参照。

**プロファイル構造:**

```toml
# --profile が指定されていない場合のデフォルトプロファイル
default_profile = "dev"

[profiles.dev]
url = "https://dev.vantiq.com"
token = "YOUR_ACCESS_TOKEN"           # パブリッククラウド用（推奨）
# username = "user"                   # エッジサーバーのみ
# password = "pass"                   # エッジサーバーのみ
# namespace = "MyNamespace"           # username/passwordでのみ使用可能
trust_ssl = false
description = "開発環境"
use_secure_storage = false            # true = キーリングに認証情報を保存
```

**認証オプション（[CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/) に基づく）:**

| フィールド | CLI フラグ | 説明 |
|-----------|----------|------|
| `url` | `-b` | Vantiq サーバー URL |
| `token` | `-t` | アクセストークン（パブリッククラウド必須） |
| `username` | `-u` | ユーザー名（エッジサーバーのみ） |
| `password` | `-p` | パスワード（エッジサーバーのみ） |
| `namespace` | `-n` | ターゲット名前空間（username/passwordのみ） |
| `trust_ssl` | `-trust` | SSL証明書を信頼 |

**プロファイル管理コマンド:**

```bash
# 対話形式でプロファイル作成
vqx profile init

# プロファイル作成/更新
vqx profile set myprofile --url https://dev.vantiq.com --token YOUR_TOKEN

# 安全なストレージ（キーリング）でプロファイル作成
vqx profile set myprofile --url https://dev.vantiq.com --token YOUR_TOKEN --secure

# 全プロファイル一覧
vqx profile list

# プロファイル詳細表示
vqx profile show myprofile

# デフォルトプロファイル設定
vqx profile default myprofile

# プロファイル削除
vqx profile delete myprofile
```

### グローバル設定 (config.toml)

設定ファイルの場所:
- macOS/Linux: `~/.config/vqx/config.toml`
- Windows: `%APPDATA%\vqx\config.toml`

サンプル設定は [examples/config.toml](examples/config.toml) を参照。

**設定オプション:**

```toml
# CLI実行ファイルパス
cli_path = "vantiq"

# 実行設定
timeout_seconds = 120
max_retries = 3
retry_delay_ms = 1000
default_chunk_size = 5000

# ログ
[logging]
level = "info"              # trace, debug, info, warn, error
format = "text"             # text, json

# 出力
[output]
default_format = "table"    # json, table, csv
pretty_json = true
colors = true
progress = true

# gitフレンドリーな差分のためのJSON正規化
[normalization]
sort_keys = true
sort_arrays = true
excluded_fields = [
    "ars_modifiedAt",
    "ars_createdAt",
    "ars_modifiedBy",
    "ars_createdBy",
    "_id",
    "ars_version",
]

# 安全な削除設定（Phase 4）
[safe_delete]
require_confirm = true
require_backup = true
max_items_without_force = 10
blocked_prefixes = ["System", "ARS"]
```

**環境変数:**

| 変数 | 説明 |
|------|------|
| `VQX_CLI_PATH` | `cli_path` 設定を上書き |
| `VQX_PROFILE` | デフォルトプロファイル名 |
| `VQX_CONFIG` | config.toml のパス |

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

プロファイルオプション（[CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/)「Command Line Options」に基づく）:

| オプション | CLI フラグ | 説明 |
|-----------|-----------|------|
| `--url` | `-b` | ベースURL（デフォルト: https://dev.vantiq.com） |
| `--username` | `-u` | ユーザー名（Edgeサーバーのみ） |
| `--password` | `-p` | パスワード |
| `--token` | `-t` | アクセストークン（パブリッククラウド推奨） |
| `--namespace` | `-n` | ターゲット名前空間（username/passwordのみ） |
| `--trust-ssl` | `-trust` | SSL証明書を信頼 |

#### CLI 直接実行

認識されないコマンドは自動的に underlying Vantiq CLI に渡されます。

```bash
# これらのコマンドは Vantiq CLI に直接渡されます
vqx list types
vqx find procedures MyProc
vqx select types
vqx --profile prod run procedure Utils.getNamespaceAndProfiles

# プロファイル指定
vqx --profile dev list types
```

#### export

Vantiq からリソースをエクスポート。JSON 正規化により git diff しやすい出力を生成。

```bash
# メタデータをエクスポート（デフォルト）
vqx export -d ./export

# タイプを指定してエクスポート
vqx export metadata -d ./export
vqx export data -d ./export

# プロジェクトをエクスポート
vqx export project --project MyProject -d ./export

# チャンク指定（大量エクスポート時）
vqx export metadata -d ./export --chunk 5000

# 特定タイプのみエクスポート
vqx export metadata --include types --include procedures

# JSON正規化を無効化
vqx export metadata -d ./export --normalize false
```

エクスポートオプション（[CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/)「Export」セクションに基づく）:

| オプション | CLI フラグ | 説明 |
|-----------|-----------|------|
| `-d, --directory` | `-d` | 出力ディレクトリ |
| `--chunk` | `-chunk` | 大量エクスポート時のチャンクサイズ |
| `--include` | `-include` | 含めるタイプ |
| `--exclude` | `-exclude` | 除外するタイプ |
| `--until` | `-until` | 指定タイムスタンプまでのデータをエクスポート |
| `--ignore-errors` | `-ignoreErrors` | エクスポート中のエラーを無視 |
| `--normalize` | (vqx拡張) | git diff 用 JSON 正規化（デフォルト: true） |

**JSON 正規化**（vqx 拡張機能）:
- オブジェクトキーをアルファベット順にソート
- 配列を `name` または `ars_version` で安定ソート
- 変動するタイムスタンプを除去（`ars_createdAt`, `ars_modifiedAt` など）
- 一貫したインデント（2スペース）

#### import

Vantiq へリソースをインポート。安全確認機能付き。

```bash
# メタデータをインポート（確認プロンプトあり）
vqx import metadata -d ./export

# データをインポート
vqx import data -d ./data

# チャンク指定でインポート
vqx import metadata -d ./export --chunk 5000

# 特定タイプのみインポート
vqx import metadata --include types --exclude rules

# 確認をスキップ（CI/CD用）
vqx import metadata -d ./export --yes
```

インポートオプション（[CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/)「Import」セクションに基づく）:

| オプション | CLI フラグ | 説明 |
|-----------|-----------|------|
| `-d, --directory` | `-d` | 入力ディレクトリ |
| `--chunk` | `-chunk` | 大量インポート時のチャンクサイズ |
| `--include` | `-include` | 含めるタイプ |
| `--exclude` | `-exclude` | 除外するタイプ |
| `--ignore` | `-ignore` | 無視するリソースタイプ |
| `-y, --yes` | (vqx拡張) | 確認プロンプトをスキップ |

**安全機能**（vqx 拡張機能）:
- インポート前の確認プロンプト（誤操作による上書きを防止）
- ファイル数のプレビュー
- サーバーとプロファイルの表示

#### diff

2つのソース（プロファイルまたはディレクトリ）間のリソースを比較。

```bash
# 2つのディレクトリを比較
vqx diff ./local-export ./other-export

# プロファイルとローカルディレクトリを比較
vqx diff my-profile ./local-export

# 2つのプロファイルを比較（リモート間）
vqx diff dev-profile prod-profile

# 完全な差分出力を表示
vqx diff ./source ./target --full

# リソースタイプでフィルタ
vqx diff ./source ./target --resource types --resource procedures
```

diffオプション:

| オプション | 説明 |
|-----------|------|
| `--full` | 完全な差分出力を表示（サマリーだけでなく） |
| `--resource` | 特定のリソースタイプにフィルタ |

**機能:**
- プロファイルから自動エクスポートして比較
- JSON正規化により正確な比較を実現
- 追加、削除、変更されたリソースを表示
- 見やすいカラー出力

#### sync

ローカルディレクトリとVantiqサーバー間のリソースを同期。

**sync pull** - リモートからローカルへエクスポート:

```bash
# サーバーからローカルディレクトリへ pull
vqx sync pull -d ./local

# 強制上書き（確認スキップ）
vqx sync pull -d ./local --force
```

**sync push** - ローカルからリモートへインポート:

```bash
# 差分プレビューと確認付きでpush
vqx sync push -d ./local

# ドライラン - pushされる内容を表示
vqx sync push -d ./local --dry-run

# 確認をスキップ（CI/CD用）
vqx sync push -d ./local --yes
```

syncオプション:

| サブコマンド | オプション | 説明 |
|-------------|-----------|------|
| `pull` | `-d, --directory` | 同期先のローカルディレクトリ |
| `pull` | `--force` | 確認なしで強制上書き |
| `push` | `-d, --directory` | 同期元のローカルディレクトリ |
| `push` | `--dry-run` | 変更を適用せずに表示 |
| `push` | `-y, --yes` | 確認プロンプトをスキップ |

**機能:**
- push前の自動差分プレビュー
- pull時のJSON正規化
- 安全のための確認プロンプト
- 進捗インジケーター

## CLI Reference マッピング

### 接続オプション

| vqx | CLI フラグ | 説明 |
|-----|---------------|------|
| `--profile` | `-s` | プロファイル名 |
| Profile.url | `-b` | ベースURL |
| Profile.username | `-u` | ユーザー名 |
| Profile.password | `-p` | パスワード |
| Profile.token | `-t` | アクセストークン |
| Profile.namespace | `-n` | ターゲット名前空間 |
| Profile.trust_ssl | `-trust` | SSL信頼 |

### 重要な注記（[CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/) より）

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
  normalizer.rs     # git diff 用 JSON 正規化
  profile.rs        # プロファイル管理
  underlying.rs     # CLI実行層
  commands/
    mod.rs
    diff.rs         # 環境比較
    doctor.rs       # 環境チェック
    export.rs       # 正規化付きエクスポート
    external.rs     # CLI直接実行（パススルー）
    import.rs       # 安全確認付きインポート
    profile.rs      # プロファイル管理
    promote.rs      # 環境間移行
    run.rs          # テスト/プロシージャ実行
    safe_delete.rs  # バックアップ付き安全削除
    sync.rs         # 双方向同期（pull/push）
```

### 新しいコマンドの追加

1. `src/cli.rs` でコマンドを定義
2. `src/commands/` に実装を作成
3. `src/main.rs` のディスパッチに追加
4. コードコメントに CLI Reference マッピングを文書化

## リリース手順

リリースはコスト管理のため GitHub Actions で手動で行います。

### リリースの作成

1. [Actions → Release](https://github.com/shun-sonohara/vqx/actions/workflows/auto-release.yml) に移動
2. "Run workflow" をクリック
3. バージョンタイプを選択：
   - `patch` - バグ修正（0.1.0 → 0.1.1）
   - `minor` - 新機能（0.1.0 → 0.2.0）
   - `major` - 破壊的変更（0.1.0 → 1.0.0）
4. "Run workflow" をクリック

ワークフローは以下を実行します：
1. `Cargo.toml` のバージョンをバンプ
2. git タグを作成してプッシュ
3. 全プラットフォーム用バイナリをビルド
4. アーティファクト付き GitHub Release を作成

### CI/CD

| ワークフロー | トリガー | 説明 |
|-------------|---------|------|
| CI | プルリクエスト | フォーマット、clippy、ビルド、テスト（ubuntuのみ） |
| Release | 手動（workflow_dispatch） | バージョンバンプ + ビルド + リリース |

## ライセンス

MIT

## コントリビューション

ガイドラインは [CONTRIBUTING.md](CONTRIBUTING.md) を参照。
