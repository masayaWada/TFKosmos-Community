/// テストデータビルダー
///
/// ビルダーパターンでテストデータを簡単に作成するためのヘルパー

use crate::domain::iam::{IAMUser, IAMGroup, IAMRole, IAMPolicy};

/// IAMユーザービルダー
pub struct IAMUserBuilder {
    id: String,
    name: String,
    arn: String,
    created_at: Option<String>,
}

impl IAMUserBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            id: format!("AIDA{}", name.to_uppercase()),
            name: name.to_string(),
            arn: format!("arn:aws:iam::123456789012:user/{}", name),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn arn(mut self, arn: &str) -> Self {
        self.arn = arn.to_string();
        self
    }

    pub fn created_at(mut self, created_at: &str) -> Self {
        self.created_at = Some(created_at.to_string());
        self
    }

    pub fn build(self) -> IAMUser {
        IAMUser {
            id: self.id,
            name: self.name,
            arn: self.arn,
            path: "/".to_string(),
            created_at: self.created_at,
            tags: None,
            inline_policies: vec![],
            managed_policy_arns: vec![],
            groups: vec![],
        }
    }
}

/// IAMグループビルダー
pub struct IAMGroupBuilder {
    id: String,
    name: String,
    arn: String,
}

impl IAMGroupBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            id: format!("AGPA{}", name.to_uppercase()),
            name: name.to_string(),
            arn: format!("arn:aws:iam::123456789012:group/{}", name),
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn arn(mut self, arn: &str) -> Self {
        self.arn = arn.to_string();
        self
    }

    pub fn build(self) -> IAMGroup {
        IAMGroup {
            id: self.id,
            name: self.name,
            arn: self.arn,
            path: "/".to_string(),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            inline_policies: vec![],
            managed_policy_arns: vec![],
            members: vec![],
        }
    }
}

/// IAMロールビルダー
pub struct IAMRoleBuilder {
    id: String,
    name: String,
    arn: String,
    assume_role_policy: String,
}

impl IAMRoleBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            id: format!("AROA{}", name.to_uppercase()),
            name: name.to_string(),
            arn: format!("arn:aws:iam::123456789012:role/{}", name),
            assume_role_policy: r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"lambda.amazonaws.com"},"Action":"sts:AssumeRole"}]}"#.to_string(),
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn arn(mut self, arn: &str) -> Self {
        self.arn = arn.to_string();
        self
    }

    pub fn assume_role_policy(mut self, policy: &str) -> Self {
        self.assume_role_policy = policy.to_string();
        self
    }

    pub fn build(self) -> IAMRole {
        IAMRole {
            id: self.id,
            name: self.name,
            arn: self.arn,
            path: "/".to_string(),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            assume_role_policy_document: self.assume_role_policy,
            inline_policies: vec![],
            managed_policy_arns: vec![],
            max_session_duration: Some(3600),
            description: None,
            tags: None,
        }
    }
}

/// IAMポリシービルダー
pub struct IAMPolicyBuilder {
    id: String,
    name: String,
    arn: String,
}

impl IAMPolicyBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            id: format!("ANPA{}", name.to_uppercase()),
            name: name.to_string(),
            arn: format!("arn:aws:iam::123456789012:policy/{}", name),
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn arn(mut self, arn: &str) -> Self {
        self.arn = arn.to_string();
        self
    }

    pub fn build(self) -> IAMPolicy {
        IAMPolicy {
            id: self.id,
            name: self.name,
            arn: self.arn,
            path: "/".to_string(),
            default_version_id: Some("v1".to_string()),
            attachment_count: Some(0),
            permissions_boundary_usage_count: Some(0),
            is_attachable: Some(true),
            description: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            tags: None,
            policy_document: r#"{"Version":"2012-10-17","Statement":[]}"#.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iam_user_builder() {
        let user = IAMUserBuilder::new("test-user")
            .id("AIDATEST123")
            .build();

        assert_eq!(user.id, "AIDATEST123");
        assert_eq!(user.name, "test-user");
        assert!(user.arn.contains("test-user"));
    }

    #[test]
    fn test_iam_group_builder() {
        let group = IAMGroupBuilder::new("test-group").build();

        assert_eq!(group.name, "test-group");
        assert!(group.arn.contains("test-group"));
    }

    #[test]
    fn test_iam_role_builder() {
        let role = IAMRoleBuilder::new("test-role").build();

        assert_eq!(role.name, "test-role");
        assert!(role.arn.contains("test-role"));
        assert!(!role.assume_role_policy_document.is_empty());
    }

    #[test]
    fn test_iam_policy_builder() {
        let policy = IAMPolicyBuilder::new("test-policy").build();

        assert_eq!(policy.name, "test-policy");
        assert!(policy.arn.contains("test-policy"));
    }
}
