# セキュリティポリシー

## 認証情報の保存

vqx は認証情報の保存に複数のオプションを提供し、安全なストレージをデフォルトとしています。

### ストレージオプション

1. **システムキーリング（デフォルト、推奨）**
   - macOS: キーチェーンアクセス
   - Windows: 資格情報マネージャー
   - Linux: Secret Service（GNOME Keyring, KWallet）

   ```bash
   vqx profile set myprofile --token YOUR_TOKEN --secure
   ```

2. **暗号化ファイルフォールバック**
   - キーリングが利用できない場合に使用
   - `age` 暗号化を使用
   - ユーザーパスワードから鍵を導出

3. **プレーンテキスト（非推奨）**
   - 開発/テスト用途のみ
   - 認証情報は `~/.config/vqx/profiles.toml` に保存
   - ファイル権限を制限すべき（600）

### ベストプラクティス

1. **認証情報設定時は常に `--secure` フラグを使用**:
   ```bash
   vqx profile set production --token $VANTIQ_TOKEN --secure
   ```

2. **CI/CD では環境変数を使用**:
   ```bash
   export VQX_PROFILE=ci
   # または underlying CLI に直接トークンを渡す
   vqx passthrough -t $VANTIQ_TOKEN list types
   ```

3. **ファイル権限を制限**:
   ```bash
   chmod 600 ~/.config/vqx/profiles.toml
   chmod 700 ~/.config/vqx/
   ```

## 機密データの取り扱い

### ログ記録

- パスワードとトークンは**決して**ログに記録されない
- CLI コマンドはマスクされた引数で表示:
  ```
  Args: -b https://dev.vantiq.com -u user -p ******** list types
  ```

### メモリ

- 認証情報は必要以上に長く保持されない
- Rust の所有権モデルにより偶発的な漏洩を防止

### ネットワーク

- デフォルトで全ての接続は HTTPS を使用
- `--trust-ssl` は自己署名証明書を持つ Edge サーバーにのみ使用すべき

## 破壊的操作（Phase 4）

vqx は破壊的な CLI 操作を安全ガードでラップします:

### 保護対象コマンド

CLI Reference Guide PDF に基づく:
- `delete <resource> <resourceId>`
- `deleteMatching <resource> <query>`
- `undeploy <configurationName>`

### 安全対策

1. **ドライラン**: 影響を受けるものをプレビュー
   ```bash
   vqx safe-delete types MyType --dry-run
   ```

2. **確認必須**: デフォルトで対話的プロンプト
   ```bash
   vqx safe-delete types MyType
   # 1件のアイテムを削除しますか？ [y/N]
   ```

3. **自動バックアップ**: 削除前にエクスポート
   ```bash
   vqx safe-delete types MyType
   # バックアップ保存先: ~/.vqx/backups/2024-01-15T10-30-00/
   ```

4. **ブロックリスト**: システムリソースの削除を防止
   ```toml
   [safe_delete]
   blocked_prefixes = ["System", "ARS"]
   ```

5. **数量制限**: 大量削除には `--force` が必要
   ```toml
   [safe_delete]
   max_items_without_force = 10
   ```

## セキュリティ問題の報告

セキュリティ脆弱性を発見した場合:

1. 公開イシューを**作成しない**
2. セキュリティに関する懸念をメンテナーにメール
3. 再現手順を含める
4. 開示前に修正のための時間を確保

## 監査証跡

有効化すると、vqx は全ての操作を以下の情報とともにログ記録:
- プロファイル名（認証情報ではない）
- サーバー URL
- 実行されたコマンド
- タイムスタンプ
- 結果ステータス

ログエントリの例:
```
2024-01-15T10:30:00Z INFO vqx: Executing CLI command
    profile=production
    url=https://prod.vantiq.com
    command=export
    args=["metadata", "-d", "./export"]
```

## underlying CLI の仕様制約（PDF準拠）

vqx は以下の PDF 記載の制約をコードレベルで強制します:

### 認証方式の制約

| 制約 | PDF 記載箇所 | vqx での実装 |
|------|-------------|-------------|
| パブリッククラウドではトークン必須 | Profile セクション | `Profile.validate()` で警告 |
| username/password は Edge サーバーのみ | Profile セクション注記 | プロファイル作成時に注意表示 |
| namespace は token と併用不可 | Profile セクション注記 | `VqxError::NamespaceWithToken` エラー |
| password 指定時は token より優先 | Command Line Options | `CliOptions::to_args()` で実装 |

### 非推奨コマンド

| コマンド | 代替 | PDF 記載 |
|----------|------|----------|
| `execute` | `run procedure` | 「deprecated in favor of the run procedure command as of release 1.37」 |

## 設定ファイルのセキュリティ

### 推奨パーミッション

```bash
# 設定ディレクトリ
chmod 700 ~/.config/vqx/

# プロファイルファイル（機密情報を含む可能性）
chmod 600 ~/.config/vqx/profiles.toml

# 設定ファイル（機密情報なし）
chmod 644 ~/.config/vqx/config.toml
```

### gitignore 推奨設定

プロジェクトで vqx 設定を使用する場合:

```gitignore
# vqx 設定（認証情報を含む可能性）
.vqx/
profiles.toml

# バックアップディレクトリ
.vqx-backups/
```

## コンプライアンス考慮事項

- 認証情報は可能な限りシステムキーリングに保存
- プレーンテキスト保存時は警告を表示
- 全ての操作はオプションで監査ログ可能
- 破壊的操作は確認とバックアップを強制可能
