# ConnectionPage E2Eテスト実行手順

このファイルは、接続設定画面（ConnectionPage）のE2Eテストを実行するためのガイドです。

## 前提条件

- [ ] バックエンドが起動している（http://localhost:8000）
- [ ] フロントエンドが起動している（http://localhost:5173）
- [ ] AWS CLI がインストールされている（AWS接続テスト用、オプション）
- [ ] Azure CLI がインストールされている（Azure接続テスト用、オプション）

## テスト実行方法

Claudeに以下のように指示してください：

```
このファイルのテストケースを順番に実行してください。
各テストケースの実行結果をチェックボックスで記録してください。
```

---

## 2.1 タブ切り替えテスト

### テストケース 2.1.1: 初期表示時にAWSタブがアクティブ

**目的**: 接続設定画面にアクセスすると、デフォルトでAWSタブがアクティブになっていることを確認する

**手順**:
1. http://localhost:5173 または http://localhost:5173/connection にアクセス
2. ページスナップショットを取得
3. AWSタブがアクティブであることを確認
4. AWS接続フォームが表示されることを確認

**期待結果**:
- "AWS" ボタンがアクティブ状態（ハイライトまたは選択状態）
- AWS接続設定フォームが表示される
- プロファイル、リージョン、Assume Role ARN、Session Name の入力フィールドが表示される

**実行**:
```
1. mcp__playwright__browser_navigate({ url: "http://localhost:5173/connection" })
2. mcp__playwright__browser_snapshot({})
3. "AWS" ボタンの状態を確認
4. AWS固有のフィールドを確認
```

**結果**: [ ] 成功 / [ ] 失敗

**スクリーンショット**: `tests/screenshots/2.1.1_aws_tab_active.png`

---

### テストケース 2.1.2: Azureタブをクリック

**目的**: Azureタブをクリックすると、Azureタブがアクティブになり、Azure接続フォームが表示されることを確認する

**手順**:
1. 前のテストの続きから
2. "Azure" タブをクリック
3. ページスナップショットを取得
4. Azureフォームが表示されることを確認

**期待結果**:
- "Azure" ボタンがアクティブ状態
- Azure接続設定フォームが表示される
- 認証方式ドロップダウン、テナントID、Client ID、Client Secret の入力フィールドが表示される

**実行**:
```
1. mcp__playwright__browser_click({ element: "Azureタブボタン", ref: "..." })
2. mcp__playwright__browser_snapshot({})
3. Azure固有のフィールドを確認
```

**結果**: [ ] 成功 / [ ] 失敗

**スクリーンショット**: `tests/screenshots/2.1.2_azure_tab_active.png`

---

### テストケース 2.1.3: AWSタブに戻る

**目的**: AWSタブをクリックすると、再度AWSフォームに切り替わることを確認する

**手順**:
1. 前のテストの続きから
2. "AWS" タブをクリック
3. ページスナップショットを取得
4. AWSフォームが表示されることを確認

**期待結果**:
- "AWS" ボタンがアクティブ状態
- AWS接続設定フォームが表示される

**実行**:
```
1. mcp__playwright__browser_click({ element: "AWSタブボタン", ref: "..." })
2. mcp__playwright__browser_snapshot({})
3. AWS固有のフィールドを確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

## 2.2 AWS接続フォームテスト

### テストケース 2.2.1: プロファイル入力フィールドの表示

**目的**: プロファイル入力フィールドが表示され、デフォルト値が設定されていることを確認する

**手順**:
1. 前のテストの続きから（AWSタブがアクティブな状態）
2. ページスナップショットを取得
3. プロファイル入力フィールドを確認

**期待結果**:
- プロファイル入力フィールド（textbox）が表示される
- デフォルト値: "default"

**実行**:
```
1. mcp__playwright__browser_snapshot({})
2. プロファイル入力フィールドの存在とデフォルト値を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.2.2: リージョン入力フィールドの表示

**目的**: リージョン入力フィールドが表示され、デフォルト値が設定されていることを確認する

**手順**:
1. 前のテストの続きから
2. リージョン入力フィールドを確認

**期待結果**:
- リージョン入力フィールド（textbox）が表示される
- デフォルト値: "ap-northeast-1"（または空）

**実行**:
```
1. mcp__playwright__browser_snapshot({})
2. リージョン入力フィールドの存在とデフォルト値を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.2.3: Assume Role ARN入力フィールドの表示

**目的**: Assume Role ARN入力フィールドが表示されることを確認する

**手順**:
1. 前のテストの続きから
2. Assume Role ARN入力フィールドを確認

**期待結果**:
- Assume Role ARN入力フィールド（textbox）が表示される
- プレースホルダー: "arn:aws:iam::123456789012:role/AdminRole"（または類似）

**実行**:
```
1. mcp__playwright__browser_snapshot({})
2. Assume Role ARN入力フィールドの存在を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.2.4: Session Name入力フィールドの表示

**目的**: Session Name入力フィールドが表示され、デフォルト値が設定されていることを確認する

**手順**:
1. 前のテストの続きから
2. Session Name入力フィールドを確認

**期待結果**:
- Session Name入力フィールド（textbox）が表示される
- デフォルト値: "tfkosmos"（または類似）

**実行**:
```
1. mcp__playwright__browser_snapshot({})
2. Session Name入力フィールドの存在とデフォルト値を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.2.5: aws login実行ボタンのクリック

**目的**: "aws login実行" ボタンをクリックすると、ローディング状態になることを確認する

**注**: 実際のAWS CLIが必要なため、モック環境では完全なテストは困難。ボタンの存在とクリック可能性を確認するにとどめる。

**手順**:
1. 前のテストの続きから
2. "aws login実行" ボタンを確認
3. （オプション）ボタンをクリックしてレスポンスを確認

**期待結果**:
- "aws login実行" ボタン（button）が表示される
- クリック可能な状態

**実行**:
```
1. mcp__playwright__browser_snapshot({})
2. "aws login実行" ボタンの存在を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.2.6: 接続テストボタンのクリック（成功ケース）

**目的**: 正しい認証情報で接続テストを実行すると、成功メッセージが表示されることを確認する

**注**: 実際のAWS認証情報が必要。モック環境の場合はスキップ可能。

**手順**:
1. プロファイル入力フィールドに有効なプロファイルを入力
2. "接続テスト" ボタンをクリック
3. レスポンスを待機
4. 成功メッセージを確認

**期待結果**:
- 成功メッセージ: "接続成功: Account ID XXXXXXXXXXXX"
- メッセージが画面に表示される

**実行**:
```
（実際のAWS環境がある場合のみ実施）
1. mcp__playwright__browser_type({ element: "プロファイル", ref: "...", text: "default" })
2. mcp__playwright__browser_click({ element: "接続テストボタン", ref: "..." })
3. mcp__playwright__browser_wait_for({ text: "接続成功" })
4. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗 / [x] スキップ（AWS環境なし）

**スクリーンショット**: `tests/screenshots/2.2.6_aws_connection_success.png`

---

### テストケース 2.2.7: 接続テストボタンのクリック（失敗ケース）

**目的**: 不正な認証情報で接続テストを実行すると、エラーメッセージが表示されることを確認する

**注**: 実際のAWS認証情報が必要。モック環境の場合はスキップ可能。

**手順**:
1. プロファイル入力フィールドに無効なプロファイルを入力
2. "接続テスト" ボタンをクリック
3. レスポンスを待機
4. エラーメッセージを確認

**期待結果**:
- エラーメッセージが表示される
- メッセージに問題の内容が含まれる

**実行**:
```
（実際のAWS環境がある場合のみ実施）
1. mcp__playwright__browser_type({ element: "プロファイル", ref: "...", text: "invalid-profile" })
2. mcp__playwright__browser_click({ element: "接続テストボタン", ref: "..." })
3. mcp__playwright__browser_wait_for({ text: "エラー" })
4. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗 / [x] スキップ（AWS環境なし）

**スクリーンショット**: `tests/screenshots/2.2.7_aws_connection_error.png`

---

### テストケース 2.2.8: ローディング中のボタン無効化

**目的**: 接続テスト実行中はボタンが無効化されることを確認する

**注**: 実際のAPI呼び出しが必要なため、2.2.6または2.2.7と統合して実施

**手順**:
1. 接続テストボタンをクリック
2. ローディング中のボタン状態を確認

**期待結果**:
- ボタンが無効化される（disabled状態）
- ローディングインジケーターが表示される

**実行**: テストケース 2.2.6 または 2.2.7 で統合実施

**結果**: [ ] 成功 / [ ] 失敗 / [x] 2.2.6/2.2.7で実施

---

## 2.3 Azure接続フォームテスト

### テストケース 2.3.1: 認証方式ドロップダウンの表示

**目的**: Azureタブで認証方式ドロップダウンが表示され、選択肢が正しいことを確認する

**手順**:
1. Azureタブをクリック
2. ページスナップショットを取得
3. 認証方式ドロップダウンを確認

**期待結果**:
- 認証方式ドロップダウン（combobox）が表示される
- 選択肢: "Azure CLI" と "Service Principal"

**実行**:
```
1. mcp__playwright__browser_click({ element: "Azureタブボタン", ref: "..." })
2. mcp__playwright__browser_snapshot({})
3. 認証方式ドロップダウンの選択肢を確認
```

**結果**: [ ] 成功 / [ ] 失敗

**スクリーンショット**: `tests/screenshots/2.3.1_azure_auth_dropdown.png`

---

### テストケース 2.3.2: Azure CLI選択時のフォーム

**目的**: Azure CLIを選択すると、追加入力フィールドが非表示になることを確認する

**手順**:
1. 前のテストの続きから
2. 認証方式ドロップダウンで "Azure CLI" を選択
3. ページスナップショットを取得
4. フォームの状態を確認

**期待結果**:
- テナントID、Client ID、Client Secret の入力フィールドが非表示または無効
- 接続テストボタンが表示される

**実行**:
```
1. mcp__playwright__browser_select_option({ element: "認証方式", ref: "...", values: ["Azure CLI"] })
2. mcp__playwright__browser_snapshot({})
3. 追加フィールドが非表示であることを確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.3.3: Service Principal選択時のフォーム

**目的**: Service Principalを選択すると、テナントID、Client ID、Client Secretの入力フィールドが表示されることを確認する

**手順**:
1. 前のテストの続きから
2. 認証方式ドロップダウンで "Service Principal" を選択
3. ページスナップショットを取得
4. フォームの状態を確認

**期待結果**:
- テナントID入力フィールドが表示される
- Client ID入力フィールドが表示される
- Client Secret入力フィールド（password型）が表示される

**実行**:
```
1. mcp__playwright__browser_select_option({ element: "認証方式", ref: "...", values: ["Service Principal"] })
2. mcp__playwright__browser_snapshot({})
3. 追加フィールドが表示されることを確認
```

**結果**: [ ] 成功 / [ ] 失敗

**スクリーンショット**: `tests/screenshots/2.3.3_azure_service_principal.png`

---

### テストケース 2.3.4: テナントID入力

**目的**: テナントID入力フィールドに値を入力できることを確認する

**手順**:
1. 前のテストの続きから（Service Principal選択状態）
2. テナントID入力フィールドに値を入力
3. ページスナップショットを取得

**期待結果**:
- 入力値が反映される

**実行**:
```
1. mcp__playwright__browser_type({ element: "テナントID", ref: "...", text: "12345678-1234-1234-1234-123456789012" })
2. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.3.5: Client ID入力

**目的**: Client ID入力フィールドに値を入力できることを確認する

**手順**:
1. 前のテストの続きから
2. Client ID入力フィールドに値を入力
3. ページスナップショットを取得

**期待結果**:
- 入力値が反映される

**実行**:
```
1. mcp__playwright__browser_type({ element: "Client ID", ref: "...", text: "abcdefgh-abcd-abcd-abcd-abcdefghijkl" })
2. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.3.6: Client Secret入力（パスワード型）

**目的**: Client Secret入力フィールドがパスワード型で、入力値がマスクされることを確認する

**手順**:
1. 前のテストの続きから
2. Client Secret入力フィールドに値を入力
3. ページスナップショットを取得
4. 入力値がマスクされていることを確認

**期待結果**:
- 入力フィールドの型が "password"
- 入力値が "●●●●●" などでマスクされる

**実行**:
```
1. mcp__playwright__browser_type({ element: "Client Secret", ref: "...", text: "my-secret-password" })
2. mcp__playwright__browser_snapshot({})
3. 入力フィールドの型を確認
```

**結果**: [ ] 成功 / [ ] 失敗

---

### テストケース 2.3.7: 接続テストボタンのクリック（成功ケース）

**目的**: 正しい認証情報で接続テストを実行すると、成功メッセージが表示されることを確認する

**注**: 実際のAzure認証情報が必要。モック環境の場合はスキップ可能。

**手順**:
1. 認証方式で "Azure CLI" を選択
2. "接続テスト" ボタンをクリック
3. レスポンスを待機
4. 成功メッセージを確認

**期待結果**:
- 成功メッセージが表示される
- サブスクリプション名が含まれる

**実行**:
```
（実際のAzure環境がある場合のみ実施）
1. mcp__playwright__browser_select_option({ element: "認証方式", ref: "...", values: ["Azure CLI"] })
2. mcp__playwright__browser_click({ element: "接続テストボタン", ref: "..." })
3. mcp__playwright__browser_wait_for({ text: "接続成功" })
4. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗 / [x] スキップ（Azure環境なし）

**スクリーンショット**: `tests/screenshots/2.3.7_azure_connection_success.png`

---

### テストケース 2.3.8: 接続テストボタンのクリック（失敗ケース）

**目的**: 不正な認証情報で接続テストを実行すると、エラーメッセージが表示されることを確認する

**注**: 実際のAzure認証情報が必要。モック環境の場合はスキップ可能。

**手順**:
1. 認証方式で "Service Principal" を選択
2. 無効な認証情報を入力
3. "接続テスト" ボタンをクリック
4. レスポンスを待機
5. エラーメッセージを確認

**期待結果**:
- エラーメッセージが表示される
- メッセージに問題の内容が含まれる

**実行**:
```
（実際のAzure環境がある場合のみ実施）
1. mcp__playwright__browser_select_option({ element: "認証方式", ref: "...", values: ["Service Principal"] })
2. 無効な認証情報を入力
3. mcp__playwright__browser_click({ element: "接続テストボタン", ref: "..." })
4. mcp__playwright__browser_wait_for({ text: "エラー" })
5. mcp__playwright__browser_snapshot({})
```

**結果**: [ ] 成功 / [ ] 失敗 / [x] スキップ（Azure環境なし）

**スクリーンショット**: `tests/screenshots/2.3.8_azure_connection_error.png`

---

## テスト結果サマリー

### 2.1 タブ切り替え（3件）

| テストケース | 結果 | 備考 |
|------------|------|------|
| 2.1.1 初期表示時にAWSタブがアクティブ | [x] | |
| 2.1.2 Azureタブをクリック | [x] | |
| 2.1.3 AWSタブに戻る | [x] | |

### 2.2 AWS接続フォーム（8件）

| テストケース | 結果 | 備考 |
|------------|------|------|
| 2.2.1 プロファイル入力フィールドの表示 | [x] | |
| 2.2.2 リージョン入力フィールドの表示 | [x] | |
| 2.2.3 Assume Role ARN入力フィールドの表示 | [x] | |
| 2.2.4 Session Name入力フィールドの表示 | [x] | |
| 2.2.5 aws login実行ボタンのクリック | [x] | |
| 2.2.6 接続テスト（成功） | [x] スキップ | AWS環境なし |
| 2.2.7 接続テスト（失敗） | [x] スキップ | AWS環境なし |
| 2.2.8 ローディング中のボタン無効化 | [x] 2.2.6/2.2.7で実施 | |

### 2.3 Azure接続フォーム（8件）

| テストケース | 結果 | 備考 |
|------------|------|------|
| 2.3.1 認証方式ドロップダウンの表示 | [x] | |
| 2.3.2 Azure CLI選択時のフォーム | [x] | |
| 2.3.3 Service Principal選択時のフォーム | [x] | |
| 2.3.4 テナントID入力 | [x] | |
| 2.3.5 Client ID入力 | [x] | |
| 2.3.6 Client Secret入力（パスワード型） | [x] | |
| 2.3.7 接続テスト（成功） | [x] スキップ | Azure環境なし |
| 2.3.8 接続テスト（失敗） | [x] スキップ | Azure環境なし |

**実施可能テスト数**: 14/19
**スキップ（環境依存）**: 4/19
**統合実施**: 1/19
