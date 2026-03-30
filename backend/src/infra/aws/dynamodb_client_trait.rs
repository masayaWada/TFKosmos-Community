//! DynamoDBクライアント操作の抽象化トレイト

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// DynamoDBテーブル情報
#[derive(Debug, Clone)]
pub struct DynamoDBTableInfo {
    pub table_name: String,
    pub table_arn: Option<String>,
    pub status: Option<String>,
    pub billing_mode: Option<String>,
    pub read_capacity: Option<i64>,
    pub write_capacity: Option<i64>,
    pub key_schema: Vec<KeySchemaElementInfo>,
    pub attribute_definitions: Vec<AttributeDefinitionInfo>,
    pub global_secondary_indexes: Vec<GSIInfo>,
    pub local_secondary_indexes: Vec<LSIInfo>,
    pub stream_enabled: bool,
    pub stream_view_type: Option<String>,
    pub tags: HashMap<String, String>,
}

/// キースキーマ要素
#[derive(Debug, Clone)]
pub struct KeySchemaElementInfo {
    pub attribute_name: String,
    pub key_type: String,
}

/// 属性定義
#[derive(Debug, Clone)]
pub struct AttributeDefinitionInfo {
    pub attribute_name: String,
    pub attribute_type: String,
}

/// GSI情報
#[derive(Debug, Clone)]
pub struct GSIInfo {
    pub index_name: String,
    pub key_schema: Vec<KeySchemaElementInfo>,
    pub projection_type: String,
    pub non_key_attributes: Vec<String>,
    pub read_capacity: Option<i64>,
    pub write_capacity: Option<i64>,
}

/// LSI情報
#[derive(Debug, Clone)]
pub struct LSIInfo {
    pub index_name: String,
    pub key_schema: Vec<KeySchemaElementInfo>,
    pub projection_type: String,
    pub non_key_attributes: Vec<String>,
}

/// TTL情報
#[derive(Debug, Clone)]
pub struct TTLInfo {
    pub attribute_name: Option<String>,
    pub enabled: bool,
}

/// DynamoDBクライアント操作を抽象化するトレイト
#[async_trait]
pub trait DynamoDBClientOps: Send + Sync {
    /// テーブル名一覧を取得
    async fn list_table_names(&self) -> Result<Vec<String>>;

    /// テーブルの詳細情報を取得
    async fn describe_table(&self, table_name: &str) -> Result<DynamoDBTableInfo>;

    /// TTL設定を取得
    async fn describe_ttl(&self, table_name: &str) -> Result<TTLInfo>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub DynamoDBClient {}

        #[async_trait]
        impl DynamoDBClientOps for DynamoDBClient {
            async fn list_table_names(&self) -> Result<Vec<String>>;
            async fn describe_table(&self, table_name: &str) -> Result<DynamoDBTableInfo>;
            async fn describe_ttl(&self, table_name: &str) -> Result<TTLInfo>;
        }
    }
}
