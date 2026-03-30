# Contributing to TFKosmos

TFKosmosへのコントリビューションを歓迎します。

## クイックスタート

```bash
# リポジトリをクローン
git clone https://github.com/masayaWada/TFKosmos.git
cd TFKosmos

# バックエンド + フロントエンドを起動
make dev
```

## 開発環境要件

- **Rust**: stable（`rustup` でインストール）
- **Node.js**: v18+
- **npm**: v9+

## ビルド・テスト

```bash
make test           # 全テスト実行
make test-backend   # バックエンドのみ
make test-frontend  # フロントエンドのみ
make test-e2e       # E2Eテスト
```

## 新リソーススキャナーの追加

CLAUDE.mdの「新リソーススキャナーの追加パターン（10ステップ）」を参照してください。

## コミット規約

[Conventional Commits](https://www.conventionalcommits.org/) に準拠します。
詳細は `.claude/rules/commit-strategy.md` を参照してください。

## 詳細ガイド

詳しいコントリビューション手順は [docs/03_開発ガイド/コントリビューションガイド.md](docs/03_開発ガイド/コントリビューションガイド.md) を参照してください。
