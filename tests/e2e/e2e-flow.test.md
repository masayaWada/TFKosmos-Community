# E2Eフローテスト実行手順

このファイルは、TFKosmosの主要な業務フロー全体をエンドツーエンドでテストするためのガイドです。

## 前提条件

- [ ] バックエンドが起動している（http://localhost:8000）
- [ ] フロントエンドが起動している（http://localhost:5173）
- [ ] AWS認証情報が設定されている（`~/.aws/credentials`）
- [ ] Azure認証情報が設定されている（`az login`）※Azureテストを実行する場合

## テスト実行方法

Claudeに以下のように指示してください：

```
このファイルのテストケースを順番に実行してください。
各テストケースの実行結果をチェックボックスで記録してください。
```

---

## 7.1 AWSスキャン〜生成フロー

このフローテストでは、AWS IAMリソースのスキャンから、Terraformコード生成、ダウンロードまでの一連の流れを確認します。

### テストケース 7.1.1: AWS接続テスト成功

**目的**: AWS接続設定が正しく動作し、接続テストが成功することを確認する

**手順**:
1. http://localhost:5173/connection にアクセス
2. ページスナップショットを取得
3. AWSタブが選択されていることを確認
4. プロファイル入力欄に "default" を入力
5. リージョン入力欄に "us-east-1" を入力
6. 「接続をテスト」ボタンをクリック
7. 処理完了を待つ（最大30秒）
8. 成功メッセージを確認

**期待結果**:
- 「接続成功: Account ID XXXX」のメッセージが表示される
- エラーメッセージが表示されない

**実行**:
```
1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/connection" })
2. mcp__playwright__browser_snapshot({})
3. AWSタブがアクティブであることを確認
4. mcp__playwright__browser_type({ element: "Profile input", ref: "...", text: "default" })
5. mcp__playwright__browser_type({ element: "Region input", ref: "...", text: "us-east-1" })
6. mcp__playwright__browser_click({ element: "Test connection button", ref: "..." })
7. mcp__playwright__browser_wait_for({ text: "接続成功", time: 30 })
8. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.1.2: AWSスキャン実行・完了

**目的**: AWSスキャンが正しく実行され、リソース一覧画面に自動遷移することを確認する

**手順**:
1. ナビゲーションバーの「スキャン」リンクをクリック
2. スキャン設定画面に遷移
3. AWSが選択されていることを確認
4. プロファイルに "default" を入力
5. スキャン対象のデフォルト選択を確認（Users, Groups, Attachments）
6. 「スキャンを実行」ボタンをクリック
7. プログレスバーの表示を確認
8. スキャン完了を待つ（最大60秒）
9. リソース一覧画面への自動遷移を確認

**期待結果**:
- プログレスバーが表示される
- 「スキャン完了」メッセージが表示される
- リソース一覧画面（http://localhost:5173/resources/:scanId）に自動遷移する
- スキャンIDがURLに含まれる

**実行**:
```
1. mcp__playwright__browser_click({ element: "Scan navigation link", ref: "..." })
2. mcp__playwright__browser_snapshot({})
3. AWSラジオボタンが選択されていることを確認
4. mcp__playwright__browser_type({ element: "Profile input", ref: "...", text: "default" })
5. スキャン対象チェックボックスを確認（Users, Groups, Attachmentsがチェック済み）
6. mcp__playwright__browser_click({ element: "Execute scan button", ref: "..." })
7. mcp__playwright__browser_snapshot({})（プログレスバー確認）
8. mcp__playwright__browser_wait_for({ text: "スキャン完了", time: 60 })
9. URLに /resources/ が含まれることを確認
10. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.1.3: リソース選択

**目的**: リソース一覧画面で複数のリソースを選択できることを確認する

**手順**:
1. 前のテストの続きから（リソース一覧画面に滞在）
2. Usersタブがアクティブであることを確認
3. 最初のユーザーのチェックボックスをクリック
4. 2番目のユーザーのチェックボックスをクリック
5. 選択件数が「2件選択中」と表示されることを確認
6. Groupsタブをクリック
7. 最初のグループのチェックボックスをクリック
8. 選択件数が「3件選択中」と表示されることを確認

**期待結果**:
- 各リソースのチェックボックスがチェック状態になる
- 選択件数が正しく表示される
- タブを切り替えても選択状態が保持される

**実行**:
```
1. Usersタブがアクティブであることを確認
2. mcp__playwright__browser_snapshot({})
3. mcp__playwright__browser_click({ element: "First user checkbox", ref: "..." })
4. mcp__playwright__browser_click({ element: "Second user checkbox", ref: "..." })
5. 選択件数「2件選択中」を確認
6. mcp__playwright__browser_click({ element: "Groups tab", ref: "..." })
7. mcp__playwright__browser_snapshot({})
8. mcp__playwright__browser_click({ element: "First group checkbox", ref: "..." })
9. 選択件数「3件選択中」を確認
10. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.1.4: 生成画面へ遷移

**目的**: 選択したリソースが保持された状態で生成画面に遷移することを確認する

**手順**:
1. 前のテストの続きから（リソース一覧画面に滞在）
2. 「生成ページへ」ボタンをクリック
3. 生成画面（http://localhost:5173/generate/:scanId）に遷移
4. スキャンIDがURLに含まれることを確認
5. 選択リソース件数「3件」が表示されることを確認

**期待結果**:
- 生成画面に遷移する
- URLにスキャンIDが含まれる
- 選択したリソース件数が正しく表示される

**実行**:
```
1. mcp__playwright__browser_click({ element: "Navigate to generate page button", ref: "..." })
2. URLに /generate/ が含まれることを確認
3. mcp__playwright__browser_snapshot({})
4. 「選択したリソース: 3件」などの表示を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.1.5: Terraform生成実行

**目的**: Terraformコードの生成が成功することを確認する

**手順**:
1. 前のテストの続きから（生成画面に滞在）
2. 出力パスがデフォルト値（./terraform-output）であることを確認
3. ファイル分割ルールが選択されていることを確認
4. 「生成を実行」ボタンをクリック
5. ローディング状態を確認
6. 生成完了を待つ（最大30秒）
7. 成功メッセージを確認
8. コードプレビューが表示されることを確認

**期待結果**:
- 「Terraformコードの生成が完了しました」のメッセージが表示される
- コードプレビューセクションが表示される
- ファイルタブが表示される
- ダウンロードボタンが表示される

**実行**:
```
1. 出力パスが "./terraform-output" であることを確認
2. mcp__playwright__browser_snapshot({})
3. mcp__playwright__browser_click({ element: "Execute generation button", ref: "..." })
4. ローディングスピナーが表示されることを確認
5. mcp__playwright__browser_wait_for({ text: "生成が完了しました", time: 30 })
6. mcp__playwright__browser_snapshot({})
7. コードプレビューセクションを確認
8. ダウンロードボタンを確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.1.6: ZIPダウンロード

**目的**: 生成されたTerraformコードをZIPファイルとしてダウンロードできることを確認する

**手順**:
1. 前のテストの続きから（生成画面に滞在）
2. 「ZIPダウンロード」ボタンをクリック
3. ダウンロード成功メッセージを確認
4. （ブラウザの動作確認のため、実際のファイルダウンロードは目視で確認）

**期待結果**:
- 「ZIPファイルのダウンロードが完了しました」のメッセージが表示される
- エラーメッセージが表示されない

**実行**:
```
1. mcp__playwright__browser_click({ element: "Download ZIP button", ref: "..." })
2. mcp__playwright__browser_snapshot({})
3. 成功メッセージを確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

## 7.2 Azureスキャン〜生成フロー

このフローテストでは、Azure IAMリソースのスキャンから、Terraformコード生成までの一連の流れを確認します。

### テストケース 7.2.1: Azure接続テスト成功

**目的**: Azure接続設定が正しく動作し、接続テストが成功することを確認する

**手順**:
1. http://localhost:5173/connection にアクセス
2. Azureタブをクリック
3. 認証方式ドロップダウンで「Azure CLI」を選択
4. 「接続をテスト」ボタンをクリック
5. 処理完了を待つ（最大30秒）
6. 成功メッセージを確認

**期待結果**:
- 「接続成功」のメッセージが表示される
- エラーメッセージが表示されない

**実行**:
```
1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/connection" })
2. mcp__playwright__browser_click({ element: "Azure tab", ref: "..." })
3. mcp__playwright__browser_snapshot({})
4. mcp__playwright__browser_select_option({ element: "Auth method dropdown", ref: "...", values: ["cli"] })
5. mcp__playwright__browser_click({ element: "Test connection button", ref: "..." })
6. mcp__playwright__browser_wait_for({ text: "接続成功", time: 30 })
7. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.2.2: サブスクリプション選択

**目的**: Azureスキャン設定画面でサブスクリプションを選択できることを確認する

**手順**:
1. ナビゲーションバーの「スキャン」リンクをクリック
2. Azureラジオボタンをクリック
3. スコープタイプが「subscription」であることを確認
4. サブスクリプションドロップダウンが表示されることを確認
5. サブスクリプション一覧から最初のサブスクリプションを選択

**期待結果**:
- サブスクリプションドロップダウンが表示される
- サブスクリプション一覧が読み込まれる
- サブスクリプションを選択できる

**実行**:
```
1. mcp__playwright__browser_click({ element: "Scan navigation link", ref: "..." })
2. mcp__playwright__browser_click({ element: "Azure radio button", ref: "..." })
3. mcp__playwright__browser_snapshot({})
4. スコープタイプが "subscription" であることを確認
5. mcp__playwright__browser_select_option({ element: "Subscription dropdown", ref: "...", values: ["最初のサブスクリプションID"] })
6. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.2.3: Azureスキャン実行・完了

**目的**: Azureスキャンが正しく実行され、リソース一覧画面に自動遷移することを確認する

**手順**:
1. 前のテストの続きから（スキャン設定画面に滞在）
2. スキャン対象のデフォルト選択を確認（Role Definitions, Role Assignments）
3. 「スキャンを実行」ボタンをクリック
4. プログレスバーの表示を確認
5. スキャン完了を待つ（最大60秒）
6. リソース一覧画面への自動遷移を確認

**期待結果**:
- プログレスバーが表示される
- 「スキャン完了」メッセージが表示される
- リソース一覧画面（http://localhost:5173/resources/:scanId）に自動遷移する

**実行**:
```
1. スキャン対象チェックボックスを確認
2. mcp__playwright__browser_click({ element: "Execute scan button", ref: "..." })
3. mcp__playwright__browser_snapshot({})（プログレスバー確認）
4. mcp__playwright__browser_wait_for({ text: "スキャン完了", time: 60 })
5. URLに /resources/ が含まれることを確認
6. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.2.4: リソース選択

**目的**: リソース一覧画面で複数のAzureリソースを選択できることを確認する

**手順**:
1. 前のテストの続きから（リソース一覧画面に滞在）
2. Role Assignmentsタブがアクティブであることを確認
3. 最初のロール割り当てのチェックボックスをクリック
4. 2番目のロール割り当てのチェックボックスをクリック
5. 選択件数が「2件選択中」と表示されることを確認

**期待結果**:
- 各リソースのチェックボックスがチェック状態になる
- 選択件数が正しく表示される

**実行**:
```
1. Role Assignmentsタブがアクティブであることを確認
2. mcp__playwright__browser_snapshot({})
3. mcp__playwright__browser_click({ element: "First role assignment checkbox", ref: "..." })
4. mcp__playwright__browser_click({ element: "Second role assignment checkbox", ref: "..." })
5. 選択件数「2件選択中」を確認
6. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.2.5: 生成画面へ遷移

**目的**: 選択したリソースが保持された状態で生成画面に遷移することを確認する

**手順**:
1. 前のテストの続きから（リソース一覧画面に滞在）
2. 「生成ページへ」ボタンをクリック
3. 生成画面に遷移
4. 選択リソース件数「2件」が表示されることを確認

**期待結果**:
- 生成画面に遷移する
- 選択したリソース件数が正しく表示される

**実行**:
```
1. mcp__playwright__browser_click({ element: "Navigate to generate page button", ref: "..." })
2. URLに /generate/ が含まれることを確認
3. mcp__playwright__browser_snapshot({})
4. 「選択したリソース: 2件」などの表示を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.2.6: Terraform生成実行

**目的**: AzureリソースのTerraformコード生成が成功することを確認する

**手順**:
1. 前のテストの続きから（生成画面に滞在）
2. 「生成を実行」ボタンをクリック
3. ローディング状態を確認
4. 生成完了を待つ（最大30秒）
5. 成功メッセージを確認
6. コードプレビューが表示されることを確認

**期待結果**:
- 「Terraformコードの生成が完了しました」のメッセージが表示される
- コードプレビューセクションが表示される

**実行**:
```
1. mcp__playwright__browser_click({ element: "Execute generation button", ref: "..." })
2. ローディングスピナーが表示されることを確認
3. mcp__playwright__browser_wait_for({ text: "生成が完了しました", time: 30 })
4. mcp__playwright__browser_snapshot({})
5. コードプレビューセクションを確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

## 7.3 テンプレートカスタマイズフロー

このフローテストでは、テンプレートの選択、編集、プレビュー、保存、復元の一連の流れを確認します。

### テストケース 7.3.1: テンプレート選択

**目的**: テンプレート管理画面でテンプレートを選択し、エディタに内容が表示されることを確認する

**手順**:
1. http://localhost:5173/templates にアクセス
2. ページスナップショットを取得
3. 左カラムのテンプレート一覧を確認
4. AWS/user.tf テンプレートをクリック
5. エディタに内容が表示されることを確認
6. テンプレートの状態（デフォルト/カスタム）を確認

**期待結果**:
- テンプレート一覧が表示される
- 選択したテンプレートがハイライトされる
- エディタにテンプレート内容が表示される
- デフォルト状態であることが表示される

**実行**:
```
1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/templates" })
2. mcp__playwright__browser_snapshot({})
3. テンプレート一覧を確認
4. mcp__playwright__browser_click({ element: "AWS/user.tf template", ref: "..." })
5. mcp__playwright__browser_snapshot({})
6. エディタに内容が表示されていることを確認
7. "デフォルト" 表示を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.3.2: テンプレート編集

**目的**: テンプレートの内容を編集できることを確認する

**手順**:
1. 前のテストの続きから（テンプレート管理画面に滞在）
2. エディタの内容の末尾にコメントを追加
   - 例: `# Custom comment added by E2E test`
3. 編集内容が反映されることを確認

**期待結果**:
- エディタに入力した内容が表示される
- リアルタイムバリデーションが動作する（エラーがない場合）

**実行**:
```
1. エディタの末尾をクリック
2. mcp__playwright__browser_type({ element: "Editor", ref: "...", text: "\n# Custom comment added by E2E test" })
3. mcp__playwright__browser_snapshot({})
4. バリデーションエラーが表示されないことを確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.3.3: プレビュー確認

**目的**: プレビュー機能で変更がレンダリング結果に反映されることを確認する

**手順**:
1. 前のテストの続きから（テンプレート管理画面に滞在）
2. 「プレビュー」ボタンをクリック
3. プレビュー結果が表示されることを確認
4. 追加したコメントがプレビューに含まれることを確認

**期待結果**:
- プレビューセクションが表示される
- レンダリング結果に編集内容が反映される
- エラーが表示されない

**実行**:
```
1. mcp__playwright__browser_click({ element: "Preview button", ref: "..." })
2. mcp__playwright__browser_wait_for({ text: "プレビュー", time: 10 })
3. mcp__playwright__browser_snapshot({})
4. プレビュー結果に "# Custom comment added by E2E test" が含まれることを確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.3.4: 保存

**目的**: テンプレートの変更を保存できることを確認する

**手順**:
1. 前のテストの続きから（テンプレート管理画面に滞在）
2. 「保存」ボタンをクリック
3. 保存完了を待つ（最大10秒）
4. 成功メッセージを確認
5. テンプレートの状態が「カスタム」に変わることを確認
6. 「デフォルトに復元」ボタンが表示されることを確認

**期待結果**:
- 「テンプレートを保存しました」のメッセージが表示される
- テンプレートが「カスタム」表示に変わる
- 「デフォルトに復元」ボタンが表示される

**実行**:
```
1. mcp__playwright__browser_click({ element: "Save button", ref: "..." })
2. mcp__playwright__browser_wait_for({ text: "保存しました", time: 10 })
3. mcp__playwright__browser_snapshot({})
4. "カスタム" 表示を確認
5. "デフォルトに復元" ボタンを確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 7.3.5: デフォルトに復元

**目的**: カスタムテンプレートをデフォルトに復元できることを確認する

**手順**:
1. 前のテストの続きから（テンプレート管理画面に滞在）
2. 「デフォルトに復元」ボタンをクリック
3. 確認ダイアログが表示されることを確認（もし実装されている場合）
4. 復元を確認
5. 復元完了を待つ（最大10秒）
6. 成功メッセージを確認
7. エディタの内容がデフォルトに戻ることを確認
8. テンプレートの状態が「デフォルト」に戻ることを確認
9. 追加したコメントが削除されていることを確認

**期待結果**:
- 確認ダイアログが表示される（もし実装されている場合）
- 「デフォルトに復元しました」のメッセージが表示される
- エディタの内容がデフォルトに戻る
- テンプレートが「デフォルト」表示に変わる
- カスタム変更が削除される

**実行**:
```
1. mcp__playwright__browser_click({ element: "Restore to default button", ref: "..." })
2. （確認ダイアログがある場合）mcp__playwright__browser_handle_dialog({ accept: true })
3. mcp__playwright__browser_wait_for({ text: "復元しました", time: 10 })
4. mcp__playwright__browser_snapshot({})
5. エディタ内容を確認（コメントが削除されている）
6. "デフォルト" 表示を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

## テスト完了

すべてのテストケースの実行が完了しました。

### 結果サマリー

- **7.1 AWSスキャン〜生成フロー**: [ ] 成功 / [ ] 失敗
  - 7.1.1 AWS接続テスト成功: [ ] ✅ / [ ] ❌
  - 7.1.2 AWSスキャン実行・完了: [ ] ✅ / [ ] ❌
  - 7.1.3 リソース選択: [ ] ✅ / [ ] ❌
  - 7.1.4 生成画面へ遷移: [ ] ✅ / [ ] ❌
  - 7.1.5 Terraform生成実行: [ ] ✅ / [ ] ❌
  - 7.1.6 ZIPダウンロード: [ ] ✅ / [ ] ❌

- **7.2 Azureスキャン〜生成フロー**: [ ] 成功 / [ ] 失敗
  - 7.2.1 Azure接続テスト成功: [ ] ✅ / [ ] ❌
  - 7.2.2 サブスクリプション選択: [ ] ✅ / [ ] ❌
  - 7.2.3 Azureスキャン実行・完了: [ ] ✅ / [ ] ❌
  - 7.2.4 リソース選択: [ ] ✅ / [ ] ❌
  - 7.2.5 生成画面へ遷移: [ ] ✅ / [ ] ❌
  - 7.2.6 Terraform生成実行: [ ] ✅ / [ ] ❌

- **7.3 テンプレートカスタマイズフロー**: [ ] 成功 / [ ] 失敗
  - 7.3.1 テンプレート選択: [ ] ✅ / [ ] ❌
  - 7.3.2 テンプレート編集: [ ] ✅ / [ ] ❌
  - 7.3.3 プレビュー確認: [ ] ✅ / [ ] ❌
  - 7.3.4 保存: [ ] ✅ / [ ] ❌
  - 7.3.5 デフォルトに復元: [ ] ✅ / [ ] ❌

### 次のステップ

テストに失敗した場合は、以下を確認してください：

1. バックエンド/フロントエンドが起動しているか
2. 認証情報が正しく設定されているか
3. ブラウザのコンソールエラーを確認
4. バックエンドのログを確認

---

最終更新: 2026-01-03
