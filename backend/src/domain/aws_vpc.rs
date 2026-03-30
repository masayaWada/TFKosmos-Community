#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use super::aws_iam::Tag;

/// VPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vpc {
    pub vpc_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default)]
    pub enable_dns_support: bool,
    #[serde(default)]
    pub enable_dns_hostnames: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_tenancy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// サブネット
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subnet {
    pub subnet_id: String,
    pub vpc_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(default)]
    pub map_public_ip_on_launch: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// ルートテーブル
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTable {
    pub route_table_id: String,
    pub vpc_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routes: Option<Vec<Route>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associations: Option<Vec<RouteTableAssociation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// ルート
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nat_gateway_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_interface_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transit_gateway_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_peering_connection_id: Option<String>,
}

/// ルートテーブル関連付け
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTableAssociation {
    pub route_table_association_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet_id: Option<String>,
    #[serde(default)]
    pub main: bool,
}

/// セキュリティグループ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityGroup {
    pub group_id: String,
    pub group_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub vpc_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingress_rules: Option<Vec<SecurityGroupRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub egress_rules: Option<Vec<SecurityGroupRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// セキュリティグループルール
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityGroupRule {
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cidr_blocks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_security_group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// ネットワークACL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAcl {
    pub network_acl_id: String,
    pub vpc_id: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<NetworkAclEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// ネットワークACLエントリ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAclEntry {
    pub rule_number: i32,
    pub protocol: String,
    pub rule_action: String,
    pub egress: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vpc_serde_roundtrip() {
        let vpc = Vpc {
            vpc_id: "vpc-12345".to_string(),
            cidr_block: Some("10.0.0.0/16".to_string()),
            state: Some("available".to_string()),
            enable_dns_support: true,
            enable_dns_hostnames: true,
            instance_tenancy: Some("default".to_string()),
            tags: Some(vec![Tag {
                key: "Name".to_string(),
                value: "main-vpc".to_string(),
            }]),
        };

        let json = serde_json::to_string(&vpc).unwrap();
        let deserialized: Vpc = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.vpc_id, "vpc-12345");
        assert!(deserialized.enable_dns_support);
    }

    #[test]
    fn test_subnet_serde_roundtrip() {
        let subnet = Subnet {
            subnet_id: "subnet-12345".to_string(),
            vpc_id: "vpc-12345".to_string(),
            cidr_block: Some("10.0.1.0/24".to_string()),
            availability_zone: Some("ap-northeast-1a".to_string()),
            map_public_ip_on_launch: false,
            tags: None,
        };

        let json = serde_json::to_string(&subnet).unwrap();
        let deserialized: Subnet = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.subnet_id, "subnet-12345");
    }

    #[test]
    fn test_security_group_serde_roundtrip() {
        let sg = SecurityGroup {
            group_id: "sg-12345".to_string(),
            group_name: "web-sg".to_string(),
            description: Some("Web server security group".to_string()),
            vpc_id: "vpc-12345".to_string(),
            ingress_rules: Some(vec![SecurityGroupRule {
                protocol: "tcp".to_string(),
                from_port: Some(443),
                to_port: Some(443),
                cidr_blocks: Some(vec!["0.0.0.0/0".to_string()]),
                source_security_group_id: None,
                description: Some("HTTPS".to_string()),
            }]),
            egress_rules: None,
            tags: None,
        };

        let json = serde_json::to_string(&sg).unwrap();
        let deserialized: SecurityGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.group_name, "web-sg");
        assert_eq!(deserialized.ingress_rules.unwrap()[0].from_port, Some(443));
    }

    #[test]
    fn test_network_acl_serde_roundtrip() {
        let nacl = NetworkAcl {
            network_acl_id: "acl-12345".to_string(),
            vpc_id: "vpc-12345".to_string(),
            is_default: true,
            entries: Some(vec![NetworkAclEntry {
                rule_number: 100,
                protocol: "tcp".to_string(),
                rule_action: "allow".to_string(),
                egress: false,
                cidr_block: Some("0.0.0.0/0".to_string()),
                from_port: Some(80),
                to_port: Some(80),
            }]),
            associations: Some(vec!["subnet-12345".to_string()]),
            tags: None,
        };

        let json = serde_json::to_string(&nacl).unwrap();
        let deserialized: NetworkAcl = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.network_acl_id, "acl-12345");
        assert!(deserialized.is_default);
    }
}
