#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// AWS IAMユーザー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamUser {
    pub user_name: String,
    pub user_id: String,
    pub arn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// AWS IAMグループ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamGroup {
    pub group_name: String,
    pub group_id: String,
    pub arn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<String>>,
}

/// AWS IAMロール
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamRole {
    pub role_name: String,
    pub role_id: String,
    pub arn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_session_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assume_role_policy: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// AWS IAMポリシー（管理ポリシー）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamManagedPolicy {
    pub policy_name: String,
    pub policy_id: String,
    pub arn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_document: Option<serde_json::Value>,
}

/// ポリシーアタッチメント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAttachment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_arn: Option<String>,
    pub policy_type: String, // "inline" or "managed"
}

/// ユーザーとグループの関連
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGroupMembership {
    pub user_name: String,
    pub group_name: String,
}

/// AWSリソースタグ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value")]
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iam_user_serde_roundtrip() {
        let user = IamUser {
            user_name: "test-user".to_string(),
            user_id: "AIDAEXAMPLE".to_string(),
            arn: "arn:aws:iam::123456789012:user/test-user".to_string(),
            create_date: Some("2024-01-01T00:00:00Z".to_string()),
            path: Some("/".to_string()),
            tags: Some(vec![Tag {
                key: "Environment".to_string(),
                value: "Production".to_string(),
            }]),
        };

        let json = serde_json::to_string(&user).unwrap();
        let deserialized: IamUser = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_name, "test-user");
        assert_eq!(deserialized.user_id, "AIDAEXAMPLE");
    }

    #[test]
    fn test_iam_group_serde_roundtrip() {
        let group = IamGroup {
            group_name: "admin-group".to_string(),
            group_id: "AGPAEXAMPLE".to_string(),
            arn: "arn:aws:iam::123456789012:group/admin-group".to_string(),
            create_date: None,
            path: Some("/".to_string()),
            members: Some(vec!["user1".to_string(), "user2".to_string()]),
        };

        let json = serde_json::to_string(&group).unwrap();
        let deserialized: IamGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.group_name, "admin-group");
        assert_eq!(deserialized.members.unwrap().len(), 2);
    }

    #[test]
    fn test_iam_role_serde_roundtrip() {
        let role = IamRole {
            role_name: "lambda-role".to_string(),
            role_id: "AROAEXAMPLE".to_string(),
            arn: "arn:aws:iam::123456789012:role/lambda-role".to_string(),
            create_date: None,
            path: Some("/".to_string()),
            description: Some("Role for Lambda".to_string()),
            max_session_duration: Some(3600),
            assume_role_policy: None,
            tags: None,
        };

        let json = serde_json::to_string(&role).unwrap();
        let deserialized: IamRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role_name, "lambda-role");
        assert_eq!(deserialized.max_session_duration, Some(3600));
    }

    #[test]
    fn test_iam_managed_policy_serde_roundtrip() {
        let policy = IamManagedPolicy {
            policy_name: "ReadOnlyAccess".to_string(),
            policy_id: "ANPAEXAMPLE".to_string(),
            arn: "arn:aws:iam::aws:policy/ReadOnlyAccess".to_string(),
            path: Some("/".to_string()),
            default_version_id: Some("v1".to_string()),
            attachment_count: Some(5),
            description: Some("Provides read-only access".to_string()),
            policy_document: None,
        };

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: IamManagedPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.policy_name, "ReadOnlyAccess");
        assert_eq!(deserialized.attachment_count, Some(5));
    }

    #[test]
    fn test_policy_attachment_serde() {
        let attachment = PolicyAttachment {
            user_name: Some("test-user".to_string()),
            group_name: None,
            role_name: None,
            policy_name: None,
            policy_arn: Some("arn:aws:iam::aws:policy/ReadOnlyAccess".to_string()),
            policy_type: "managed".to_string(),
        };

        let json = serde_json::to_string(&attachment).unwrap();
        let deserialized: PolicyAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.policy_type, "managed");
        assert!(deserialized.group_name.is_none());
    }

    #[test]
    fn test_user_group_membership_serde() {
        let membership = UserGroupMembership {
            user_name: "alice".to_string(),
            group_name: "developers".to_string(),
        };

        let json = serde_json::to_string(&membership).unwrap();
        let deserialized: UserGroupMembership = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_name, "alice");
        assert_eq!(deserialized.group_name, "developers");
    }
}
