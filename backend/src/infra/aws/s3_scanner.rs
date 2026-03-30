//! AWS S3スキャナー

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

use crate::infra::aws::s3_client_trait::S3ClientOps;
use crate::models::ScanConfig;

/// AWS S3スキャナー
pub struct AwsS3Scanner<C: S3ClientOps> {
    config: ScanConfig,
    s3_client: Arc<C>,
}

impl<C: S3ClientOps> AwsS3Scanner<C> {
    /// 本番用・テスト用共通：クライアントを指定してスキャナーを作成
    pub fn new(config: ScanConfig, client: Arc<C>) -> Self {
        Self {
            config,
            s3_client: client,
        }
    }

    /// テスト用：モッククライアントを使用してスキャナーを作成
    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: C) -> Self {
        Self {
            config,
            s3_client: Arc::new(client),
        }
    }

    /// S3リソースをスキャンし結果をresultsに追加
    pub async fn scan_into(
        &self,
        results: &mut serde_json::Map<String, Value>,
        progress_callback: &(dyn Fn(u32, String) + Send + Sync),
        completed_targets: &AtomicUsize,
        total_targets: usize,
    ) -> Result<()> {
        let scan_targets = &self.config.scan_targets;

        // Buckets
        if scan_targets.get("buckets").copied().unwrap_or(false) {
            debug!("S3 Bucketsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "S3 Bucketsのスキャン中...".to_string(),
            );
            let buckets = self.scan_buckets().await?;
            let count = buckets.len();
            results.insert("buckets".to_string(), Value::Array(buckets));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            debug!(count, "S3 Bucketsのスキャン完了");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("S3 Bucketsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("buckets".to_string(), Value::Array(Vec::new()));
        }

        // Bucket Policies
        if scan_targets
            .get("bucket_policies")
            .copied()
            .unwrap_or(false)
        {
            debug!("S3 Bucket Policiesのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "S3 Bucket Policiesのスキャン中...".to_string(),
            );
            let policies = self.scan_bucket_policies().await?;
            let count = policies.len();
            results.insert("bucket_policies".to_string(), Value::Array(policies));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            debug!(count, "S3 Bucket Policiesのスキャン完了");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("S3 Bucket Policiesのスキャン完了: {}件", count),
            );
        } else {
            results.insert("bucket_policies".to_string(), Value::Array(Vec::new()));
        }

        // Lifecycle Rules
        if scan_targets
            .get("lifecycle_rules")
            .copied()
            .unwrap_or(false)
        {
            debug!("S3 Lifecycle Rulesのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "S3 Lifecycle Rulesのスキャン中...".to_string(),
            );
            let rules = self.scan_lifecycle_rules().await?;
            let count = rules.len();
            results.insert("lifecycle_rules".to_string(), Value::Array(rules));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            debug!(count, "S3 Lifecycle Rulesのスキャン完了");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("S3 Lifecycle Rulesのスキャン完了: {}件", count),
            );
        } else {
            results.insert("lifecycle_rules".to_string(), Value::Array(Vec::new()));
        }

        Ok(())
    }

    /// S3バケットをスキャン
    async fn scan_buckets(&self) -> Result<Vec<Value>> {
        let buckets_info = self.s3_client.list_buckets().await?;
        let mut buckets = Vec::new();

        for bucket in buckets_info {
            if !self.apply_name_filter(&bucket.name) {
                continue;
            }

            let detail = self.s3_client.get_bucket_detail(&bucket.name).await;
            let mut bucket_json = json!({
                "name": bucket.name,
                "arn": format!("arn:aws:s3:::{}", bucket.name),
            });

            if let Some(date) = &bucket.creation_date {
                bucket_json["creation_date"] = json!(date);
            }
            if let Some(region) = &bucket.region {
                bucket_json["region"] = json!(region);
            }

            if let Ok(detail) = detail {
                if let Some(versioning) = &detail.versioning {
                    bucket_json["versioning"] = json!(versioning);
                }
                if let Some(algo) = &detail.encryption_algorithm {
                    let mut encryption = json!({ "sse_algorithm": algo });
                    if let Some(key_id) = &detail.kms_key_id {
                        encryption["kms_master_key_id"] = json!(key_id);
                    }
                    bucket_json["encryption"] = encryption;
                }
                if !detail.tags.is_empty() {
                    bucket_json["tags"] = json!(detail.tags);
                }
            }

            buckets.push(bucket_json);
        }

        info!(count = buckets.len(), "S3バケットスキャン完了");
        Ok(buckets)
    }

    /// バケットポリシーをスキャン
    async fn scan_bucket_policies(&self) -> Result<Vec<Value>> {
        let buckets_info = self.s3_client.list_buckets().await?;
        let mut policies = Vec::new();

        for bucket in buckets_info {
            if !self.apply_name_filter(&bucket.name) {
                continue;
            }

            if let Ok(Some(policy_info)) = self.s3_client.get_bucket_policy(&bucket.name).await {
                let policy_value: serde_json::Value =
                    serde_json::from_str(&policy_info.policy).unwrap_or(json!(policy_info.policy));
                policies.push(json!({
                    "bucket": policy_info.bucket,
                    "policy": policy_value,
                }));
            }
        }

        Ok(policies)
    }

    /// ライフサイクルルールをスキャン
    async fn scan_lifecycle_rules(&self) -> Result<Vec<Value>> {
        let buckets_info = self.s3_client.list_buckets().await?;
        let mut all_rules = Vec::new();

        for bucket in buckets_info {
            if !self.apply_name_filter(&bucket.name) {
                continue;
            }

            if let Ok(rules) = self.s3_client.get_lifecycle_rules(&bucket.name).await {
                for rule in rules {
                    let transitions: Vec<Value> = rule
                        .transitions
                        .iter()
                        .map(|t| {
                            json!({
                                "days": t.days,
                                "storage_class": t.storage_class,
                            })
                        })
                        .collect();

                    let mut rule_json = json!({
                        "bucket": bucket.name,
                        "id": rule.id,
                        "status": rule.status,
                    });

                    if let Some(prefix) = &rule.prefix {
                        rule_json["prefix"] = json!(prefix);
                    }
                    if let Some(days) = rule.expiration_days {
                        rule_json["expiration_days"] = json!(days);
                    }
                    if !transitions.is_empty() {
                        rule_json["transition"] = json!(transitions);
                    }

                    all_rules.push(rule_json);
                }
            }
        }

        Ok(all_rules)
    }

    fn apply_name_filter(&self, name: &str) -> bool {
        if let Some(prefix) = self.config.filters.get("name_prefix") {
            name.starts_with(prefix)
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::s3_client_trait::mock::MockS3Client;
    use crate::infra::aws::s3_client_trait::{
        S3BucketDetail, S3BucketInfo, S3BucketPolicyInfo, S3LifecycleRuleInfo, S3TransitionInfo,
    };
    use std::collections::HashMap;

    fn make_test_config(targets: Vec<&str>) -> ScanConfig {
        let mut scan_targets = HashMap::new();
        for t in targets {
            scan_targets.insert(t.to_string(), true);
        }
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
            filters: HashMap::new(),
            include_tags: true,
        }
    }

    #[tokio::test]
    async fn test_scan_buckets() {
        let mut mock = MockS3Client::new();
        mock.expect_list_buckets().returning(|| {
            Ok(vec![S3BucketInfo {
                name: "test-bucket".to_string(),
                creation_date: Some("2024-01-01".to_string()),
                region: Some("ap-northeast-1".to_string()),
            }])
        });
        mock.expect_get_bucket_detail().returning(|_| {
            Ok(S3BucketDetail {
                versioning: Some("Enabled".to_string()),
                encryption_algorithm: Some("AES256".to_string()),
                kms_key_id: None,
                tags: HashMap::new(),
            })
        });

        let config = make_test_config(vec!["buckets"]);
        let scanner = AwsS3Scanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let buckets = results.get("buckets").unwrap().as_array().unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0]["name"], "test-bucket");
        assert_eq!(buckets[0]["versioning"], "Enabled");
    }

    #[tokio::test]
    async fn test_scan_bucket_policies() {
        let mut mock = MockS3Client::new();
        mock.expect_list_buckets().returning(|| {
            Ok(vec![S3BucketInfo {
                name: "policy-bucket".to_string(),
                creation_date: None,
                region: None,
            }])
        });
        mock.expect_get_bucket_policy().returning(|_| {
            Ok(Some(S3BucketPolicyInfo {
                bucket: "policy-bucket".to_string(),
                policy: r#"{"Version":"2012-10-17","Statement":[]}"#.to_string(),
            }))
        });

        let config = make_test_config(vec!["bucket_policies"]);
        let scanner = AwsS3Scanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let policies = results.get("bucket_policies").unwrap().as_array().unwrap();
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0]["bucket"], "policy-bucket");
    }

    #[tokio::test]
    async fn test_scan_lifecycle_rules() {
        let mut mock = MockS3Client::new();
        mock.expect_list_buckets().returning(|| {
            Ok(vec![S3BucketInfo {
                name: "lifecycle-bucket".to_string(),
                creation_date: None,
                region: None,
            }])
        });
        mock.expect_get_lifecycle_rules().returning(|_| {
            Ok(vec![S3LifecycleRuleInfo {
                id: "archive-rule".to_string(),
                status: "Enabled".to_string(),
                prefix: Some("logs/".to_string()),
                expiration_days: Some(365),
                transitions: vec![S3TransitionInfo {
                    days: 30,
                    storage_class: "GLACIER".to_string(),
                }],
            }])
        });

        let config = make_test_config(vec!["lifecycle_rules"]);
        let scanner = AwsS3Scanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let rules = results.get("lifecycle_rules").unwrap().as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["id"], "archive-rule");
    }

    #[tokio::test]
    async fn test_name_prefix_filter() {
        let mut mock = MockS3Client::new();
        mock.expect_list_buckets().returning(|| {
            Ok(vec![
                S3BucketInfo {
                    name: "prod-bucket".to_string(),
                    creation_date: None,
                    region: None,
                },
                S3BucketInfo {
                    name: "dev-bucket".to_string(),
                    creation_date: None,
                    region: None,
                },
            ])
        });
        mock.expect_get_bucket_detail().returning(|_| {
            Ok(S3BucketDetail {
                versioning: None,
                encryption_algorithm: None,
                kms_key_id: None,
                tags: HashMap::new(),
            })
        });

        let mut config = make_test_config(vec!["buckets"]);
        config
            .filters
            .insert("name_prefix".to_string(), "prod".to_string());
        let scanner = AwsS3Scanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let buckets = results.get("buckets").unwrap().as_array().unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0]["name"], "prod-bucket");
    }
}
