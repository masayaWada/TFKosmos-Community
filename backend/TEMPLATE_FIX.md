# Terraformテンプレート修正内容

## 修正日時
2026-01-04

## 問題点

### 1. テンプレート補間エラー
- **エラー内容**: `Extra characters after interpolation expression`
- **原因**: IAMポリシーのリソースARNに`${aws:username}`などの変数が含まれている場合、Terraformが`${}`を補間構文として誤解釈
- **影響範囲**: `iam_policy.tf.j2`, `iam_role.tf.j2`

### 2. 不要な空行
- **問題**: 生成されたTerraformコード内に不要な空行が多数存在
- **原因**: Jinja2テンプレート内の条件分岐後に空行が挿入されていた
- **影響**: コードの可読性低下、`terraform fmt`では空行は削除されない

## 修正内容

### 1. `${}`のエスケープ処理追加

**修正ファイル**:
- `backend/templates_default/terraform/aws/iam_policy.tf.j2`
- `backend/templates_default/terraform/aws/iam_role.tf.j2`

**修正箇所**:
```jinja2
# 修正前
"{{ action }}",
"{{ resource }}",

# 修正後
"{{ action | replace('${', '$${') }}",
"{{ resource | replace('${', '$${') }}",
```

**対象フィールド**:
- IAMポリシー: `actions`, `resources`, `not_actions`, `not_resources`
- IAMロール: `identifiers`, `actions`, `condition.variable`, `condition.values`

### 2. 不要な空行の削除

**修正内容**:
- Jinja2テンプレート内の条件分岐(`{% if %}`, `{% endif %}`)の後の空行を削除
- ブロック間の空行を最小化

**例**:
```jinja2
# 修正前
{% if statement.actions %}
    actions = [
{% for action in statement.actions %}
      "{{ action }}",
{% endfor %}
    ]
{% endif %}

{% if statement.resources %}  # 空行が多い
    resources = [

# 修正後
{% if statement.actions %}
    actions = [
{% for action in statement.actions %}
      "{{ action }}",
{% endfor %}
    ]
{% endif %}
{% if statement.resources %}  # 空行を削除
    resources = [
```

## 効果

1. **補間エラーの解消**
   - `${aws:username}`などの変数を含むARNが正しく`$${aws:username}`にエスケープされる
   - `terraform validate`が成功する

2. **コード品質の向上**
   - 生成されるTerraformコードの不要な空行が削除される
   - より読みやすく、整形されたコードが生成される

## テスト結果

- ✅ コンパイルチェック成功
- ✅ 既存テスト通過
- ✅ テンプレート構文エラーなし

## 今後の課題

- [ ] 他のクラウドプロバイダー（Azure）のテンプレートも同様の修正を検討
- [ ] インラインポリシーを持つリソースの対応
- [ ] カスタムテンプレート（`templates_user/`）のガイドライン更新

## 関連ドキュメント

- [Terraform Interpolation Syntax](https://www.terraform.io/language/expressions/strings#interpolation)
- [Jinja2 Template Designer Documentation](https://jinja.palletsprojects.com/en/3.1.x/templates/)
