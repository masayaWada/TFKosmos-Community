#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use super::aws_iam::Tag;

/// EC2インスタンス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ec2Instance {
    pub instance_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ami_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iam_instance_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ec2_instance_serde_roundtrip() {
        let instance = Ec2Instance {
            instance_id: "i-0123456789abcdef0".to_string(),
            instance_type: Some("t3.micro".to_string()),
            state: Some("running".to_string()),
            ami_id: Some("ami-0123456789abcdef0".to_string()),
            vpc_id: Some("vpc-12345".to_string()),
            subnet_id: Some("subnet-12345".to_string()),
            private_ip: Some("10.0.1.100".to_string()),
            public_ip: None,
            key_name: Some("my-key".to_string()),
            iam_instance_profile: Some("my-profile".to_string()),
            security_groups: Some(vec!["sg-12345".to_string()]),
            tags: Some(vec![Tag {
                key: "Name".to_string(),
                value: "web-server".to_string(),
            }]),
        };

        let json = serde_json::to_string(&instance).unwrap();
        let deserialized: Ec2Instance = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.instance_id, "i-0123456789abcdef0");
        assert_eq!(deserialized.instance_type, Some("t3.micro".to_string()));
    }

    #[test]
    fn test_ec2_instance_minimal() {
        let instance = Ec2Instance {
            instance_id: "i-minimal".to_string(),
            instance_type: None,
            state: None,
            ami_id: None,
            vpc_id: None,
            subnet_id: None,
            private_ip: None,
            public_ip: None,
            key_name: None,
            iam_instance_profile: None,
            security_groups: None,
            tags: None,
        };

        let json = serde_json::to_string(&instance).unwrap();
        assert!(!json.contains("vpc_id"));
        let deserialized: Ec2Instance = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.instance_id, "i-minimal");
    }
}
