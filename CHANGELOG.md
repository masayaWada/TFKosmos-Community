# 変更履歴

TFKosmosの主要な変更点をこのファイルに記録します。

フォーマットは [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) に基づき、
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) に準拠しています。

## [1.0.0] - 2026-03-29

### 追加
- React ErrorBoundaryコンポーネント：レンダリングエラー時のアプリ全体クラッシュを防止
- セキュリティヘッダーミドルウェア（X-Content-Type-Options, X-Frame-Options, X-XSS-Protection, Referrer-Policy, Permissions-Policy, Cache-Control）を全APIレスポンスに適用
- `ConnectionService`, `GenerationService`, `ResourceService`, `DependencyService` モジュールにDocコメントを追加

### 変更
- `ResourcesPage`: イベントハンドラとカラム定義に `useCallback` / `useMemo` を適用しパフォーマンスを改善
- フロントエンド依存関係: monaco-editorの中程度のXSS脆弱性を修正するため `dompurify` オーバーライドを `^3.3.3` に追加
- CI: バックエンドカバレッジ閾値を80%に設定し、SDKラッパー・main.rsを`--ignore-filename-regex`で除外

### 修正
- フロントエンド: npm auditの脆弱性7件を解消（flatted プロトタイプ汚染, minimatch ReDoS, picomatch ReDoS, rollup パストラバーサル）
- フロントエンド: `ErrorBoundary.test.tsx` の `ThrowError` テストコンポーネントにおけるJSX型エラーを修正
- `terraform.rs`: `render_resource()`で`functions`/`dynamodb_tables`/`internet_gateways`等12リソースタイプのテンプレートコンテキスト変数が正しくマッピングされていなかったバグを修正

### セキュリティ
- `audit_service.rs` のパストラバーサル脆弱性を修正（`validated_log_dir()` による安全なパス検証を導入）
- `audit_service.rs` の `log_file_path()` を検証済みベースディレクトリを受け取る設計に変更
- `config_management_service.rs` の `sanitize_filename()` を `Option<String>` 返却に変更し、不正ファイル名を確実に拒否

### テスト
- バックエンドテストカバレッジを74.45%から87.88%に改善（423→515テスト、+92テスト追加）
- カバレッジ0%だったAPIルート5ファイルにテスト追加: `config.rs`(7), `drift.rs`(5), `export.rs`(4), `audit.rs`(2), `security_headers.rs`(1)
- サービス層テスト拡充: `scan_service.rs`, `config.rs`, `audit_middleware.rs`, `resource_service.rs`, `zip_service.rs`, `export_service.rs`
- インフラ層テスト拡充: `vpc_scanner.rs`, `azure/scanner.rs`, `terraform.rs`, `rds_scanner.rs`, `cloudwatch_scanner.rs`, `dynamodb_scanner.rs`
- APIルートテスト拡充: `scan.rs`(SSEストリーム), `connection.rs`(Azure失敗パス), `generate.rs`(実データgenerate)

### その他
- CI: Releaseワークフローの強化（バージョン検証、Tauri設定の動的バージョン同期、バンドルターゲットの明示指定）
- リリース手順書を追加（`docs/05_運用サポート/リリース手順.md`）
- コード品質: 全domainファイルにdead_code警告抑制を追加、Clippy警告を全面解消
- テスト: フロントエンドテストのThemeContext/i18nモック整備（Navigation, ConnectionPage, ScanPage, GeneratePage）

## [0.6.0] - 2026-03-29

### 追加
- Azure追加リソーススキャン: 仮想マシン, 仮想ネットワーク, NSG, ストレージアカウント, SQLデータベース
- AWS追加リソーススキャン: EC2インスタンス, VPC, サブネット, ルートテーブル, セキュリティグループ, ネットワークACL, S3バケット, RDSインスタンス
- 設定管理API (`/api/config`): 接続設定のCRUD操作（JSON/TOML対応）
- エクスポートAPI (`/api/export`): スキャン済みリソースのCSV/JSONエクスポート
- リソースクエリ言語（`==` / `!=` 演算子対応）
- `terraform validate` によるTerraform検証（プレビューエンドポイント）
- `@xyflow/react` による依存関係グラフの可視化

### 変更
- Terraform生成の改善: コンテキスト/リソース名の処理、Azure VM OS分岐ロジックを改善

## [0.5.0] - 2026-03-28

### 追加
- 型安全なプロバイダー選択のための `CloudProvider` 列挙型と `ProviderConfig` 列挙型
- mockall対応のAWS EC2/VPC/S3/RDSスキャナートレイト
- mockall対応のAzure Compute/Network/Storage/SQLスキャナートレイト

### 変更
- フロントエンド本番コードから全ての `any` 型を排除

## [0.4.0] - 2026-03-28

### 追加
- `Arc<dyn Trait>` パターンによる全サービスの依存性注入
- `axum-test` を使用した統合テスト
- テスト用 `ScanService::insert_test_scan_data` ヘルパー

### 変更
- 全サービス層のロギングを `tracing` に統一

## [0.3.0] - 2026-03-26

### 追加
- Azure接続・スキャン用POSTエンドポイント
- AWSプロファイル名バリデーション
- レート制限（`tower_governor`: IPあたり10リクエスト/秒, バースト5）
- ファイルサービスのパストラバーサル防止
- セキュリティヘッダーの検証

### 修正
- CORS: 本番モードで `TFKOSMOS_CORS_ORIGINS` によるオリジン制限を適用

## [0.2.0] - 2026-01-05

### 追加
- フロントエンドユニットテスト（Vitest + Testing Library）
- バックエンド統合テスト（cargo test）
- PlaywrightとモックサーバーによるE2Eテスト
- カバレッジレポート（`cargo-llvm-cov`, `vitest --coverage`）

## [0.1.0] - 2025-12-15

### 追加
- 初回リリース
- AWS IAMリソーススキャン（ユーザー, グループ, ロール, ポリシー, アタッチメント）
- Azure IAMリソーススキャン（ロール定義, ロール割り当て）
- Jinja2テンプレートによるTerraform `.tf` ファイル生成
- インポートスクリプト生成（`.sh` / `.ps1`）
- テンプレート管理API（CRUD, バリデーション）
- Monaco Editorコードプレビュー付きReactフロントエンド
- Tauriデスクトップアプリケーションラッパー
- `/swagger-ui` でのSwagger UI公開（開発環境のみ）

[1.0.0]: https://github.com/masayaWada/TFKosmos/compare/v0.6.0...v1.0.0
[0.6.0]: https://github.com/masayaWada/TFKosmos/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/masayaWada/TFKosmos/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/masayaWada/TFKosmos/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/masayaWada/TFKosmos/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/masayaWada/TFKosmos/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/masayaWada/TFKosmos/releases/tag/v0.1.0
