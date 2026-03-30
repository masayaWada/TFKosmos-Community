#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use super::aws_iam::Tag;

/// S3バケット
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Bucket {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versioning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<S3Encryption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// S3バケット暗号化設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Encryption {
    pub sse_algorithm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kms_master_key_id: Option<String>,
}

/// S3バケットポリシー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3BucketPolicy {
    pub bucket: String,
    pub policy: serde_json::Value,
}

/// S3ライフサイクルルール
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3LifecycleRule {
    pub bucket: String,
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_days: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<Vec<S3Transition>>,
}

/// S3ライフサイクル遷移設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Transition {
    pub days: i32,
    pub storage_class: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3_bucket_serde_roundtrip() {
        let bucket = S3Bucket {
            name: "my-bucket".to_string(),
            arn: Some("arn:aws:s3:::my-bucket".to_string()),
            region: Some("ap-northeast-1".to_string()),
            creation_date: Some("2024-01-01T00:00:00Z".to_string()),
            versioning: Some("Enabled".to_string()),
            encryption: Some(S3Encryption {
                sse_algorithm: "aws:kms".to_string(),
                kms_master_key_id: Some(
                    "arn:aws:kms:ap-northeast-1:123456789012:key/test".to_string(),
                ),
            }),
            tags: Some(vec![Tag {
                key: "Environment".to_string(),
                value: "Production".to_string(),
            }]),
        };

        let json = serde_json::to_string(&bucket).unwrap();
        let deserialized: S3Bucket = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "my-bucket");
        assert_eq!(deserialized.versioning, Some("Enabled".to_string()));
    }

    #[test]
    fn test_s3_bucket_minimal_serde() {
        let bucket = S3Bucket {
            name: "simple-bucket".to_string(),
            arn: None,
            region: None,
            creation_date: None,
            versioning: None,
            encryption: None,
            tags: None,
        };

        let json = serde_json::to_string(&bucket).unwrap();
        assert!(!json.contains("arn"));
        let deserialized: S3Bucket = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "simple-bucket");
    }

    #[test]
    fn test_s3_bucket_policy_serde_roundtrip() {
        let policy = S3BucketPolicy {
            bucket: "my-bucket".to_string(),
            policy: serde_json::json!({
                "Version": "2012-10-17",
                "Statement": [{
                    "Effect": "Allow",
                    "Principal": "*",
                    "Action": "s3:GetObject",
                    "Resource": "arn:aws:s3:::my-bucket/*"
                }]
            }),
        };

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: S3BucketPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bucket, "my-bucket");
    }

    #[test]
    fn test_s3_lifecycle_rule_serde_roundtrip() {
        let rule = S3LifecycleRule {
            bucket: "my-bucket".to_string(),
            id: "archive-rule".to_string(),
            status: "Enabled".to_string(),
            prefix: Some("logs/".to_string()),
            expiration_days: Some(365),
            transition: Some(vec![S3Transition {
                days: 30,
                storage_class: "GLACIER".to_string(),
            }]),
        };

        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: S3LifecycleRule = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "archive-rule");
        assert_eq!(deserialized.transition.unwrap()[0].storage_class, "GLACIER");
    }
}
