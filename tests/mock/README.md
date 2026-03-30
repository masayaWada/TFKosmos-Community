# モックデータ

E2Eテストおよび開発時に使用するモックデータを格納しています。

## ファイル一覧

### スキャン結果

- `aws-scan-result.json` - AWS IAMスキャンの完了レスポンス
- `azure-scan-result.json` - Azure IAMスキャンの完了レスポンス

これらのファイルは、`/api/scan/:scan_id/status` エンドポイントのレスポンスと同じ構造を持ちます。

**構造:**
```json
{
  "scan_id": "スキャンID",
  "status": "completed | in_progress | failed",
  "progress": 0-100,
  "message": "ステータスメッセージ",
  "summary": {
    "リソースタイプ": 件数
  }
}
```

### リソース一覧

- `aws-resources.json` - AWSリソース（Users, Groups, Roles, Policies, Attachments）の一覧
- `azure-resources.json` - Azureリソース（Role Assignments, Role Definitions）の一覧

これらのファイルは、`/api/resources/:scan_id` エンドポイントのレスポンスと同じ構造を持ちます。

**構造:**
```json
{
  "resources": [
    {
      "type": "リソースタイプ",
      "name": "リソース名",
      // ... その他のプロパティ
    }
  ],
  "total": 総件数,
  "page": 現在のページ,
  "page_size": ページサイズ,
  "total_pages": 総ページ数,
  "provider": "aws | azure"
}
```

## モックデータの使用方法

### E2Eテストでの使用

Playwright MCPを使用したE2Eテストでは、以下のように実際のバックエンドを使用するか、モックデータを参考にテストデータを準備します。

```markdown
# テスト例

1. バックエンドを起動
2. モックデータに基づいて、スキャン結果を確認
3. リソース一覧画面で表示を検証
```

### 開発時のUIテスト

フロントエンド開発時に、バックエンドなしでUIをテストする場合は、モックデータを直接使用できます。

```typescript
// 例: モックデータの読み込み
import awsResources from '../tests/mock/aws-resources.json';
import azureResources from '../tests/mock/azure-resources.json';

// UIコンポーネントのテスト
const mockData = awsResources;
// ...
```

## モックデータの拡張

新しいテストケースや機能追加に伴い、モックデータを拡張する場合は以下のガイドラインに従ってください：

1. **実際のAPIレスポンスと同じ構造を維持する**
   - `backend/src/models/response.rs` の定義を参照

2. **多様なケースをカバーする**
   - 正常ケース、エッジケース、エラーケースを含める

3. **分かりやすい命名**
   - `<provider>-<resource-type>-<scenario>.json` の形式を推奨
   - 例: `aws-users-with-mfa.json`, `azure-role-assignments-empty.json`

4. **ドキュメントの更新**
   - 新しいモックデータファイルを追加したら、このREADMEを更新する

## 関連ドキュメント

- [E2Eテストガイド](../e2e/README.md)
- [テスト環境セットアップ](../start-test-env.sh)
- [バックエンドAPIドキュメント](../../docs/詳細設計書.md)
