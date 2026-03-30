//! AWS SDK S3クライアントの本番実装

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_s3::Client as S3Client;
use std::collections::HashMap;

use super::s3_client_trait::{
    S3BucketDetail, S3BucketInfo, S3BucketPolicyInfo, S3ClientOps, S3LifecycleRuleInfo,
    S3TransitionInfo,
};

/// AWS SDK S3クライアントをラップした本番実装
pub struct RealS3Client {
    client: S3Client,
}

impl RealS3Client {
    pub fn new(client: S3Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl S3ClientOps for RealS3Client {
    async fn list_buckets(&self) -> Result<Vec<S3BucketInfo>> {
        let output = self
            .client
            .list_buckets()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list S3 buckets: {}", e))?;

        let buckets = output
            .buckets()
            .iter()
            .map(|b| S3BucketInfo {
                name: b.name().unwrap_or_default().to_string(),
                creation_date: b.creation_date().map(|d| d.to_string()),
                region: None,
            })
            .collect();

        Ok(buckets)
    }

    async fn get_bucket_detail(&self, bucket_name: &str) -> Result<S3BucketDetail> {
        // バージョニング
        let versioning = match self
            .client
            .get_bucket_versioning()
            .bucket(bucket_name)
            .send()
            .await
        {
            Ok(v) => v.status().map(|s| s.as_str().to_string()),
            Err(_) => None,
        };

        // 暗号化
        let (encryption_algorithm, kms_key_id) = match self
            .client
            .get_bucket_encryption()
            .bucket(bucket_name)
            .send()
            .await
        {
            Ok(enc) => {
                let rule = enc
                    .server_side_encryption_configuration()
                    .and_then(|c| c.rules().first())
                    .and_then(|r| r.apply_server_side_encryption_by_default());
                let algo = rule.map(|r| r.sse_algorithm().as_str().to_string());
                let key = rule.and_then(|r| r.kms_master_key_id().map(|k| k.to_string()));
                (algo, key)
            }
            Err(_) => (None, None),
        };

        // タグ
        let tags = match self
            .client
            .get_bucket_tagging()
            .bucket(bucket_name)
            .send()
            .await
        {
            Ok(t) => t
                .tag_set()
                .iter()
                .map(|tag| (tag.key().to_string(), tag.value().to_string()))
                .collect(),
            Err(_) => HashMap::new(),
        };

        Ok(S3BucketDetail {
            versioning,
            encryption_algorithm,
            kms_key_id,
            tags,
        })
    }

    async fn get_bucket_policy(&self, bucket_name: &str) -> Result<Option<S3BucketPolicyInfo>> {
        match self
            .client
            .get_bucket_policy()
            .bucket(bucket_name)
            .send()
            .await
        {
            Ok(output) => {
                if let Some(policy) = output.policy() {
                    Ok(Some(S3BucketPolicyInfo {
                        bucket: bucket_name.to_string(),
                        policy: policy.to_string(),
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    async fn get_lifecycle_rules(&self, bucket_name: &str) -> Result<Vec<S3LifecycleRuleInfo>> {
        match self
            .client
            .get_bucket_lifecycle_configuration()
            .bucket(bucket_name)
            .send()
            .await
        {
            Ok(output) => {
                let rules = output
                    .rules()
                    .iter()
                    .map(|rule| {
                        let transitions = rule
                            .transitions()
                            .iter()
                            .map(|t| S3TransitionInfo {
                                days: t.days().unwrap_or(0),
                                storage_class: t
                                    .storage_class()
                                    .map(|s| s.as_str().to_string())
                                    .unwrap_or_default(),
                            })
                            .collect();

                        S3LifecycleRuleInfo {
                            id: rule.id().unwrap_or_default().to_string(),
                            status: rule.status().as_str().to_string(),
                            prefix: rule
                                .filter()
                                .and_then(|f| f.prefix())
                                .map(|p| p.to_string()),
                            expiration_days: rule.expiration().and_then(|e| e.days()),
                            transitions,
                        }
                    })
                    .collect();
                Ok(rules)
            }
            Err(_) => Ok(Vec::new()),
        }
    }
}
