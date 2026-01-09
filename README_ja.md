# vqx - Vantiq CLI ラッパー

Vantiq CLI を安全かつ高機能にラップする Rust 製 CLI ツール。

## 概要

vqx は Vantiq CLI に対して、ワークフロー自動化、安全ガード、開発者フレンドリーな機能を提供します。

**準拠ドキュメント**: [Vantiq CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/)

## 機能

- **プロファイル管理** - キーリング対応のセキュアな認証情報管理
- **エクスポート/インポート** - git diff しやすい JSON 正規化
- **環境比較** - プロファイルまたはディレクトリ間の差分比較
- **同期** - pull/push ワークフローによる双方向同期
- **安全な操作** - 破壊的操作のバックアップと確認
- **環境間移行** - クロス環境デプロイの自動ワークフロー
- **テスト実行** - テスト、テストスイート、プロシージャの実行
- **CLI直接アクセス** - Vantiq CLI へのパススルー

## 前提条件

[Vantiq CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/) より:

> The Vantiq CLI is a Java (Groovy) application and requires an installation of Java 11.

- Java 11 以降
- Vantiq CLI がインストール済みで PATH に設定されていること

## インストール

### GitHub Releases からダウンロード（推奨）

[GitHub Releases](https://github.com/shun-sonohara/vqx/releases) からバイナリをダウンロード。

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

macOS でブロックされた場合、quarantine 属性を削除:

```bash
xattr -d com.apple.quarantine /usr/local/bin/vqx
```

#### Windows

1. [Releases](https://github.com/shun-sonohara/vqx/releases) から `vqx-windows-x86_64.zip` をダウンロード
2. ZIP を展開
3. `vqx.exe` を PATH に含まれるディレクトリに配置

### ソースからビルド

```bash
cargo install --path .
# または手動ビルド
cargo build --release
./target/release/vqx --help
```

## クイックスタート

### 1. 環境チェック

```bash
vqx doctor
```

### 2. プロファイル作成

```bash
# 対話形式
vqx profile init

# または手動設定
vqx profile set dev \
    --url https://dev.vantiq.com \
    --token YOUR_ACCESS_TOKEN
```

### 3. コマンド実行

```bash
# リソースをエクスポート
vqx -s dev export -d ./export

# タイプ一覧（CLI直接アクセス）
vqx -s dev list types
```

## 設定

### プロファイル保存場所

- macOS/Linux: `~/.config/vqx/profiles.toml`
- Windows: `%APPDATA%\vqx\profiles.toml`

```toml
default_profile = "dev"

[profiles.dev]
url = "https://dev.vantiq.com"
token = "YOUR_ACCESS_TOKEN"
trust_ssl = false
description = "開発環境"
use_secure_storage = false  # true = キーリングに保存
```

**認証オプション** ([CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/) より):

| フィールド | CLIフラグ | 説明 |
|-----------|----------|------|
| `url` | `-b` | Vantiq サーバー URL |
| `token` | `-t` | アクセストークン（パブリッククラウドで必須） |
| `username` | `-u` | ユーザー名（Edge サーバーのみ） |
| `password` | `-p` | パスワード（Edge サーバーのみ） |
| `namespace` | `-n` | ターゲット名前空間（username/password のみ） |
| `trust_ssl` | `-trust` | SSL証明書を信頼 |

### グローバル設定

設定ファイルの場所:
- macOS/Linux: `~/.config/vqx/config.toml`
- Windows: `%APPDATA%\vqx\config.toml`

```toml
cli_path = "vantiq"
timeout_seconds = 120
max_retries = 3

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
```

### 環境変数

| 変数 | 説明 |
|------|------|
| `VQX_CLI_PATH` | Vantiq CLI 実行ファイルのパス |
| `VQX_PROFILE` | デフォルトプロファイル名 |
| `VQX_CONFIG` | config.toml のパス |

## コマンド

### グローバルオプション

```
-s, --profile <name>  接続に使用するプロファイル
--cli <path>          Vantiq CLI 実行ファイルのパス
--config <path>       設定ファイルのパス
-v, --verbose         詳細出力を有効化
-q, --quiet           非必須出力を抑制
--output <format>     出力形式: text, json, csv
```

---

### doctor

環境と CLI の前提条件をチェック。

```bash
vqx doctor                    # 全チェック
vqx doctor --java-only        # Java のみ
vqx doctor --cli-only         # CLI のみ
vqx doctor --test-connection  # サーバー接続もテスト
```

---

### profile

セキュアな認証情報ストレージでプロファイルを管理。

```bash
# プロファイル一覧
vqx profile list

# プロファイル詳細
vqx profile show dev

# 対話形式で作成
vqx profile init

# プロファイル作成/更新
vqx profile set dev --url https://dev.vantiq.com --token YOUR_TOKEN

# キーリングに保存
vqx profile set dev --url https://dev.vantiq.com --token YOUR_TOKEN --secure

# デフォルト設定
vqx profile default dev

# プロファイル削除
vqx profile delete dev

# エクスポート/インポート
vqx profile export profiles.toml
vqx profile import profiles.toml --overwrite
```

**プロファイルオプション:**

| オプション | CLIフラグ | 説明 |
|-----------|----------|------|
| `--url` | `-b` | ベース URL |
| `--username` | `-u` | ユーザー名（Edge のみ） |
| `--password` | `-p` | パスワード |
| `--token` | `-t` | アクセストークン（推奨） |
| `--namespace` | `-n` | ターゲット名前空間 |
| `--trust-ssl` | `-trust` | SSL証明書を信頼 |
| `--secure` | - | キーリングに保存 |

---

### export

git diff しやすい JSON 正規化付きでリソースをエクスポート。

```bash
# メタデータをエクスポート（デフォルト）
vqx -s dev export -d ./export

# データをエクスポート
vqx -s dev export data -d ./data

# プロジェクトをエクスポート
vqx -s dev export project --project MyProject -d ./export

# チャンク指定（大量データ用）
vqx -s dev export -d ./export --chunk 5000

# 特定タイプのみ
vqx -s dev export --include types --include procedures

# 特定タイプを除外
vqx -s dev export --exclude rules

# JSON 正規化を無効化
vqx -s dev export -d ./export --normalize false
```

**エクスポートオプション:**

| オプション | CLIフラグ | 説明 |
|-----------|----------|------|
| `-d, --directory` | `-d` | 出力ディレクトリ |
| `--chunk` | `-chunk` | チャンクサイズ |
| `--include` | `-include` | 含めるタイプ（複数可） |
| `--exclude` | `-exclude` | 除外するタイプ（複数可） |
| `--until` | `-until` | タイムスタンプまでエクスポート |
| `--ignore-errors` | `-ignoreErrors` | エラーを無視 |
| `--normalize` | - | JSON 正規化（デフォルト: true） |

**JSON 正規化:**
- オブジェクトキーをアルファベット順にソート
- 配列を `name` フィールドで安定化
- 変動するタイムスタンプを除去（`ars_createdAt`, `ars_modifiedAt` 等）
- 2スペースインデント

---

### import

安全確認付きでリソースをインポート。

```bash
# メタデータをインポート（確認あり）
vqx -s dev import -d ./export

# データをインポート
vqx -s dev import data -d ./data

# チャンク指定
vqx -s dev import -d ./export --chunk 5000

# 特定タイプのみ
vqx -s dev import --include types

# 確認をスキップ（CI/CD 用）
vqx -s dev import -d ./export --yes
```

**インポートオプション:**

| オプション | CLIフラグ | 説明 |
|-----------|----------|------|
| `-d, --directory` | `-d` | 入力ディレクトリ |
| `--chunk` | `-chunk` | チャンクサイズ |
| `--include` | `-include` | 含めるタイプ |
| `--exclude` | `-exclude` | 除外するタイプ |
| `--ignore` | `-ignore` | 無視するリソースタイプ |
| `-y, --yes` | - | 確認をスキップ |

---

### diff

2つのソース（プロファイルまたはディレクトリ）間でリソースを比較。

```bash
# ディレクトリ間比較
vqx diff ./local ./other

# プロファイルとディレクトリを比較
vqx diff dev ./local

# プロファイル間比較
vqx diff dev prod

# 完全な差分出力
vqx diff ./source ./target --full

# リソースタイプでフィルタ
vqx diff ./source ./target --resource types --resource procedures
```

**diff オプション:**

| オプション | 説明 |
|-----------|------|
| `--full` | 完全な差分出力を表示 |
| `--resource` | リソースタイプでフィルタ（複数可） |

**機能:**
- プロファイルから自動エクスポートして比較
- JSON 正規化で正確な比較
- 色分け出力（緑: 追加、赤: 削除、黄: 変更）

---

### sync

ローカルディレクトリと Vantiq サーバー間でリソースを同期。

#### sync pull

リモートからローカルへエクスポート。

```bash
# サーバーからプル
vqx -s dev sync pull -d ./local

# 強制上書き
vqx -s dev sync pull -d ./local --force
```

#### sync push

差分プレビュー付きでローカルからリモートへインポート。

```bash
# 差分プレビューと確認付きでプッシュ
vqx -s dev sync push -d ./local

# ドライラン - プッシュ内容を表示
vqx -s dev sync push -d ./local --dry-run

# 確認をスキップ（CI/CD 用）
vqx -s dev sync push -d ./local --yes
```

**sync オプション:**

| サブコマンド | オプション | 説明 |
|-------------|-----------|------|
| `pull` | `-d, --directory` | ローカルディレクトリ |
| `pull` | `--force` | 強制上書き |
| `push` | `-d, --directory` | ローカルディレクトリ |
| `push` | `--dry-run` | 変更のプレビューのみ |
| `push` | `-y, --yes` | 確認をスキップ |

---

### run

Vantiq でテスト、テストスイート、プロシージャを実行。

#### run test

```bash
vqx -s dev run test MyTest
```

#### run testsuite

```bash
# テストスイートを実行
vqx -s dev run testsuite MyTestSuite

# 特定のテストから開始
vqx -s dev run testsuite MyTestSuite --start-from SpecificTest
```

#### run procedure

```bash
# プロシージャを実行
vqx -s dev run procedure MyProcedure

# パラメータ付きで実行
vqx -s dev run procedure MyProcedure param1:value1 param2:value2
```

**run オプション:**

| サブコマンド | オプション | 説明 |
|-------------|-----------|------|
| `test` | `<name>` | テスト名 |
| `testsuite` | `<name>` | テストスイート名 |
| `testsuite` | `--start-from` | 開始テスト |
| `procedure` | `<name>` | プロシージャ名 |
| `procedure` | `[params...]` | パラメータ（`name:value` 形式） |

---

### safe-delete

バックアップと確認付きで安全にリソースを削除。

```bash
# 単一リソースを削除
vqx -s dev safe-delete types MyType

# クエリで削除（deleteMatching）
vqx -s dev safe-delete types '{"name": {"$regex": "Test.*"}}'

# ドライラン - プレビューのみ
vqx -s dev safe-delete types MyType --dry-run

# バックアップをスキップ
vqx -s dev safe-delete types MyType --no-backup

# 確認をスキップ
vqx -s dev safe-delete types MyType --yes

# 100件以上を強制削除
vqx -s dev safe-delete types '{"obsolete": true}' --force
```

**safe-delete オプション:**

| オプション | 説明 |
|-----------|------|
| `--dry-run` | 削除せずプレビュー |
| `--no-backup` | バックアップをスキップ |
| `-y, --yes` | 確認をスキップ |
| `--force` | 100件以上の削除を許可 |

**安全機能:**
- `~/.local/share/vqx/backups/` への自動バックアップ
- 削除対象を表示する確認プロンプト
- deleteMatching の 100件制限（`--force` で解除）
- ドライランモード

---

### promote

環境間でリソースを移行。

```bash
# 基本的な移行
vqx promote --from dev --to prod

# 差分表示をスキップ
vqx promote --from dev --to prod --no-diff

# 移行後にテストスイートを実行
vqx promote --from dev --to prod --testsuite SmokeTests

# 移行後にプロシージャを実行
vqx promote --from dev --to prod --procedure ValidateDeployment

# 確認をスキップ（CI/CD 用）
vqx promote --from dev --to prod --yes
```

**promote オプション:**

| オプション | 説明 |
|-----------|------|
| `--from` | ソースプロファイル |
| `--to` | ターゲットプロファイル |
| `--no-diff` | 差分表示をスキップ |
| `--no-test` | 移行後テストをスキップ |
| `--testsuite` | 移行後に実行するテストスイート |
| `--procedure` | 移行後に実行するプロシージャ |
| `-y, --yes` | 確認をスキップ |

**ワークフロー:**
1. ソースからメタデータをエクスポート
2. ターゲットと比較（オプション）
3. 移行を確認
4. ターゲットへインポート
5. テスト実行（オプション）

---

### CLI 直接アクセス

認識されないコマンドは Vantiq CLI に直接渡されます。

```bash
# タイプ一覧
vqx -s dev list types

# リソース検索
vqx -s dev find procedures MyProc

# クエリで選択
vqx -s dev select types

# その他の CLI コマンド
vqx -s dev <command> [args...]
```

## CLI リファレンスノート

[Vantiq CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/) より:

1. **トークン vs パスワード**: 「パスワードが指定された場合、トークンの代わりにパスワードが使用される」

2. **トークン必須**: 「パブリッククラウドおよび keycloak を使用するサーバーではトークンオプションが必要」

3. **username/password の制限**: 「username/password は Edge サーバーでのみ使用可能」

4. **namespace の制限**: 「namespace オプションは username/password でのみ使用可能。長期アクセストークンでは使用不可」

## セキュリティ

### 認証情報ストレージ

vqx は複数の認証情報保存方法をサポート:

1. **キーリング**（推奨）: システムキーチェーン（macOS Keychain, Windows Credential Manager, Linux Secret Service）
2. **プレーンテキスト**: profiles.toml 内（本番環境では非推奨）

### 機密データの取り扱い

- パスワードとトークンはログに記録されない
- 詳細出力でも CLI 引数はマスクされる
- `--secure` フラグでセキュアストレージを利用可能

## 開発

### ビルド

```bash
cargo build
cargo test
cargo build --release
```

### プロジェクト構造

```
src/
  main.rs           # エントリーポイント
  cli.rs            # CLI 定義（clap）
  config.rs         # 設定
  error.rs          # エラー型
  normalizer.rs     # JSON 正規化
  profile.rs        # プロファイル管理
  underlying.rs     # CLI 実行層
  commands/
    doctor.rs       # 環境チェック
    profile.rs      # プロファイル管理
    export.rs       # 正規化付きエクスポート
    import.rs       # 確認付きインポート
    diff.rs         # 環境比較
    sync.rs         # pull/push 同期
    run.rs          # テスト/プロシージャ実行
    safe_delete.rs  # 安全な削除
    promote.rs      # 環境間移行
    external.rs     # CLI パススルー
```

## リリース手順

リリースは GitHub Actions で手動作成。

### リリースの作成

1. [Actions → Release](https://github.com/shun-sonohara/vqx/actions/workflows/auto-release.yml) に移動
2. "Run workflow" をクリック
3. バージョンタイプを選択:
   - `patch` - バグ修正（0.1.0 → 0.1.1）
   - `minor` - 新機能（0.1.0 → 0.2.0）
   - `major` - 破壊的変更（0.1.0 → 1.0.0）
4. "Run workflow" をクリック

### CI/CD

| ワークフロー | トリガー | 説明 |
|-------------|---------|------|
| CI | プルリクエスト | フォーマット、lint、ビルド、テスト |
| Release | 手動 | バージョンバンプ + ビルド + リリース |

## ライセンス

MIT

## コントリビューション

ガイドラインは [CONTRIBUTING.md](CONTRIBUTING.md) を参照。
