//! IAMクライアント操作の抽象化トレイト
//!
//! このモジュールは、AWS IAMクライアントの操作を抽象化し、
//! テスト時にモック実装を注入できるようにします。

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// IAMユーザー情報
#[derive(Debug, Clone)]
pub struct IamUserInfo {
    pub user_name: String,
    pub user_id: String,
    pub arn: String,
    pub create_date: i64,
    pub path: String,
    pub tags: HashMap<String, String>,
}

/// IAMグループ情報
#[derive(Debug, Clone)]
pub struct IamGroupInfo {
    pub group_name: String,
    pub group_id: String,
    pub arn: String,
    pub create_date: i64,
    pub path: String,
}

/// IAMロール情報
#[derive(Debug, Clone)]
pub struct IamRoleInfo {
    pub role_name: String,
    pub role_id: String,
    pub arn: String,
    pub create_date: i64,
    pub path: String,
    pub assume_role_policy_document: Option<String>,
    pub tags: HashMap<String, String>,
}

/// IAMポリシー情報
#[derive(Debug, Clone)]
pub struct IamPolicyInfo {
    pub policy_name: String,
    pub policy_id: String,
    pub arn: String,
    pub path: String,
    pub default_version_id: String,
    pub attachment_count: i32,
    pub create_date: i64,
    pub update_date: i64,
    pub description: String,
}

/// ポリシーアタッチメント情報
#[derive(Debug, Clone)]
pub struct PolicyAttachment {
    pub policy_arn: String,
    #[allow(dead_code)]
    pub policy_name: Option<String>,
}

/// ポリシードキュメント情報
#[derive(Debug, Clone)]
pub struct PolicyDocument {
    pub document: String,
}

/// IAMクライアント操作を抽象化するトレイト
///
/// このトレイトを実装することで、本番用のAWS SDKクライアントと
/// テスト用のモッククライアントを切り替えることができます。
#[async_trait]
pub trait IamClientOps: Send + Sync {
    /// IAMユーザー一覧を取得（タグ情報付き）
    ///
    /// これは `list_users_with_options(true)` のエイリアスです。
    /// テストや互換性のために維持されています。
    #[allow(dead_code)]
    async fn list_users(&self) -> Result<Vec<IamUserInfo>>;

    /// IAMユーザー一覧を取得（オプション付き）
    ///
    /// # Arguments
    /// * `include_tags` - タグ情報を取得するかどうか。falseの場合、tagsは空のHashMapになる
    async fn list_users_with_options(&self, include_tags: bool) -> Result<Vec<IamUserInfo>>;

    /// IAMグループ一覧を取得
    async fn list_groups(&self) -> Result<Vec<IamGroupInfo>>;

    /// IAMロール一覧を取得（タグ情報付き）
    ///
    /// これは `list_roles_with_options(true)` のエイリアスです。
    /// テストや互換性のために維持されています。
    #[allow(dead_code)]
    async fn list_roles(&self) -> Result<Vec<IamRoleInfo>>;

    /// IAMロール一覧を取得（オプション付き）
    ///
    /// # Arguments
    /// * `include_tags` - タグ情報を取得するかどうか。falseの場合、tagsは空のHashMapになる
    async fn list_roles_with_options(&self, include_tags: bool) -> Result<Vec<IamRoleInfo>>;

    /// IAMポリシー一覧を取得（ローカルスコープのみ）
    async fn list_policies(&self) -> Result<Vec<IamPolicyInfo>>;

    /// ユーザーのインラインポリシー名一覧を取得
    async fn list_user_policies(&self, user_name: &str) -> Result<Vec<String>>;

    /// ユーザーにアタッチされたマネージドポリシー一覧を取得
    async fn list_attached_user_policies(&self, user_name: &str) -> Result<Vec<PolicyAttachment>>;

    /// グループのインラインポリシー名一覧を取得
    async fn list_group_policies(&self, group_name: &str) -> Result<Vec<String>>;

    /// グループにアタッチされたマネージドポリシー一覧を取得
    async fn list_attached_group_policies(&self, group_name: &str)
        -> Result<Vec<PolicyAttachment>>;

    /// ロールのインラインポリシー名一覧を取得
    async fn list_role_policies(&self, role_name: &str) -> Result<Vec<String>>;

    /// ロールにアタッチされたマネージドポリシー一覧を取得
    async fn list_attached_role_policies(&self, role_name: &str) -> Result<Vec<PolicyAttachment>>;

    /// ユーザーが所属するグループ一覧を取得
    async fn list_groups_for_user(&self, user_name: &str) -> Result<Vec<String>>;

    /// ポリシーバージョンのドキュメントを取得
    async fn get_policy_version(
        &self,
        policy_arn: &str,
        version_id: &str,
    ) -> Result<Option<PolicyDocument>>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub IamClient {}

        #[async_trait]
        impl IamClientOps for IamClient {
            async fn list_users(&self) -> Result<Vec<IamUserInfo>>;
            async fn list_users_with_options(&self, include_tags: bool) -> Result<Vec<IamUserInfo>>;
            async fn list_groups(&self) -> Result<Vec<IamGroupInfo>>;
            async fn list_roles(&self) -> Result<Vec<IamRoleInfo>>;
            async fn list_roles_with_options(&self, include_tags: bool) -> Result<Vec<IamRoleInfo>>;
            async fn list_policies(&self) -> Result<Vec<IamPolicyInfo>>;
            async fn list_user_policies(&self, user_name: &str) -> Result<Vec<String>>;
            async fn list_attached_user_policies(&self, user_name: &str) -> Result<Vec<PolicyAttachment>>;
            async fn list_group_policies(&self, group_name: &str) -> Result<Vec<String>>;
            async fn list_attached_group_policies(&self, group_name: &str) -> Result<Vec<PolicyAttachment>>;
            async fn list_role_policies(&self, role_name: &str) -> Result<Vec<String>>;
            async fn list_attached_role_policies(&self, role_name: &str) -> Result<Vec<PolicyAttachment>>;
            async fn list_groups_for_user(&self, user_name: &str) -> Result<Vec<String>>;
            async fn get_policy_version(&self, policy_arn: &str, version_id: &str) -> Result<Option<PolicyDocument>>;
        }
    }
}
