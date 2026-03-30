#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use super::aws_iam::Tag;

/// RDS DBインスタンス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsInstance {
    pub db_instance_identifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_instance_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allocated_storage: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_az: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publicly_accessible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_subnet_group_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_group_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_retention_period: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_encrypted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// RDS DBサブネットグループ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsSubnetGroup {
    pub db_subnet_group_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// RDS パラメータグループ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsParameterGroup {
    pub db_parameter_group_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<RdsParameter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// RDS パラメータ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsParameter {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_method: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rds_instance_serde_roundtrip() {
        let instance = RdsInstance {
            db_instance_identifier: "mydb".to_string(),
            db_instance_class: Some("db.t3.micro".to_string()),
            engine: Some("postgres".to_string()),
            engine_version: Some("15.4".to_string()),
            status: Some("available".to_string()),
            allocated_storage: Some(20),
            storage_type: Some("gp3".to_string()),
            multi_az: Some(false),
            publicly_accessible: Some(false),
            vpc_id: Some("vpc-12345".to_string()),
            db_subnet_group_name: Some("my-subnet-group".to_string()),
            availability_zone: Some("ap-northeast-1a".to_string()),
            endpoint: Some("mydb.123456789012.ap-northeast-1.rds.amazonaws.com".to_string()),
            port: Some(5432),
            master_username: Some("admin".to_string()),
            security_groups: Some(vec!["sg-12345".to_string()]),
            parameter_group_name: Some("default.postgres15".to_string()),
            backup_retention_period: Some(7),
            storage_encrypted: Some(true),
            tags: None,
        };

        let json = serde_json::to_string(&instance).unwrap();
        let deserialized: RdsInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.db_instance_identifier, "mydb");
        assert_eq!(deserialized.engine, Some("postgres".to_string()));
    }

    #[test]
    fn test_rds_subnet_group_serde_roundtrip() {
        let group = RdsSubnetGroup {
            db_subnet_group_name: "my-subnet-group".to_string(),
            description: Some("My DB subnet group".to_string()),
            vpc_id: Some("vpc-12345".to_string()),
            subnet_ids: Some(vec!["subnet-1".to_string(), "subnet-2".to_string()]),
            status: Some("Complete".to_string()),
            tags: None,
        };

        let json = serde_json::to_string(&group).unwrap();
        let deserialized: RdsSubnetGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.db_subnet_group_name, "my-subnet-group");
        assert_eq!(deserialized.subnet_ids.unwrap().len(), 2);
    }

    #[test]
    fn test_rds_parameter_group_serde_roundtrip() {
        let pg = RdsParameterGroup {
            db_parameter_group_name: "custom-pg15".to_string(),
            family: Some("postgres15".to_string()),
            description: Some("Custom parameter group".to_string()),
            parameters: Some(vec![RdsParameter {
                name: "max_connections".to_string(),
                value: "200".to_string(),
                apply_method: Some("pending-reboot".to_string()),
            }]),
            tags: None,
        };

        let json = serde_json::to_string(&pg).unwrap();
        let deserialized: RdsParameterGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.db_parameter_group_name, "custom-pg15");
    }
}
