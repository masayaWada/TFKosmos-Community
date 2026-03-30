//! AWS SDK IAMクライアントの本番実装
//!
//! このモジュールは、`IamClientOps`トレイトの本番実装を提供します。

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_iam::Client as IamClient;
use std::collections::HashMap;

use super::iam_client_trait::{
    IamClientOps, IamGroupInfo, IamPolicyInfo, IamRoleInfo, IamUserInfo, PolicyAttachment,
    PolicyDocument,
};

/// AWS SDK IAMクライアントをラップした本番実装
pub struct RealIamClient {
    client: IamClient,
}

impl RealIamClient {
    pub fn new(client: IamClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl IamClientOps for RealIamClient {
    async fn list_users(&self) -> Result<Vec<IamUserInfo>> {
        // デフォルトはタグ情報を取得
        self.list_users_with_options(true).await
    }

    async fn list_users_with_options(&self, include_tags: bool) -> Result<Vec<IamUserInfo>> {
        let mut users = Vec::new();
        let mut paginator = self
            .client
            .list_users()
            .into_paginator()
            .page_size(100)
            .send();

        while let Some(page_result) = paginator.next().await {
            let page = page_result.map_err(|e| anyhow!("Failed to list users: {}", e))?;

            for user in page.users() {
                let user_name = user.user_name().to_string();

                // タグを取得（include_tagsがtrueの場合のみ）
                let tags = if include_tags {
                    match self
                        .client
                        .list_user_tags()
                        .user_name(&user_name)
                        .send()
                        .await
                    {
                        Ok(tags_result) => tags_result
                            .tags()
                            .iter()
                            .map(|tag| (tag.key().to_string(), tag.value().to_string()))
                            .collect(),
                        Err(_) => HashMap::new(),
                    }
                } else {
                    HashMap::new()
                };

                users.push(IamUserInfo {
                    user_name,
                    user_id: user.user_id().to_string(),
                    arn: user.arn().to_string(),
                    create_date: user.create_date().secs(),
                    path: user.path().to_string(),
                    tags,
                });
            }
        }

        Ok(users)
    }

    async fn list_groups(&self) -> Result<Vec<IamGroupInfo>> {
        let mut groups = Vec::new();
        let mut paginator = self
            .client
            .list_groups()
            .into_paginator()
            .page_size(100)
            .send();

        while let Some(page_result) = paginator.next().await {
            let page = page_result.map_err(|e| anyhow!("Failed to list groups: {}", e))?;

            for group in page.groups() {
                groups.push(IamGroupInfo {
                    group_name: group.group_name().to_string(),
                    group_id: group.group_id().to_string(),
                    arn: group.arn().to_string(),
                    create_date: group.create_date().secs(),
                    path: group.path().to_string(),
                });
            }
        }

        Ok(groups)
    }

    async fn list_roles(&self) -> Result<Vec<IamRoleInfo>> {
        // デフォルトはタグ情報を取得
        self.list_roles_with_options(true).await
    }

    async fn list_roles_with_options(&self, include_tags: bool) -> Result<Vec<IamRoleInfo>> {
        let mut roles = Vec::new();
        let mut paginator = self
            .client
            .list_roles()
            .into_paginator()
            .page_size(100)
            .send();

        while let Some(page_result) = paginator.next().await {
            let page = page_result.map_err(|e| anyhow!("Failed to list roles: {}", e))?;

            for role in page.roles() {
                let role_name = role.role_name().to_string();

                // タグを取得（include_tagsがtrueの場合のみ）
                let tags = if include_tags {
                    match self
                        .client
                        .list_role_tags()
                        .role_name(&role_name)
                        .send()
                        .await
                    {
                        Ok(tags_result) => tags_result
                            .tags()
                            .iter()
                            .map(|tag| (tag.key().to_string(), tag.value().to_string()))
                            .collect(),
                        Err(_) => HashMap::new(),
                    }
                } else {
                    HashMap::new()
                };

                roles.push(IamRoleInfo {
                    role_name,
                    role_id: role.role_id().to_string(),
                    arn: role.arn().to_string(),
                    create_date: role.create_date().secs(),
                    path: role.path().to_string(),
                    assume_role_policy_document: role
                        .assume_role_policy_document()
                        .map(|s| s.to_string()),
                    tags,
                });
            }
        }

        Ok(roles)
    }

    async fn list_policies(&self) -> Result<Vec<IamPolicyInfo>> {
        let mut policies = Vec::new();
        let mut paginator = self
            .client
            .list_policies()
            .scope(aws_sdk_iam::types::PolicyScopeType::Local)
            .into_paginator()
            .page_size(100)
            .send();

        while let Some(page_result) = paginator.next().await {
            let page = page_result.map_err(|e| anyhow!("Failed to list policies: {}", e))?;

            for policy in page.policies() {
                policies.push(IamPolicyInfo {
                    policy_name: policy.policy_name().unwrap_or("").to_string(),
                    policy_id: policy.policy_id().unwrap_or("").to_string(),
                    arn: policy.arn().unwrap_or("").to_string(),
                    path: policy.path().unwrap_or("/").to_string(),
                    default_version_id: policy.default_version_id().unwrap_or("").to_string(),
                    attachment_count: policy.attachment_count().unwrap_or(0),
                    create_date: policy.create_date().map(|dt| dt.secs()).unwrap_or(0),
                    update_date: policy.update_date().map(|dt| dt.secs()).unwrap_or(0),
                    description: policy.description().unwrap_or("").to_string(),
                });
            }
        }

        Ok(policies)
    }

    async fn list_user_policies(&self, user_name: &str) -> Result<Vec<String>> {
        let result = self
            .client
            .list_user_policies()
            .user_name(user_name)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list user policies: {}", e))?;

        Ok(result.policy_names().to_vec())
    }

    async fn list_attached_user_policies(&self, user_name: &str) -> Result<Vec<PolicyAttachment>> {
        let result = self
            .client
            .list_attached_user_policies()
            .user_name(user_name)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list attached user policies: {}", e))?;

        Ok(result
            .attached_policies()
            .iter()
            .map(|p| PolicyAttachment {
                policy_arn: p.policy_arn().unwrap_or("").to_string(),
                policy_name: p.policy_name().map(|s| s.to_string()),
            })
            .collect())
    }

    async fn list_group_policies(&self, group_name: &str) -> Result<Vec<String>> {
        let result = self
            .client
            .list_group_policies()
            .group_name(group_name)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list group policies: {}", e))?;

        Ok(result.policy_names().to_vec())
    }

    async fn list_attached_group_policies(
        &self,
        group_name: &str,
    ) -> Result<Vec<PolicyAttachment>> {
        let result = self
            .client
            .list_attached_group_policies()
            .group_name(group_name)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list attached group policies: {}", e))?;

        Ok(result
            .attached_policies()
            .iter()
            .map(|p| PolicyAttachment {
                policy_arn: p.policy_arn().unwrap_or("").to_string(),
                policy_name: p.policy_name().map(|s| s.to_string()),
            })
            .collect())
    }

    async fn list_role_policies(&self, role_name: &str) -> Result<Vec<String>> {
        let result = self
            .client
            .list_role_policies()
            .role_name(role_name)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list role policies: {}", e))?;

        Ok(result.policy_names().to_vec())
    }

    async fn list_attached_role_policies(&self, role_name: &str) -> Result<Vec<PolicyAttachment>> {
        let result = self
            .client
            .list_attached_role_policies()
            .role_name(role_name)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list attached role policies: {}", e))?;

        Ok(result
            .attached_policies()
            .iter()
            .map(|p| PolicyAttachment {
                policy_arn: p.policy_arn().unwrap_or("").to_string(),
                policy_name: p.policy_name().map(|s| s.to_string()),
            })
            .collect())
    }

    async fn list_groups_for_user(&self, user_name: &str) -> Result<Vec<String>> {
        let result = self
            .client
            .list_groups_for_user()
            .user_name(user_name)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list groups for user: {}", e))?;

        Ok(result
            .groups()
            .iter()
            .map(|g| g.group_name().to_string())
            .collect())
    }

    async fn get_policy_version(
        &self,
        policy_arn: &str,
        version_id: &str,
    ) -> Result<Option<PolicyDocument>> {
        let result = self
            .client
            .get_policy_version()
            .policy_arn(policy_arn)
            .version_id(version_id)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to get policy version: {}", e))?;

        Ok(result.policy_version().and_then(|pv| {
            pv.document().map(|doc| PolicyDocument {
                document: doc.to_string(),
            })
        }))
    }
}
