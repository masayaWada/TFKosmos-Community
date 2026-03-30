#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use super::aws_iam::Tag;

/// Lambda関数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaFunction {
    pub function_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_config: Option<LambdaVpcConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// Lambda VPC設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaVpcConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_group_ids: Option<Vec<String>>,
}

/// Lambdaレイヤー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaLayer {
    pub layer_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatible_runtimes: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lambda_function_serde_roundtrip() {
        let func = LambdaFunction {
            function_name: "my-function".to_string(),
            arn: Some(
                "arn:aws:lambda:ap-northeast-1:123456789012:function:my-function".to_string(),
            ),
            runtime: Some("python3.12".to_string()),
            handler: Some("index.handler".to_string()),
            role: Some("arn:aws:iam::123456789012:role/lambda-role".to_string()),
            description: Some("Test function".to_string()),
            memory_size: Some(256),
            timeout: Some(30),
            code_size: Some(1024),
            last_modified: Some("2024-01-01T00:00:00Z".to_string()),
            environment: Some(std::collections::HashMap::from([(
                "ENV".to_string(),
                "production".to_string(),
            )])),
            vpc_config: Some(LambdaVpcConfig {
                subnet_ids: Some(vec!["subnet-123".to_string()]),
                security_group_ids: Some(vec!["sg-123".to_string()]),
            }),
            layers: Some(vec![
                "arn:aws:lambda:ap-northeast-1:123456789012:layer:my-layer:1".to_string(),
            ]),
            tags: Some(vec![Tag {
                key: "Environment".to_string(),
                value: "Production".to_string(),
            }]),
        };

        let json = serde_json::to_string(&func).unwrap();
        let deserialized: LambdaFunction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.function_name, "my-function");
        assert_eq!(deserialized.runtime, Some("python3.12".to_string()));
        assert_eq!(deserialized.memory_size, Some(256));
    }

    #[test]
    fn test_lambda_function_minimal_serde() {
        let func = LambdaFunction {
            function_name: "simple-func".to_string(),
            arn: None,
            runtime: None,
            handler: None,
            role: None,
            description: None,
            memory_size: None,
            timeout: None,
            code_size: None,
            last_modified: None,
            environment: None,
            vpc_config: None,
            layers: None,
            tags: None,
        };

        let json = serde_json::to_string(&func).unwrap();
        assert!(!json.contains("arn"));
        assert!(!json.contains("runtime"));
        let deserialized: LambdaFunction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.function_name, "simple-func");
    }

    #[test]
    fn test_lambda_layer_serde_roundtrip() {
        let layer = LambdaLayer {
            layer_name: "my-layer".to_string(),
            layer_arn: Some(
                "arn:aws:lambda:ap-northeast-1:123456789012:layer:my-layer".to_string(),
            ),
            latest_version: Some(3),
            description: Some("Shared utilities".to_string()),
            compatible_runtimes: Some(vec!["python3.11".to_string(), "python3.12".to_string()]),
        };

        let json = serde_json::to_string(&layer).unwrap();
        let deserialized: LambdaLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.layer_name, "my-layer");
        assert_eq!(deserialized.compatible_runtimes.unwrap().len(), 2);
    }
}
