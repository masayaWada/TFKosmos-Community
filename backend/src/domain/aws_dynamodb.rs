#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use super::aws_iam::Tag;

/// DynamoDBテーブル
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBTable {
    pub table_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_capacity: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_capacity: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_schema: Option<Vec<DynamoDBKeySchemaElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute_definitions: Option<Vec<DynamoDBAttributeDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_secondary_indexes: Option<Vec<DynamoDBGSI>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_secondary_indexes: Option<Vec<DynamoDBLSI>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_attribute: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_view_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_in_time_recovery: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

/// キースキーマ要素
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBKeySchemaElement {
    pub attribute_name: String,
    pub key_type: String,
}

/// 属性定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBAttributeDefinition {
    pub attribute_name: String,
    pub attribute_type: String,
}

/// グローバルセカンダリインデックス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBGSI {
    pub index_name: String,
    pub key_schema: Vec<DynamoDBKeySchemaElement>,
    pub projection_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub non_key_attributes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_capacity: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_capacity: Option<i64>,
}

/// ローカルセカンダリインデックス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBLSI {
    pub index_name: String,
    pub key_schema: Vec<DynamoDBKeySchemaElement>,
    pub projection_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub non_key_attributes: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamodb_table_serde_roundtrip() {
        let table = DynamoDBTable {
            table_name: "my-table".to_string(),
            arn: Some("arn:aws:dynamodb:ap-northeast-1:123456789012:table/my-table".to_string()),
            status: Some("ACTIVE".to_string()),
            billing_mode: Some("PAY_PER_REQUEST".to_string()),
            read_capacity: None,
            write_capacity: None,
            key_schema: Some(vec![
                DynamoDBKeySchemaElement {
                    attribute_name: "pk".to_string(),
                    key_type: "HASH".to_string(),
                },
                DynamoDBKeySchemaElement {
                    attribute_name: "sk".to_string(),
                    key_type: "RANGE".to_string(),
                },
            ]),
            attribute_definitions: Some(vec![
                DynamoDBAttributeDefinition {
                    attribute_name: "pk".to_string(),
                    attribute_type: "S".to_string(),
                },
                DynamoDBAttributeDefinition {
                    attribute_name: "sk".to_string(),
                    attribute_type: "S".to_string(),
                },
            ]),
            global_secondary_indexes: Some(vec![DynamoDBGSI {
                index_name: "gsi-1".to_string(),
                key_schema: vec![DynamoDBKeySchemaElement {
                    attribute_name: "gsi1pk".to_string(),
                    key_type: "HASH".to_string(),
                }],
                projection_type: "ALL".to_string(),
                non_key_attributes: None,
                read_capacity: None,
                write_capacity: None,
            }]),
            local_secondary_indexes: None,
            ttl_attribute: Some("expires_at".to_string()),
            stream_enabled: Some(true),
            stream_view_type: Some("NEW_AND_OLD_IMAGES".to_string()),
            point_in_time_recovery: Some(true),
            tags: Some(vec![Tag {
                key: "Environment".to_string(),
                value: "Production".to_string(),
            }]),
        };

        let json = serde_json::to_string(&table).unwrap();
        let deserialized: DynamoDBTable = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.table_name, "my-table");
        assert_eq!(
            deserialized.billing_mode,
            Some("PAY_PER_REQUEST".to_string())
        );
    }

    #[test]
    fn test_dynamodb_table_minimal_serde() {
        let table = DynamoDBTable {
            table_name: "simple-table".to_string(),
            arn: None,
            status: None,
            billing_mode: None,
            read_capacity: None,
            write_capacity: None,
            key_schema: None,
            attribute_definitions: None,
            global_secondary_indexes: None,
            local_secondary_indexes: None,
            ttl_attribute: None,
            stream_enabled: None,
            stream_view_type: None,
            point_in_time_recovery: None,
            tags: None,
        };

        let json = serde_json::to_string(&table).unwrap();
        assert!(!json.contains("arn"));
        let deserialized: DynamoDBTable = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.table_name, "simple-table");
    }
}
