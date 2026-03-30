//! S3クライアント操作の抽象化トレイト

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// S3バケット情報
#[derive(Debug, Clone)]
pub struct S3BucketInfo {
    pub name: String,
    pub creation_date: Option<String>,
    pub region: Option<String>,
}

/// S3バケットの詳細情報
#[derive(Debug, Clone)]
pub struct S3BucketDetail {
    pub versioning: Option<String>,
    pub encryption_algorithm: Option<String>,
    pub kms_key_id: Option<String>,
    pub tags: HashMap<String, String>,
}

/// S3バケットポリシー情報
#[derive(Debug, Clone)]
pub struct S3BucketPolicyInfo {
    pub bucket: String,
    pub policy: String,
}

/// S3ライフサイクルルール情報
#[derive(Debug, Clone)]
pub struct S3LifecycleRuleInfo {
    pub id: String,
    pub status: String,
    pub prefix: Option<String>,
    pub expiration_days: Option<i32>,
    pub transitions: Vec<S3TransitionInfo>,
}

/// S3遷移情報
#[derive(Debug, Clone)]
pub struct S3TransitionInfo {
    pub days: i32,
    pub storage_class: String,
}

/// S3クライアント操作を抽象化するトレイト
#[async_trait]
pub trait S3ClientOps: Send + Sync {
    /// S3バケット一覧を取得
    async fn list_buckets(&self) -> Result<Vec<S3BucketInfo>>;

    /// バケットの詳細情報を取得（バージョニング、暗号化、タグ）
    async fn get_bucket_detail(&self, bucket_name: &str) -> Result<S3BucketDetail>;

    /// バケットポリシーを取得
    async fn get_bucket_policy(&self, bucket_name: &str) -> Result<Option<S3BucketPolicyInfo>>;

    /// ライフサイクル設定を取得
    async fn get_lifecycle_rules(&self, bucket_name: &str) -> Result<Vec<S3LifecycleRuleInfo>>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub S3Client {}

        #[async_trait]
        impl S3ClientOps for S3Client {
            async fn list_buckets(&self) -> Result<Vec<S3BucketInfo>>;
            async fn get_bucket_detail(&self, bucket_name: &str) -> Result<S3BucketDetail>;
            async fn get_bucket_policy(&self, bucket_name: &str) -> Result<Option<S3BucketPolicyInfo>>;
            async fn get_lifecycle_rules(&self, bucket_name: &str) -> Result<Vec<S3LifecycleRuleInfo>>;
        }
    }
}
