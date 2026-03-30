/// テストフィクスチャ
///
/// テストで使用する固定データやサンプルデータを提供します。

use crate::domain::iam::{IAMUser, IAMGroup, IAMRole, IAMPolicy};
use serde_json::json;

/// サンプルAWS IAMユーザーを返す
pub fn sample_iam_user(name: &str) -> IAMUser {
    IAMUser {
        id: format!("AIDA{}", name.to_uppercase()),
        name: name.to_string(),
        arn: format!("arn:aws:iam::123456789012:user/{}", name),
        path: "/".to_string(),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        tags: None,
        inline_policies: vec![],
        managed_policy_arns: vec![],
        groups: vec![],
    }
}

/// サンプルAWS IAMグループを返す
pub fn sample_iam_group(name: &str) -> IAMGroup {
    IAMGroup {
        id: format!("AGPA{}", name.to_uppercase()),
        name: name.to_string(),
        arn: format!("arn:aws:iam::123456789012:group/{}", name),
        path: "/".to_string(),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        inline_policies: vec![],
        managed_policy_arns: vec![],
        members: vec![],
    }
}

/// サンプルAWS IAMロールを返す
pub fn sample_iam_role(name: &str) -> IAMRole {
    IAMRole {
        id: format!("AROA{}", name.to_uppercase()),
        name: name.to_string(),
        arn: format!("arn:aws:iam::123456789012:role/{}", name),
        path: "/".to_string(),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        assume_role_policy_document: sample_assume_role_policy_json(),
        inline_policies: vec![],
        managed_policy_arns: vec![],
        max_session_duration: Some(3600),
        description: None,
        tags: None,
    }
}

/// サンプルAWS IAMポリシーを返す
pub fn sample_iam_policy(name: &str) -> IAMPolicy {
    IAMPolicy {
        id: format!("ANPA{}", name.to_uppercase()),
        name: name.to_string(),
        arn: format!("arn:aws:iam::123456789012:policy/{}", name),
        path: "/".to_string(),
        default_version_id: Some("v1".to_string()),
        attachment_count: Some(0),
        permissions_boundary_usage_count: Some(0),
        is_attachable: Some(true),
        description: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        tags: None,
        policy_document: sample_policy_document_json(),
    }
}

/// サンプルAssumeRoleポリシードキュメントJSON
pub fn sample_assume_role_policy_json() -> String {
    json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": {
                "Service": "lambda.amazonaws.com"
            },
            "Action": "sts:AssumeRole"
        }]
    })
    .to_string()
}

/// サンプルポリシードキュメントJSON
pub fn sample_policy_document_json() -> String {
    json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Action": ["s3:GetObject", "s3:PutObject"],
            "Resource": "arn:aws:s3:::my-bucket/*"
        }]
    })
    .to_string()
}

/// サンプルAzure ロール定義を返す（JSON文字列）
pub fn sample_azure_role_definition_json() -> String {
    json!({
        "id": "/subscriptions/12345678-1234-1234-1234-123456789012/providers/Microsoft.Authorization/roleDefinitions/abcd1234-5678-90ab-cdef-1234567890ab",
        "name": "CustomRole",
        "type": "Microsoft.Authorization/roleDefinitions",
        "properties": {
            "roleName": "Custom Role",
            "description": "A custom role for testing",
            "type": "CustomRole",
            "permissions": [{
                "actions": ["Microsoft.Storage/storageAccounts/read"],
                "notActions": [],
                "dataActions": [],
                "notDataActions": []
            }],
            "assignableScopes": ["/subscriptions/12345678-1234-1234-1234-123456789012"]
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_iam_user() {
        let user = sample_iam_user("test-user");
        assert_eq!(user.name, "test-user");
        assert!(user.arn.contains("test-user"));
    }

    #[test]
    fn test_sample_iam_group() {
        let group = sample_iam_group("test-group");
        assert_eq!(group.name, "test-group");
    }

    #[test]
    fn test_sample_iam_role() {
        let role = sample_iam_role("test-role");
        assert_eq!(role.name, "test-role");
        assert!(!role.assume_role_policy_document.is_empty());
    }

    #[test]
    fn test_sample_iam_policy() {
        let policy = sample_iam_policy("test-policy");
        assert_eq!(policy.name, "test-policy");
        assert!(!policy.policy_document.is_empty());
    }

    #[test]
    fn test_sample_policy_document_json() {
        let json = sample_policy_document_json();
        assert!(json.contains("Version"));
        assert!(json.contains("Statement"));
    }
}
