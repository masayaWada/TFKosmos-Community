//! AWS IAMスキャナー
//!
//! AWS IAMリソース（ユーザー、グループ、ロール、ポリシー）をスキャンし、
//! Terraform生成用のデータ構造に変換します。

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::domain::iam_policy::IamPolicyDocument;
use crate::infra::aws::client_factory::AwsClientFactory;
use crate::infra::aws::iam_client_trait::IamClientOps;
use crate::infra::aws::real_iam_client::RealIamClient;
use crate::models::ScanConfig;

/// AWS IAMスキャナー
///
/// IAMクライアントを抽象化することで、テスト時にモックを注入可能にしています。
pub struct AwsIamScanner<C: IamClientOps> {
    config: ScanConfig,
    iam_client: Arc<C>,
}

impl AwsIamScanner<RealIamClient> {
    /// 本番用のスキャナーを作成
    pub async fn new(config: ScanConfig) -> Result<Self> {
        let iam_client = AwsClientFactory::create_iam_client(
            config.profile.clone(),
            config.assume_role_arn.clone(),
            config.assume_role_session_name.clone(),
        )
        .await
        .with_context(|| {
            format!(
                "Failed to create IAM client. Profile: {:?}, Assume Role ARN: {:?}. \
                Please ensure AWS credentials are configured correctly.",
                config.profile, config.assume_role_arn
            )
        })?;

        Ok(Self {
            config,
            iam_client: Arc::new(RealIamClient::new(iam_client)),
        })
    }
}

impl<C: IamClientOps> AwsIamScanner<C> {
    /// テスト用：モッククライアントを使用してスキャナーを作成
    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: C) -> Self {
        Self {
            config,
            iam_client: Arc::new(client),
        }
    }

    /// IAMリソースをスキャン
    pub async fn scan(
        &self,
        progress_callback: Box<dyn Fn(u32, String) + Send + Sync>,
    ) -> Result<Value> {
        let start_time = std::time::Instant::now();
        info!("AWS IAMスキャンを開始");
        progress_callback(0, "AWS IAMスキャンを開始しています...".to_string());

        let mut results = serde_json::Map::new();
        results.insert("provider".to_string(), Value::String("aws".to_string()));

        let scan_targets = &self.config.scan_targets;

        // スキャン対象の数をカウント
        let total_targets = scan_targets.values().filter(|&&v| v).count();
        if total_targets == 0 {
            progress_callback(100, "スキャン対象が選択されていません".to_string());
            return Ok(Value::Object(results));
        }

        let mut completed_targets = 0;

        // Users
        let users = if scan_targets.get("users").copied().unwrap_or(false) {
            debug!("IAM Usersのスキャンを開始");
            progress_callback(
                (completed_targets * 100 / total_targets) as u32,
                "IAM Usersのスキャン中...".to_string(),
            );
            let users = self.scan_users().await?;
            let count = users.len();
            results.insert("users".to_string(), Value::Array(users.clone()));
            completed_targets += 1;
            debug!(count, "IAM Usersのスキャン完了");
            progress_callback(
                (completed_targets * 100 / total_targets) as u32,
                format!("IAM Usersのスキャン完了: {}件", count),
            );
            users
        } else {
            results.insert("users".to_string(), Value::Array(Vec::new()));
            Vec::new()
        };

        // Groups
        let groups = if scan_targets.get("groups").copied().unwrap_or(false) {
            debug!("IAM Groupsのスキャンを開始");
            progress_callback(
                (completed_targets * 100 / total_targets) as u32,
                "IAM Groupsのスキャン中...".to_string(),
            );
            let groups = self.scan_groups().await?;
            let count = groups.len();
            results.insert("groups".to_string(), Value::Array(groups.clone()));
            completed_targets += 1;
            debug!(count, "IAM Groupsのスキャン完了");
            progress_callback(
                (completed_targets * 100 / total_targets) as u32,
                format!("IAM Groupsのスキャン完了: {}件", count),
            );
            groups
        } else {
            results.insert("groups".to_string(), Value::Array(Vec::new()));
            Vec::new()
        };

        // Roles
        let roles = if scan_targets.get("roles").copied().unwrap_or(false) {
            debug!("IAM Rolesのスキャンを開始");
            progress_callback(
                (completed_targets * 100 / total_targets) as u32,
                "IAM Rolesのスキャン中...".to_string(),
            );
            let roles = self.scan_roles().await?;
            let count = roles.len();
            results.insert("roles".to_string(), Value::Array(roles.clone()));
            completed_targets += 1;
            debug!(count, "IAM Rolesのスキャン完了");
            progress_callback(
                (completed_targets * 100 / total_targets) as u32,
                format!("IAM Rolesのスキャン完了: {}件", count),
            );
            roles
        } else {
            results.insert("roles".to_string(), Value::Array(Vec::new()));
            Vec::new()
        };

        // Policies
        if scan_targets.get("policies").copied().unwrap_or(false) {
            debug!("IAM Policiesのスキャンを開始");
            progress_callback(
                (completed_targets * 100 / total_targets) as u32,
                "IAM Policiesのスキャン中...".to_string(),
            );
            let policies = self.scan_policies().await?;
            let count = policies.len();
            results.insert("policies".to_string(), Value::Array(policies));
            completed_targets += 1;
            debug!(count, "IAM Policiesのスキャン完了");
            progress_callback(
                (completed_targets * 100 / total_targets) as u32,
                format!("IAM Policiesのスキャン完了: {}件", count),
            );
        } else {
            results.insert("policies".to_string(), Value::Array(Vec::new()));
        }

        // リソース間の接続情報を取得（既にスキャン済みのデータを再利用）
        let attachments = self
            .scan_attachments_with_data(&users, &groups, &roles)
            .await?;
        results.insert("attachments".to_string(), attachments);

        // クリーンアップ（マネージドポリシーのバージョン等を補完）
        self.scan_cleanup(&mut results).await?;

        let duration = start_time.elapsed();
        info!(
            "AWS IAMスキャン完了 (所要時間: {:.2}秒)",
            duration.as_secs_f64()
        );
        progress_callback(100, "AWS IAMスキャンが完了しました".to_string());

        Ok(Value::Object(results))
    }

    /// 名前プレフィックスフィルタを適用
    pub fn apply_name_prefix_filter(&self, name: &str) -> bool {
        if let Some(prefix) = self.config.filters.get("name_prefix") {
            name.starts_with(prefix)
        } else {
            true
        }
    }

    /// IAMユーザーをスキャン
    pub async fn scan_users(&self) -> Result<Vec<Value>> {
        let users_info = self
            .iam_client
            .list_users_with_options(self.config.include_tags)
            .await?;
        let mut users = Vec::new();

        for user in users_info {
            if !self.apply_name_prefix_filter(&user.user_name) {
                continue;
            }

            let mut user_json = json!({
                "user_name": user.user_name,
                "user_id": user.user_id,
                "arn": user.arn,
                "create_date": user.create_date,
                "path": user.path,
            });

            if !user.tags.is_empty() {
                user_json["tags"] = json!(user.tags);
            }

            users.push(user_json);
        }

        Ok(users)
    }

    /// IAMグループをスキャン
    pub async fn scan_groups(&self) -> Result<Vec<Value>> {
        let groups_info = self.iam_client.list_groups().await?;
        let mut groups = Vec::new();

        for group in groups_info {
            if !self.apply_name_prefix_filter(&group.group_name) {
                continue;
            }

            let group_json = json!({
                "group_name": group.group_name,
                "group_id": group.group_id,
                "arn": group.arn,
                "create_date": group.create_date,
                "path": group.path,
            });

            groups.push(group_json);
        }

        Ok(groups)
    }

    /// IAMロールをスキャン
    pub async fn scan_roles(&self) -> Result<Vec<Value>> {
        let roles_info = self
            .iam_client
            .list_roles_with_options(self.config.include_tags)
            .await?;
        let mut roles = Vec::new();

        for role in roles_info {
            if !self.apply_name_prefix_filter(&role.role_name) {
                continue;
            }

            let assume_role_statements = role
                .assume_role_policy_document
                .as_deref()
                .map(Self::parse_assume_role_policy)
                .unwrap_or_default();

            let mut role_json = json!({
                "role_name": role.role_name,
                "role_id": role.role_id,
                "arn": role.arn,
                "create_date": role.create_date,
                "path": role.path,
                "assume_role_statements": assume_role_statements,
            });

            // 生のassume_role_policy_documentも保存（Terraform生成やパース失敗時のために必要）
            // テンプレートでjsonencode()を使用するため、URLデコードされたJSON文字列として保存する
            if let Some(ref policy_doc) = role.assume_role_policy_document {
                let decoded_doc = match urlencoding::decode(policy_doc) {
                    Ok(s) => s.to_string(),
                    Err(_) => {
                        // URLエンコードされていない場合はそのまま使用
                        policy_doc.clone()
                    }
                };
                role_json["assume_role_policy_document"] = json!(decoded_doc);
            }

            if !role.tags.is_empty() {
                role_json["tags"] = json!(role.tags);
            }

            roles.push(role_json);
        }

        Ok(roles)
    }

    /// IAMポリシーをスキャン
    pub async fn scan_policies(&self) -> Result<Vec<Value>> {
        let policies_info = self.iam_client.list_policies().await?;
        let mut policies = Vec::new();

        for policy in policies_info {
            if !self.apply_name_prefix_filter(&policy.policy_name) {
                continue;
            }

            let policy_json = json!({
                "policy_name": policy.policy_name,
                "policy_id": policy.policy_id,
                "arn": policy.arn,
                "path": policy.path,
                "default_version_id": policy.default_version_id,
                "attachment_count": policy.attachment_count,
                "create_date": policy.create_date,
                "update_date": policy.update_date,
                "description": policy.description,
            });

            policies.push(policy_json);
        }

        Ok(policies)
    }

    /// 既にスキャン済みのデータを使用してリソース間の接続情報をスキャン
    ///
    /// この関数は、scan_users, scan_groups, scan_rolesで取得済みのデータを再利用し、
    /// 重複するAPI呼び出しを削減します。
    async fn scan_attachments_with_data(
        &self,
        users: &[Value],
        groups: &[Value],
        roles: &[Value],
    ) -> Result<Value> {
        let mut attachments = serde_json::Map::new();

        // ユーザー名を抽出
        let user_names: Vec<&str> = users
            .iter()
            .filter_map(|u| u.get("user_name").and_then(|v| v.as_str()))
            .collect();

        // グループ名を抽出
        let group_names: Vec<&str> = groups
            .iter()
            .filter_map(|g| g.get("group_name").and_then(|v| v.as_str()))
            .collect();

        // ロール名を抽出
        let role_names: Vec<&str> = roles
            .iter()
            .filter_map(|r| r.get("role_name").and_then(|v| v.as_str()))
            .collect();

        // UserとPolicyの接続
        let mut user_policies = Vec::new();
        for user_name in &user_names {
            // インラインポリシーを取得
            if let Ok(inline_policies) = self.iam_client.list_user_policies(user_name).await {
                for policy_name in inline_policies {
                    user_policies.push(json!({
                        "user_name": user_name,
                        "policy_name": policy_name,
                        "policy_type": "inline",
                    }));
                }
            }

            // アタッチされたマネージドポリシーを取得
            if let Ok(attached_policies) =
                self.iam_client.list_attached_user_policies(user_name).await
            {
                for policy in attached_policies {
                    user_policies.push(json!({
                        "user_name": user_name,
                        "policy_arn": policy.policy_arn,
                        "policy_type": "managed",
                    }));
                }
            }
        }
        attachments.insert("user_policies".to_string(), Value::Array(user_policies));

        // GroupとPolicyの接続
        let mut group_policies = Vec::new();
        for group_name in &group_names {
            // インラインポリシーを取得
            if let Ok(inline_policies) = self.iam_client.list_group_policies(group_name).await {
                for policy_name in inline_policies {
                    group_policies.push(json!({
                        "group_name": group_name,
                        "policy_name": policy_name,
                        "policy_type": "inline",
                    }));
                }
            }

            // アタッチされたマネージドポリシーを取得
            if let Ok(attached_policies) = self
                .iam_client
                .list_attached_group_policies(group_name)
                .await
            {
                for policy in attached_policies {
                    group_policies.push(json!({
                        "group_name": group_name,
                        "policy_arn": policy.policy_arn,
                        "policy_type": "managed",
                    }));
                }
            }
        }
        attachments.insert("group_policies".to_string(), Value::Array(group_policies));

        // RoleとPolicyの接続
        let mut role_policies = Vec::new();
        for role_name in &role_names {
            // インラインポリシーを取得
            if let Ok(inline_policies) = self.iam_client.list_role_policies(role_name).await {
                for policy_name in inline_policies {
                    role_policies.push(json!({
                        "role_name": role_name,
                        "policy_name": policy_name,
                        "policy_type": "inline",
                    }));
                }
            }

            // アタッチされたマネージドポリシーを取得
            if let Ok(attached_policies) =
                self.iam_client.list_attached_role_policies(role_name).await
            {
                for policy in attached_policies {
                    role_policies.push(json!({
                        "role_name": role_name,
                        "policy_arn": policy.policy_arn,
                        "policy_type": "managed",
                    }));
                }
            }
        }
        attachments.insert("role_policies".to_string(), Value::Array(role_policies));

        // UserとGroupの接続
        let mut user_groups = Vec::new();
        for user_name in &user_names {
            if let Ok(groups) = self.iam_client.list_groups_for_user(user_name).await {
                for group_name in groups {
                    user_groups.push(json!({
                        "user_name": user_name,
                        "group_name": group_name,
                    }));
                }
            }
        }
        attachments.insert("user_groups".to_string(), Value::Array(user_groups));

        Ok(Value::Object(attachments))
    }

    /// クリーンアップ処理（ポリシードキュメントを補完）
    async fn scan_cleanup(&self, results: &mut serde_json::Map<String, Value>) -> Result<()> {
        // Policiesにポリシードキュメントを追加
        if let Some(Value::Array(policies)) = results.get_mut("policies") {
            for policy in policies.iter_mut() {
                if let Some(policy_arn) = policy.get("arn").and_then(|v| v.as_str()) {
                    if let Some(default_version_id) = policy
                        .get("default_version_id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        if let Ok(Some(policy_doc)) = self
                            .iam_client
                            .get_policy_version(policy_arn, default_version_id)
                            .await
                        {
                            // URLデコードしてJSONパース
                            if let Ok(decoded) = urlencoding::decode(&policy_doc.document) {
                                if let Ok(parsed_doc) =
                                    serde_json::from_str::<IamPolicyDocument>(&decoded)
                                {
                                    policy
                                        .as_object_mut()
                                        .unwrap()
                                        .insert("policy_document".to_string(), json!(parsed_doc));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// AssumeRoleポリシーをパース
    pub fn parse_assume_role_policy(policy_doc: &str) -> Vec<Value> {
        if policy_doc.is_empty() {
            return Vec::new();
        }

        // URLデコード
        let decoded = match urlencoding::decode(policy_doc) {
            Ok(s) => s.to_string(),
            Err(_) => {
                // URLエンコードされていない場合はそのまま使用
                policy_doc.to_string()
            }
        };

        // JSONパース
        let policy_value: Value = match serde_json::from_str(&decoded) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to parse assume_role_policy_document: {}", e);
                return Vec::new();
            }
        };

        // Statementを抽出
        let statements = match policy_value.get("Statement") {
            Some(Value::Array(arr)) => arr,
            _ => {
                warn!("No Statement array found in assume_role_policy_document");
                return Vec::new();
            }
        };

        // 各Statementを変換
        statements
            .iter()
            .filter_map(|stmt| {
                let effect = stmt.get("Effect")?.as_str()?.to_string();

                // Principalの処理
                let (principal_type, principal_identifiers) = match stmt.get("Principal") {
                    Some(Value::Object(principal_obj)) => {
                        // Principalが{"Service": "..."}の形式
                        if let Some(service) = principal_obj.get("Service") {
                            let identifiers = match service {
                                Value::String(s) => vec![s.clone()],
                                Value::Array(arr) => arr
                                    .iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect(),
                                _ => vec![],
                            };
                            ("Service".to_string(), identifiers)
                        } else if let Some(aws) = principal_obj.get("AWS") {
                            // Principalが{"AWS": "..."}の形式
                            let identifiers = match aws {
                                Value::String(s) => vec![s.clone()],
                                Value::Array(arr) => arr
                                    .iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect(),
                                _ => vec![],
                            };
                            ("AWS".to_string(), identifiers)
                        } else if let Some(federated) = principal_obj.get("Federated") {
                            // Principalが{"Federated": "..."}の形式
                            let identifiers = match federated {
                                Value::String(s) => vec![s.clone()],
                                Value::Array(arr) => arr
                                    .iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect(),
                                _ => vec![],
                            };
                            ("Federated".to_string(), identifiers)
                        } else {
                            ("Unknown".to_string(), vec![])
                        }
                    }
                    Some(Value::String(s)) if s == "*" => {
                        // Principalが"*"の形式（{"AWS": "*"}と同じ意味）
                        ("AWS".to_string(), vec!["*".to_string()])
                    }
                    _ => ("Unknown".to_string(), vec![]),
                };

                // Actionの処理
                let actions = match stmt.get("Action") {
                    Some(Value::String(s)) => vec![s.clone()],
                    Some(Value::Array(arr)) => arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                    _ => vec![],
                };

                // Conditionの処理
                let conditions = if let Some(Value::Object(cond_obj)) = stmt.get("Condition") {
                    let mut conds = Vec::new();
                    for (operator, value) in cond_obj {
                        if let Value::Object(inner_obj) = value {
                            for (key, val) in inner_obj {
                                conds.push(json!({
                                    "operator": operator,
                                    "key": key,
                                    "value": val,
                                }));
                            }
                        }
                    }
                    conds
                } else {
                    vec![]
                };

                Some(json!({
                    "effect": effect,
                    "principal_type": principal_type,
                    "principal_identifiers": principal_identifiers,
                    "actions": actions,
                    "conditions": conditions,
                }))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::iam_client_trait::mock::MockIamClient;
    use crate::infra::aws::iam_client_trait::{
        IamGroupInfo, IamPolicyInfo, IamRoleInfo, IamUserInfo,
    };
    use std::collections::HashMap;

    fn create_test_config(
        filters: HashMap<String, String>,
        scan_targets: HashMap<String, bool>,
    ) -> ScanConfig {
        ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets,
            filters,
            include_tags: true,
        }
    }

    // ========================================
    // parse_assume_role_policy のテスト
    // ========================================

    #[test]
    fn test_parse_assume_role_policy_empty() {
        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_assume_role_policy_service_principal() {
        let policy = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {
                        "Service": "lambda.amazonaws.com"
                    },
                    "Action": "sts:AssumeRole"
                }
            ]
        }"#;

        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(policy);
        assert_eq!(result.len(), 1);

        let stmt = &result[0];
        assert_eq!(stmt["effect"], "Allow");
        assert_eq!(stmt["principal_type"], "Service");
        assert_eq!(
            stmt["principal_identifiers"],
            json!(["lambda.amazonaws.com"])
        );
        assert_eq!(stmt["actions"], json!(["sts:AssumeRole"]));
    }

    #[test]
    fn test_parse_assume_role_policy_aws_principal() {
        let policy = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {
                        "AWS": "arn:aws:iam::123456789012:root"
                    },
                    "Action": "sts:AssumeRole"
                }
            ]
        }"#;

        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(policy);
        assert_eq!(result.len(), 1);

        let stmt = &result[0];
        assert_eq!(stmt["effect"], "Allow");
        assert_eq!(stmt["principal_type"], "AWS");
        assert_eq!(
            stmt["principal_identifiers"],
            json!(["arn:aws:iam::123456789012:root"])
        );
    }

    #[test]
    fn test_parse_assume_role_policy_with_condition() {
        let policy = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {
                        "Service": "ec2.amazonaws.com"
                    },
                    "Action": "sts:AssumeRole",
                    "Condition": {
                        "StringEquals": {
                            "sts:ExternalId": "unique-external-id"
                        }
                    }
                }
            ]
        }"#;

        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(policy);
        assert_eq!(result.len(), 1);

        let stmt = &result[0];
        assert_eq!(stmt["effect"], "Allow");
        assert_eq!(stmt["conditions"].as_array().unwrap().len(), 1);

        let condition = &stmt["conditions"][0];
        assert_eq!(condition["operator"], "StringEquals");
        assert_eq!(condition["key"], "sts:ExternalId");
        assert_eq!(condition["value"], "unique-external-id");
    }

    #[test]
    fn test_parse_assume_role_policy_url_encoded() {
        let encoded_policy = "%7B%22Version%22%3A%222012-10-17%22%2C%22Statement%22%3A%5B%7B%22Effect%22%3A%22Allow%22%2C%22Principal%22%3A%7B%22Service%22%3A%22lambda.amazonaws.com%22%7D%2C%22Action%22%3A%22sts%3AAssumeRole%22%7D%5D%7D";

        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(encoded_policy);
        assert_eq!(result.len(), 1);

        let stmt = &result[0];
        assert_eq!(stmt["effect"], "Allow");
        assert_eq!(stmt["principal_type"], "Service");
        assert_eq!(
            stmt["principal_identifiers"],
            json!(["lambda.amazonaws.com"])
        );
    }

    #[test]
    fn test_parse_assume_role_policy_multiple_principals() {
        let policy = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {
                        "Service": ["ec2.amazonaws.com", "lambda.amazonaws.com"]
                    },
                    "Action": "sts:AssumeRole"
                }
            ]
        }"#;

        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(policy);
        assert_eq!(result.len(), 1);

        let stmt = &result[0];
        assert_eq!(
            stmt["principal_identifiers"],
            json!(["ec2.amazonaws.com", "lambda.amazonaws.com"])
        );
    }

    #[test]
    fn test_parse_assume_role_policy_invalid_json() {
        let invalid_json = "not a valid json";
        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(invalid_json);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_assume_role_policy_asterisk_principal() {
        let policy = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": "*",
                    "Action": "sts:AssumeRole",
                    "Condition": {
                        "StringEquals": {
                            "sts:ExternalId": "unique-external-id"
                        }
                    }
                }
            ]
        }"#;

        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(policy);
        assert_eq!(result.len(), 1);

        let stmt = &result[0];
        assert_eq!(stmt["effect"], "Allow");
        assert_eq!(stmt["principal_type"], "AWS");
        assert_eq!(stmt["principal_identifiers"], json!(["*"]));
        assert_eq!(stmt["actions"], json!(["sts:AssumeRole"]));
        assert_eq!(stmt["conditions"].as_array().unwrap().len(), 1);
    }

    // ========================================
    // apply_name_prefix_filter のテスト
    // ========================================

    #[test]
    fn test_apply_name_prefix_filter_with_prefix() {
        let mut filters = HashMap::new();
        filters.insert("name_prefix".to_string(), "test-".to_string());

        let mut mock_client = MockIamClient::new();
        mock_client.expect_list_users().returning(|| Ok(vec![]));

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(filters, HashMap::new()),
            mock_client,
        );

        // プレフィックスにマッチする場合
        assert!(scanner.apply_name_prefix_filter("test-user"));
        assert!(scanner.apply_name_prefix_filter("test-role-123"));

        // プレフィックスにマッチしない場合
        assert!(!scanner.apply_name_prefix_filter("prod-user"));
        assert!(!scanner.apply_name_prefix_filter("user"));
    }

    #[test]
    fn test_apply_name_prefix_filter_without_prefix() {
        let mock_client = MockIamClient::new();
        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), HashMap::new()),
            mock_client,
        );

        // フィルタがない場合は全て通す
        assert!(scanner.apply_name_prefix_filter("any-name"));
        assert!(scanner.apply_name_prefix_filter("test-user"));
        assert!(scanner.apply_name_prefix_filter("prod-role"));
    }

    // ========================================
    // scan_users のモックテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_users_returns_filtered_users() {
        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_users_with_options().returning(|_| {
            Ok(vec![
                IamUserInfo {
                    user_name: "test-user-1".to_string(),
                    user_id: "AIDA1234567890".to_string(),
                    arn: "arn:aws:iam::123456789012:user/test-user-1".to_string(),
                    create_date: 1609459200,
                    path: "/".to_string(),
                    tags: HashMap::new(),
                },
                IamUserInfo {
                    user_name: "prod-user".to_string(),
                    user_id: "AIDA0987654321".to_string(),
                    arn: "arn:aws:iam::123456789012:user/prod-user".to_string(),
                    create_date: 1609459200,
                    path: "/".to_string(),
                    tags: HashMap::new(),
                },
                IamUserInfo {
                    user_name: "test-user-2".to_string(),
                    user_id: "AIDA1111111111".to_string(),
                    arn: "arn:aws:iam::123456789012:user/test-user-2".to_string(),
                    create_date: 1609459200,
                    path: "/developers/".to_string(),
                    tags: {
                        let mut tags = HashMap::new();
                        tags.insert("Environment".to_string(), "test".to_string());
                        tags
                    },
                },
            ])
        });

        let mut filters = HashMap::new();
        filters.insert("name_prefix".to_string(), "test-".to_string());

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(filters, HashMap::new()),
            mock_client,
        );

        let users = scanner.scan_users().await.unwrap();

        // test-で始まるユーザーのみが返される
        assert_eq!(users.len(), 2);
        assert_eq!(users[0]["user_name"], "test-user-1");
        assert_eq!(users[1]["user_name"], "test-user-2");
        assert_eq!(users[1]["path"], "/developers/");
        assert!(users[1]["tags"].is_object());
    }

    // ========================================
    // scan_groups のモックテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_groups_returns_all_groups() {
        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_groups().returning(|| {
            Ok(vec![
                IamGroupInfo {
                    group_name: "developers".to_string(),
                    group_id: "AGPA1234567890".to_string(),
                    arn: "arn:aws:iam::123456789012:group/developers".to_string(),
                    create_date: 1609459200,
                    path: "/".to_string(),
                },
                IamGroupInfo {
                    group_name: "admins".to_string(),
                    group_id: "AGPA0987654321".to_string(),
                    arn: "arn:aws:iam::123456789012:group/admins".to_string(),
                    create_date: 1609459200,
                    path: "/admin/".to_string(),
                },
            ])
        });

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), HashMap::new()),
            mock_client,
        );

        let groups = scanner.scan_groups().await.unwrap();

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0]["group_name"], "developers");
        assert_eq!(groups[1]["group_name"], "admins");
    }

    // ========================================
    // scan_roles のモックテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_roles_with_assume_role_policy() {
        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_roles_with_options().returning(|_| {
            Ok(vec![
                IamRoleInfo {
                    role_name: "lambda-execution-role".to_string(),
                    role_id: "AROA1234567890".to_string(),
                    arn: "arn:aws:iam::123456789012:role/lambda-execution-role".to_string(),
                    create_date: 1609459200,
                    path: "/service-role/".to_string(),
                    assume_role_policy_document: Some(r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"lambda.amazonaws.com"},"Action":"sts:AssumeRole"}]}"#.to_string()),
                    tags: HashMap::new(),
                },
            ])
        });

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), HashMap::new()),
            mock_client,
        );

        let roles = scanner.scan_roles().await.unwrap();

        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0]["role_name"], "lambda-execution-role");

        // assume_role_policy_documentフィールドが含まれていることを確認
        assert!(roles[0]["assume_role_policy_document"].is_string());
        assert!(roles[0]["assume_role_policy_document"]
            .as_str()
            .unwrap()
            .contains("lambda.amazonaws.com"));

        let assume_role_statements = roles[0]["assume_role_statements"].as_array().unwrap();
        assert_eq!(assume_role_statements.len(), 1);
        assert_eq!(assume_role_statements[0]["principal_type"], "Service");
    }

    #[tokio::test]
    async fn test_scan_roles_with_url_encoded_policy_document() {
        let mut mock_client = MockIamClient::new();

        // URLエンコードされたポリシードキュメントをテスト
        let encoded_policy = "%7B%22Version%22%3A%222012-10-17%22%2C%22Statement%22%3A%5B%7B%22Effect%22%3A%22Allow%22%2C%22Principal%22%3A%7B%22Service%22%3A%22lambda.amazonaws.com%22%7D%2C%22Action%22%3A%22sts%3AAssumeRole%22%7D%5D%7D";
        let decoded_policy = r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"lambda.amazonaws.com"},"Action":"sts:AssumeRole"}]}"#;

        mock_client
            .expect_list_roles_with_options()
            .returning(move |_| {
                Ok(vec![IamRoleInfo {
                    role_name: "lambda-execution-role".to_string(),
                    role_id: "AROA1234567890".to_string(),
                    arn: "arn:aws:iam::123456789012:role/lambda-execution-role".to_string(),
                    create_date: 1609459200,
                    path: "/service-role/".to_string(),
                    assume_role_policy_document: Some(encoded_policy.to_string()),
                    tags: HashMap::new(),
                }])
            });

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), HashMap::new()),
            mock_client,
        );

        let roles = scanner.scan_roles().await.unwrap();

        assert_eq!(roles.len(), 1);

        // URLデコードされたポリシードキュメントが保存されていることを確認
        let stored_policy = roles[0]["assume_role_policy_document"].as_str().unwrap();
        assert_eq!(stored_policy, decoded_policy);

        // URLエンコードされた文字列が含まれていないことを確認
        assert!(!stored_policy.contains("%7B"));
        assert!(!stored_policy.contains("%22"));

        // デコードされたJSONが有効であることを確認
        assert!(serde_json::from_str::<serde_json::Value>(stored_policy).is_ok());
    }

    // ========================================
    // scan_policies のモックテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_policies_returns_local_policies() {
        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_policies().returning(|| {
            Ok(vec![IamPolicyInfo {
                policy_name: "CustomS3Policy".to_string(),
                policy_id: "ANPA1234567890".to_string(),
                arn: "arn:aws:iam::123456789012:policy/CustomS3Policy".to_string(),
                path: "/".to_string(),
                default_version_id: "v1".to_string(),
                attachment_count: 2,
                create_date: 1609459200,
                update_date: 1609459200,
                description: "Custom S3 access policy".to_string(),
            }])
        });

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), HashMap::new()),
            mock_client,
        );

        let policies = scanner.scan_policies().await.unwrap();

        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0]["policy_name"], "CustomS3Policy");
        assert_eq!(policies[0]["attachment_count"], 2);
    }

    // ========================================
    // 進捗コールバックのテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_progress_callback() {
        let mut mock_client = MockIamClient::new();

        // 全ての必要なメソッドにモック設定
        mock_client.expect_list_users_with_options().returning(|_| {
            Ok(vec![IamUserInfo {
                user_name: "user1".to_string(),
                user_id: "id1".to_string(),
                arn: "arn1".to_string(),
                create_date: 0,
                path: "/".to_string(),
                tags: HashMap::new(),
            }])
        });
        mock_client.expect_list_groups().returning(|| Ok(vec![]));
        mock_client
            .expect_list_roles_with_options()
            .returning(|_| Ok(vec![]));
        mock_client.expect_list_policies().returning(|| Ok(vec![]));
        mock_client
            .expect_list_user_policies()
            .returning(|_| Ok(vec![]));
        mock_client
            .expect_list_attached_user_policies()
            .returning(|_| Ok(vec![]));
        mock_client
            .expect_list_groups_for_user()
            .returning(|_| Ok(vec![]));

        let mut scan_targets = HashMap::new();
        scan_targets.insert("users".to_string(), true);

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), scan_targets),
            mock_client,
        );

        let progress_values = Arc::new(std::sync::Mutex::new(Vec::new()));
        let progress_clone = progress_values.clone();

        let callback: Box<dyn Fn(u32, String) + Send + Sync> =
            Box::new(move |progress, message| {
                progress_clone.lock().unwrap().push((progress, message));
            });

        let result = scanner.scan(callback).await.unwrap();

        // 結果の検証
        assert_eq!(result["provider"], "aws");
        assert_eq!(result["users"].as_array().unwrap().len(), 1);

        // 進捗コールバックの検証
        let values = progress_values.lock().unwrap();
        assert!(values.len() >= 2); // 開始と完了のコールバック
        assert_eq!(values.last().unwrap().0, 100); // 最後は100%
    }

    // ========================================
    // エラーハンドリングのテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_users_error_handling() {
        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_users_with_options().returning(|_| {
            Err(anyhow::anyhow!(
                "Authentication failed: invalid credentials"
            ))
        });

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), HashMap::new()),
            mock_client,
        );

        let result = scanner.scan_users().await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Authentication failed"));
    }

    #[tokio::test]
    async fn test_scan_with_permission_denied() {
        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_users_with_options().returning(|_| {
            Err(anyhow::anyhow!(
                "Access Denied: iam:ListUsers permission required"
            ))
        });

        let mut scan_targets = HashMap::new();
        scan_targets.insert("users".to_string(), true);

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), scan_targets),
            mock_client,
        );

        let callback: Box<dyn Fn(u32, String) + Send + Sync> = Box::new(|_, _| {});
        let result = scanner.scan(callback).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Access Denied"));
    }

    // ========================================
    // scan_user_policy_attachments のテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_user_policy_attachments() {
        use crate::infra::aws::iam_client_trait::PolicyAttachment;

        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_users_with_options().returning(|_| {
            Ok(vec![IamUserInfo {
                user_name: "alice".to_string(),
                user_id: "AIDA1".to_string(),
                arn: "arn:aws:iam::123:user/alice".to_string(),
                create_date: 0,
                path: "/".to_string(),
                tags: HashMap::new(),
            }])
        });
        mock_client.expect_list_groups().returning(|| Ok(vec![]));
        mock_client
            .expect_list_roles_with_options()
            .returning(|_| Ok(vec![]));
        mock_client.expect_list_policies().returning(|| Ok(vec![]));
        mock_client
            .expect_list_user_policies()
            .withf(|name| name == "alice")
            .returning(|_| Ok(vec!["InlinePolicy1".to_string()]));
        mock_client
            .expect_list_attached_user_policies()
            .withf(|name| name == "alice")
            .returning(|_| {
                Ok(vec![PolicyAttachment {
                    policy_arn: "arn:aws:iam::aws:policy/ReadOnlyAccess".to_string(),
                    policy_name: Some("ReadOnlyAccess".to_string()),
                }])
            });
        mock_client
            .expect_list_groups_for_user()
            .withf(|name| name == "alice")
            .returning(|_| Ok(vec!["developers".to_string()]));

        let mut scan_targets = HashMap::new();
        scan_targets.insert("users".to_string(), true);

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), scan_targets),
            mock_client,
        );

        let callback: Box<dyn Fn(u32, String) + Send + Sync> = Box::new(|_, _| {});
        let result = scanner.scan(callback).await.unwrap();

        let attachments = result["attachments"].as_object().unwrap();
        let user_policies = attachments["user_policies"].as_array().unwrap();
        // 1 inline + 1 managed
        assert_eq!(user_policies.len(), 2);
        let inline = user_policies
            .iter()
            .find(|p| p["policy_type"] == "inline")
            .unwrap();
        assert_eq!(inline["user_name"], "alice");
        assert_eq!(inline["policy_name"], "InlinePolicy1");

        let managed = user_policies
            .iter()
            .find(|p| p["policy_type"] == "managed")
            .unwrap();
        assert_eq!(managed["user_name"], "alice");
        assert_eq!(
            managed["policy_arn"],
            "arn:aws:iam::aws:policy/ReadOnlyAccess"
        );

        let user_groups = attachments["user_groups"].as_array().unwrap();
        assert_eq!(user_groups.len(), 1);
        assert_eq!(user_groups[0]["group_name"], "developers");
    }

    // ========================================
    // assume_role_policy_document: None のロールテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_role_without_assume_role_policy() {
        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_roles_with_options().returning(|_| {
            Ok(vec![IamRoleInfo {
                role_name: "service-role".to_string(),
                role_id: "AROA9999".to_string(),
                arn: "arn:aws:iam::123:role/service-role".to_string(),
                create_date: 0,
                path: "/".to_string(),
                assume_role_policy_document: None,
                tags: HashMap::new(),
            }])
        });

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), HashMap::new()),
            mock_client,
        );

        let roles = scanner.scan_roles().await.unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0]["role_name"], "service-role");
        // assume_role_policy_document が None の場合、フィールドなし
        assert!(roles[0].get("assume_role_policy_document").is_none());
        // assume_role_statements は空配列
        let stmts = roles[0]["assume_role_statements"].as_array().unwrap();
        assert!(stmts.is_empty());
    }

    // ========================================
    // 複数ターゲット同時スキャンのテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_multiple_targets_simultaneously() {
        use crate::infra::aws::iam_client_trait::IamGroupInfo;

        let mut mock_client = MockIamClient::new();

        mock_client.expect_list_users_with_options().returning(|_| {
            Ok(vec![IamUserInfo {
                user_name: "user1".to_string(),
                user_id: "id1".to_string(),
                arn: "arn:aws:iam::123:user/user1".to_string(),
                create_date: 0,
                path: "/".to_string(),
                tags: HashMap::new(),
            }])
        });
        mock_client.expect_list_groups().returning(|| {
            Ok(vec![IamGroupInfo {
                group_name: "group1".to_string(),
                group_id: "gid1".to_string(),
                arn: "arn:aws:iam::123:group/group1".to_string(),
                create_date: 0,
                path: "/".to_string(),
            }])
        });
        mock_client.expect_list_roles_with_options().returning(|_| {
            Ok(vec![IamRoleInfo {
                role_name: "role1".to_string(),
                role_id: "rid1".to_string(),
                arn: "arn:aws:iam::123:role/role1".to_string(),
                create_date: 0,
                path: "/".to_string(),
                assume_role_policy_document: None,
                tags: HashMap::new(),
            }])
        });
        mock_client.expect_list_policies().returning(|| {
            Ok(vec![IamPolicyInfo {
                policy_name: "pol1".to_string(),
                policy_id: "pid1".to_string(),
                arn: "arn:aws:iam::123:policy/pol1".to_string(),
                path: "/".to_string(),
                default_version_id: "v1".to_string(),
                attachment_count: 1,
                create_date: 0,
                update_date: 0,
                description: "".to_string(),
            }])
        });
        // attachment lookups for user1
        mock_client
            .expect_list_user_policies()
            .returning(|_| Ok(vec![]));
        mock_client
            .expect_list_attached_user_policies()
            .returning(|_| Ok(vec![]));
        mock_client
            .expect_list_groups_for_user()
            .returning(|_| Ok(vec![]));
        // attachment lookups for group1
        mock_client
            .expect_list_group_policies()
            .returning(|_| Ok(vec![]));
        mock_client
            .expect_list_attached_group_policies()
            .returning(|_| Ok(vec![]));
        // attachment lookups for role1
        mock_client
            .expect_list_role_policies()
            .returning(|_| Ok(vec![]));
        mock_client
            .expect_list_attached_role_policies()
            .returning(|_| Ok(vec![]));
        // policy version for pol1 (cleanup)
        mock_client
            .expect_get_policy_version()
            .returning(|_, _| Ok(None));

        let mut scan_targets = HashMap::new();
        scan_targets.insert("users".to_string(), true);
        scan_targets.insert("groups".to_string(), true);
        scan_targets.insert("roles".to_string(), true);
        scan_targets.insert("policies".to_string(), true);

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), scan_targets),
            mock_client,
        );

        let callback: Box<dyn Fn(u32, String) + Send + Sync> = Box::new(|_, _| {});
        let result = scanner.scan(callback).await.unwrap();

        assert_eq!(result["users"].as_array().unwrap().len(), 1);
        assert_eq!(result["groups"].as_array().unwrap().len(), 1);
        assert_eq!(result["roles"].as_array().unwrap().len(), 1);
        assert_eq!(result["policies"].as_array().unwrap().len(), 1);
    }

    // ========================================
    // scan_targets が空の場合のテスト
    // ========================================

    #[tokio::test]
    async fn test_scan_with_no_targets() {
        let mock_client = MockIamClient::new();

        let scanner = AwsIamScanner::new_with_client(
            create_test_config(HashMap::new(), HashMap::new()),
            mock_client,
        );

        let callback: Box<dyn Fn(u32, String) + Send + Sync> = Box::new(|_, _| {});
        let result = scanner.scan(callback).await.unwrap();

        assert_eq!(result["provider"], "aws");
    }

    // ========================================
    // Federated Principal のテスト
    // ========================================

    #[test]
    fn test_parse_assume_role_policy_federated_principal() {
        let policy = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {
                        "Federated": "cognito-identity.amazonaws.com"
                    },
                    "Action": "sts:AssumeRoleWithWebIdentity"
                }
            ]
        }"#;

        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(policy);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["principal_type"], "Federated");
        assert_eq!(
            result[0]["principal_identifiers"],
            json!(["cognito-identity.amazonaws.com"])
        );
    }

    // ========================================
    // Policy document が無い Statement テスト
    // ========================================

    #[test]
    fn test_parse_assume_role_policy_no_statement() {
        let policy = r#"{"Version": "2012-10-17"}"#;
        let result = AwsIamScanner::<MockIamClient>::parse_assume_role_policy(policy);
        assert!(result.is_empty());
    }
}
