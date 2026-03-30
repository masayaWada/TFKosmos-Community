use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use anyhow::{Context, Result};

/// Terraform state ファイル (v4) のトップレベル構造
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TfState {
    pub version: u32,
    #[serde(default)]
    pub terraform_version: String,
    #[serde(default)]
    pub serial: u64,
    #[serde(default)]
    pub lineage: String,
    #[serde(default)]
    pub resources: Vec<TfStateResource>,
}

/// Terraform state 内の個別リソース
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TfStateResource {
    pub mode: String, // "managed" or "data"
    #[serde(rename = "type")]
    pub resource_type: String,
    pub name: String,
    pub provider: String,
    #[serde(default)]
    pub instances: Vec<TfStateInstance>,
}

/// リソースの個別インスタンス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TfStateInstance {
    #[serde(default)]
    pub attributes: Value,
    #[serde(default)]
    pub index_key: Option<Value>,
}

impl TfState {
    /// JSON 文字列から Terraform state をパースする（v4 のみサポート）
    pub fn parse(json_str: &str) -> Result<Self> {
        let state: TfState =
            serde_json::from_str(json_str).context("Terraform stateファイルのパースに失敗")?;
        if state.version != 4 {
            anyhow::bail!(
                "サポートされていないstateバージョン: {} (v4のみサポート)",
                state.version
            );
        }
        Ok(state)
    }

    /// managed リソースをタイプ別にグループ化して返す
    pub fn resources_by_type(&self) -> HashMap<String, Vec<&TfStateResource>> {
        let mut map: HashMap<String, Vec<&TfStateResource>> = HashMap::new();
        for res in &self.resources {
            if res.mode == "managed" {
                map.entry(res.resource_type.clone()).or_default().push(res);
            }
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_v4_state() -> String {
        serde_json::json!({
            "version": 4,
            "terraform_version": "1.5.0",
            "serial": 10,
            "lineage": "abc-def",
            "resources": [
                {
                    "mode": "managed",
                    "type": "aws_s3_bucket",
                    "name": "my_bucket",
                    "provider": "provider[\"registry.terraform.io/hashicorp/aws\"]",
                    "instances": [
                        {
                            "attributes": {
                                "bucket": "my-bucket-name",
                                "acl": "private"
                            }
                        }
                    ]
                },
                {
                    "mode": "managed",
                    "type": "aws_instance",
                    "name": "web",
                    "provider": "provider[\"registry.terraform.io/hashicorp/aws\"]",
                    "instances": [
                        {
                            "attributes": {
                                "id": "i-1234567890",
                                "instance_type": "t3.micro"
                            }
                        }
                    ]
                }
            ]
        })
        .to_string()
    }

    #[test]
    fn parse_valid_v4_state() {
        let state = TfState::parse(&valid_v4_state()).unwrap();
        assert_eq!(state.version, 4);
        assert_eq!(state.terraform_version, "1.5.0");
        assert_eq!(state.serial, 10);
        assert_eq!(state.resources.len(), 2);
        assert_eq!(state.resources[0].resource_type, "aws_s3_bucket");
        assert_eq!(state.resources[1].resource_type, "aws_instance");
    }

    #[test]
    fn reject_non_v4_version() {
        let json = serde_json::json!({
            "version": 3,
            "resources": []
        })
        .to_string();

        let result = TfState::parse(&json);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("サポートされていないstateバージョン"),
            "Expected version error, got: {}",
            err_msg
        );
    }

    #[test]
    fn parse_empty_resources() {
        let json = serde_json::json!({
            "version": 4,
            "resources": []
        })
        .to_string();

        let state = TfState::parse(&json).unwrap();
        assert_eq!(state.version, 4);
        assert!(state.resources.is_empty());
    }

    #[test]
    fn parse_state_with_multiple_resources() {
        let state = TfState::parse(&valid_v4_state()).unwrap();
        assert_eq!(state.resources.len(), 2);

        let bucket = &state.resources[0];
        assert_eq!(bucket.mode, "managed");
        assert_eq!(bucket.resource_type, "aws_s3_bucket");
        assert_eq!(bucket.name, "my_bucket");
        assert_eq!(bucket.instances.len(), 1);
        assert_eq!(bucket.instances[0].attributes["bucket"], "my-bucket-name");
    }

    #[test]
    fn resources_by_type_groups_correctly() {
        let json = serde_json::json!({
            "version": 4,
            "resources": [
                {
                    "mode": "managed",
                    "type": "aws_s3_bucket",
                    "name": "a",
                    "provider": "aws",
                    "instances": []
                },
                {
                    "mode": "managed",
                    "type": "aws_s3_bucket",
                    "name": "b",
                    "provider": "aws",
                    "instances": []
                },
                {
                    "mode": "managed",
                    "type": "aws_instance",
                    "name": "web",
                    "provider": "aws",
                    "instances": []
                },
                {
                    "mode": "data",
                    "type": "aws_ami",
                    "name": "latest",
                    "provider": "aws",
                    "instances": []
                }
            ]
        })
        .to_string();

        let state = TfState::parse(&json).unwrap();
        let by_type = state.resources_by_type();

        // "data" mode should be excluded
        assert_eq!(by_type.len(), 2);
        assert_eq!(by_type["aws_s3_bucket"].len(), 2);
        assert_eq!(by_type["aws_instance"].len(), 1);
        assert!(!by_type.contains_key("aws_ami"));
    }

    #[test]
    fn parse_invalid_json() {
        let result = TfState::parse("not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("パースに失敗"));
    }
}
