# エラーハンドリング・エッジケーステスト実行手順

このファイルは、TFKosmosのエラーハンドリングとエッジケースをテストするためのガイドです。

## 前提条件

- [ ] バックエンドが起動している（http://localhost:8000）※一部のテストでは停止も必要
- [ ] フロントエンドが起動している（http://localhost:5173）
- [ ] AWS認証情報が設定されている（`~/.aws/credentials`）

## テスト実行方法

Claudeに以下のように指示してください：

```
このファイルのテストケースを順番に実行してください。
各テストケースの実行結果をチェックボックスで記録してください。
```

---

## 8.1 ネットワークエラー

### テストケース 8.1.1: バックエンド停止時のAPI呼び出し

**目的**: バックエンドが停止している状態でAPI呼び出しを行った際に、適切なエラーメッセージが表示されることを確認する

**手順**:
1. **準備**: バックエンドを停止する（`pkill -f cargo` または手動でCtrl+C）
2. http://localhost:5173/connection にアクセス
3. ページスナップショットを取得
4. AWSタブが選択されていることを確認
5. プロファイル入力欄に "default" を入力
6. リージョン入力欄に "us-east-1" を入力
7. 「接続をテスト」ボタンをクリック
8. エラーメッセージを確認
9. **後処理**: バックエンドを再起動する

**期待結果**:
- 「接続エラー」または「バックエンドに接続できません」などのメッセージが表示される
- ユーザーフレンドリーなエラーメッセージである
- アプリケーションがクラッシュしない

**実行**:
```
1. バックエンドを手動で停止
2. mcp__playwright__browser_navigate({ url: "http://localhost:5173/connection" })
3. mcp__playwright__browser_snapshot({})
4. AWSタブがアクティブであることを確認
5. mcp__playwright__browser_type({ element: "Profile input", ref: "...", text: "default" })
6. mcp__playwright__browser_type({ element: "Region input", ref: "...", text: "us-east-1" })
7. mcp__playwright__browser_click({ element: "Test connection button", ref: "..." })
8. mcp__playwright__browser_wait_for({ time: 3 })
9. mcp__playwright__browser_snapshot({})
10. エラーメッセージの存在を確認
11. バックエンドを再起動
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 8.1.2: APIタイムアウト

**目的**: APIリクエストがタイムアウトした場合に適切なエラーメッセージが表示されることを確認する

**手順**:
1. http://localhost:5173/scan にアクセス
2. ページスナップショットを取得
3. AWSプロバイダーを選択
4. 無効なプロファイル名（存在しないプロファイル）を入力
5. スキャンを実行
6. タイムアウトエラーまたはエラーメッセージを確認

**期待結果**:
- タイムアウトエラーメッセージが表示される
- または、認証エラーメッセージが表示される
- アプリケーションがフリーズしない

**実行**:
```
1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/scan" })
2. mcp__playwright__browser_snapshot({})
3. AWSラジオボタンをクリック（すでに選択されている場合はスキップ）
4. mcp__playwright__browser_type({ element: "Profile input", ref: "...", text: "nonexistent-profile-12345" })
5. mcp__playwright__browser_click({ element: "Execute scan button", ref: "..." })
6. mcp__playwright__browser_wait_for({ time: 10 })
7. mcp__playwright__browser_snapshot({})
8. エラーメッセージの存在を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

## 8.2 バリデーションエラー

### テストケース 8.2.1: 不正なクエリ構文

**目的**: リソース一覧画面で不正なクエリ構文を入力した場合に、構文エラーメッセージが表示されることを確認する

**前提条件**:
- 事前にスキャンが完了している必要があります
- スキャンIDを取得しておく

**手順**:
1. リソース一覧画面に遷移（http://localhost:5173/resources/{scanId}）
2. ページスナップショットを取得
3. 高度なクエリモードに切り替え（該当する場合）
4. 不正なクエリ構文を入力（例: `user_name ==` ←値が欠落）
5. 検索ボタンをクリック
6. エラーメッセージを確認

**期待結果**:
- 「クエリ構文エラー」または「不正なクエリ」などのメッセージが表示される
- エラー箇所が明示される（理想的には）
- アプリケーションがクラッシュしない

**実行**:
```
注意: このテストを実行する前に、7.1のAWSスキャンフローを実行してスキャンIDを取得してください。

1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/resources/{scanId}" })
2. mcp__playwright__browser_snapshot({})
3. 「高度なクエリ」ラジオボタンをクリック（存在する場合）
4. mcp__playwright__browser_type({ element: "Query input", ref: "...", text: "user_name ==" })
5. mcp__playwright__browser_click({ element: "Search button", ref: "..." })
6. mcp__playwright__browser_wait_for({ time: 2 })
7. mcp__playwright__browser_snapshot({})
8. エラーメッセージの存在を確認
```

**結果**: [ ] 成功 / [ ] 失敗 / [ ] スキップ（機能未実装）

---

### テストケース 8.2.2: 不正なテンプレート構文

**目的**: テンプレート管理画面で不正なJinja2構文を入力した場合に、バリデーションエラーが表示されることを確認する

**手順**:
1. テンプレート管理画面に遷移（http://localhost:5173/templates）
2. ページスナップショットを取得
3. AWSテンプレートを選択（例: aws_iam_user）
4. テンプレートの一部を不正な構文に変更（例: `{% for user in users` ←endforが欠落）
5. バリデーションの実行を待つ（デバウンス後、自動実行される場合）
6. エラーメッセージを確認

**期待結果**:
- 「バリデーションエラー」または「テンプレート構文エラー」が表示される
- エラーの行番号が表示される（理想的には）
- エディタ上にエラーマーカーが表示される（Monaco Editorの機能）

**実行**:
```
1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/templates" })
2. mcp__playwright__browser_snapshot({})
3. "aws_iam_user" テンプレートを選択
4. mcp__playwright__browser_wait_for({ time: 1 })
5. mcp__playwright__browser_snapshot({})
6. エディタ内のコードを編集（不正な構文を挿入）
   - 既存のテンプレート内容の一部を削除して不正な状態にする
   - 例: {% for user in users %} の endfor を削除
7. mcp__playwright__browser_wait_for({ time: 3 })（バリデーションのデバウンスを待つ）
8. mcp__playwright__browser_snapshot({})
9. バリデーションエラーメッセージの存在を確認
```

**結果**: [ ] 成功 / [ ] 失敗 / [ ] スキップ（機能未実装）

---

## 8.3 データなしの場合

### テストケース 8.3.1: スキャン結果が空

**目的**: スキャン実行後、リソースが1つも見つからなかった場合に適切なメッセージが表示されることを確認する

**手順**:
1. スキャン画面に遷移（http://localhost:5173/scan）
2. AWSプロバイダーを選択
3. 存在しないリージョンまたはリソースが存在しないアカウントでスキャンを実行
4. スキャン完了を待つ
5. リソース一覧画面でメッセージを確認

**期待結果**:
- 「スキャン結果が見つかりませんでした」または「リソースがありません」などのメッセージが表示される
- 空のテーブルではなく、わかりやすいメッセージが表示される
- スキャン自体は成功ステータスである

**実行**:
```
注意: このテストは実際のAWS環境に依存します。リソースが存在しないアカウント/リージョンが必要です。

1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/scan" })
2. mcp__playwright__browser_snapshot({})
3. AWSラジオボタンをクリック
4. mcp__playwright__browser_type({ element: "Profile input", ref: "...", text: "empty-test-profile" })
5. mcp__playwright__browser_type({ element: "Region input", ref: "...", text: "ap-northeast-3" })
6. スキャン対象のチェックボックスを1つだけ選択
7. mcp__playwright__browser_click({ element: "Execute scan button", ref: "..." })
8. mcp__playwright__browser_wait_for({ text: "スキャン完了", time: 60 })
9. mcp__playwright__browser_snapshot({})
10. 「リソースが見つかりませんでした」などのメッセージを確認
```

**結果**: [ ] 成功 / [ ] 失敗 / [ ] スキップ（テスト環境不足）

---

### テストケース 8.3.2: 依存関係グラフのデータなし

**目的**: 依存関係グラフのデータが存在しない場合に、適切なメッセージが表示されることを確認する

**前提条件**:
- スキャン結果が存在するが、依存関係データ（attachments等）が存在しない状態
- または、依存関係グラフ機能が未実装の場合

**手順**:
1. リソース一覧画面に遷移（http://localhost:5173/resources/{scanId}）
2. ページスナップショットを取得
3. 「依存関係」タブまたはボタンをクリック
4. メッセージを確認

**期待結果**:
- 「依存関係データがありません」または「依存関係グラフを表示できません」などのメッセージが表示される
- 空のグラフではなく、わかりやすいメッセージが表示される

**実行**:
```
注意: このテストは依存関係グラフ機能が実装されている場合のみ実行可能です。

1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/resources/{scanId}" })
2. mcp__playwright__browser_snapshot({})
3. 「依存関係」タブ/ボタンを探す
4. 依存関係グラフを表示
5. mcp__playwright__browser_wait_for({ time: 2 })
6. mcp__playwright__browser_snapshot({})
7. 「データがありません」メッセージの存在を確認
```

**結果**: [ ] 成功 / [ ] 失敗 / [ ] スキップ（機能未実装）

---

## テスト完了チェックリスト

### 8.1 ネットワークエラー
- [ ] 8.1.1: バックエンド停止時のAPI呼び出し
- [ ] 8.1.2: APIタイムアウト

### 8.2 バリデーションエラー
- [ ] 8.2.1: 不正なクエリ構文
- [ ] 8.2.2: 不正なテンプレート構文

### 8.3 データなしの場合
- [ ] 8.3.1: スキャン結果が空
- [ ] 8.3.2: 依存関係グラフのデータなし

---

## 注意事項

1. **ネットワークエラーテスト（8.1）**は実際にバックエンドを停止する必要があるため、他のテストへの影響に注意してください。
2. **バリデーションエラーテスト（8.2）**は、該当機能が実装されている場合のみ実行可能です。
3. **データなしテスト（8.3）**は、適切なテスト環境（空のAWSアカウントなど）が必要な場合があります。
4. テスト実行前に、必ずバックエンドとフロントエンドが正常に起動していることを確認してください。

---

最終更新: 2026-01-03
