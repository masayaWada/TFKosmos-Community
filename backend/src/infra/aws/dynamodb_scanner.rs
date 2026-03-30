//! AWS DynamoDBスキャナー

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::infra::aws::dynamodb_client_trait::DynamoDBClientOps;
use crate::models::ScanConfig;

/// AWS DynamoDBスキャナー
pub struct AwsDynamoDBScanner<C: DynamoDBClientOps> {
    config: ScanConfig,
    dynamodb_client: Arc<C>,
}

impl<C: DynamoDBClientOps> AwsDynamoDBScanner<C> {
    /// 本番用・テスト用共通：クライアントを指定してスキャナーを作成
    pub fn new(config: ScanConfig, client: Arc<C>) -> Self {
        Self {
            config,
            dynamodb_client: client,
        }
    }

    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: C) -> Self {
        Self {
            config,
            dynamodb_client: Arc::new(client),
        }
    }

    /// DynamoDBリソースをスキャンし結果をresultsに追加
    pub async fn scan_into(
        &self,
        results: &mut serde_json::Map<String, Value>,
        progress_callback: &(dyn Fn(u32, String) + Send + Sync),
        completed_targets: &AtomicUsize,
        total_targets: usize,
    ) -> Result<()> {
        let scan_targets = &self.config.scan_targets;

        if scan_targets
            .get("dynamodb_tables")
            .copied()
            .unwrap_or(false)
        {
            debug!("DynamoDB Tablesのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "DynamoDB Tablesのスキャン中...".to_string(),
            );
            let tables = self.scan_tables().await?;
            let count = tables.len();
            results.insert("dynamodb_tables".to_string(), Value::Array(tables));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            debug!(count, "DynamoDB Tablesのスキャン完了");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("DynamoDB Tablesのスキャン完了: {}件", count),
            );
        } else {
            results.insert("dynamodb_tables".to_string(), Value::Array(Vec::new()));
        }

        Ok(())
    }

    fn apply_name_prefix_filter(&self, name: &str) -> bool {
        if let Some(prefix) = self.config.filters.get("name_prefix") {
            name.starts_with(prefix)
        } else {
            true
        }
    }

    async fn scan_tables(&self) -> Result<Vec<Value>> {
        info!("DynamoDB Tables一覧を取得中");
        let table_names = self.dynamodb_client.list_table_names().await?;
        let mut tables = Vec::new();

        for table_name in &table_names {
            if !self.apply_name_prefix_filter(table_name) {
                continue;
            }

            match self.dynamodb_client.describe_table(table_name).await {
                Ok(table_info) => {
                    let mut table_json = json!({
                        "table_name": table_info.table_name,
                    });

                    if let Some(arn) = &table_info.table_arn {
                        table_json["arn"] = json!(arn);
                    }
                    if let Some(status) = &table_info.status {
                        table_json["status"] = json!(status);
                    }
                    if let Some(billing) = &table_info.billing_mode {
                        table_json["billing_mode"] = json!(billing);
                    }
                    if let Some(rc) = table_info.read_capacity {
                        table_json["read_capacity"] = json!(rc);
                    }
                    if let Some(wc) = table_info.write_capacity {
                        table_json["write_capacity"] = json!(wc);
                    }

                    if !table_info.key_schema.is_empty() {
                        let ks: Vec<Value> = table_info
                            .key_schema
                            .iter()
                            .map(|k| json!({"attribute_name": k.attribute_name, "key_type": k.key_type}))
                            .collect();
                        table_json["key_schema"] = json!(ks);
                    }

                    if !table_info.attribute_definitions.is_empty() {
                        let ad: Vec<Value> = table_info
                            .attribute_definitions
                            .iter()
                            .map(|a| json!({"attribute_name": a.attribute_name, "attribute_type": a.attribute_type}))
                            .collect();
                        table_json["attribute_definitions"] = json!(ad);
                    }

                    if !table_info.global_secondary_indexes.is_empty() {
                        let gsis: Vec<Value> = table_info
                            .global_secondary_indexes
                            .iter()
                            .map(|gsi| {
                                let ks: Vec<Value> = gsi
                                    .key_schema
                                    .iter()
                                    .map(|k| json!({"attribute_name": k.attribute_name, "key_type": k.key_type}))
                                    .collect();
                                let mut g = json!({
                                    "index_name": gsi.index_name,
                                    "key_schema": ks,
                                    "projection_type": gsi.projection_type,
                                });
                                if !gsi.non_key_attributes.is_empty() {
                                    g["non_key_attributes"] = json!(gsi.non_key_attributes);
                                }
                                if let Some(rc) = gsi.read_capacity {
                                    g["read_capacity"] = json!(rc);
                                }
                                if let Some(wc) = gsi.write_capacity {
                                    g["write_capacity"] = json!(wc);
                                }
                                g
                            })
                            .collect();
                        table_json["global_secondary_indexes"] = json!(gsis);
                    }

                    if !table_info.local_secondary_indexes.is_empty() {
                        let lsis: Vec<Value> = table_info
                            .local_secondary_indexes
                            .iter()
                            .map(|lsi| {
                                let ks: Vec<Value> = lsi
                                    .key_schema
                                    .iter()
                                    .map(|k| json!({"attribute_name": k.attribute_name, "key_type": k.key_type}))
                                    .collect();
                                let mut l = json!({
                                    "index_name": lsi.index_name,
                                    "key_schema": ks,
                                    "projection_type": lsi.projection_type,
                                });
                                if !lsi.non_key_attributes.is_empty() {
                                    l["non_key_attributes"] = json!(lsi.non_key_attributes);
                                }
                                l
                            })
                            .collect();
                        table_json["local_secondary_indexes"] = json!(lsis);
                    }

                    if table_info.stream_enabled {
                        table_json["stream_enabled"] = json!(true);
                        if let Some(svt) = &table_info.stream_view_type {
                            table_json["stream_view_type"] = json!(svt);
                        }
                    }

                    // TTL取得
                    if let Ok(ttl) = self.dynamodb_client.describe_ttl(table_name).await {
                        if ttl.enabled {
                            if let Some(attr) = &ttl.attribute_name {
                                table_json["ttl_attribute"] = json!(attr);
                            }
                        }
                    }

                    if !table_info.tags.is_empty() {
                        let tags: Vec<Value> = table_info
                            .tags
                            .iter()
                            .map(|(k, v)| json!({"key": k, "value": v}))
                            .collect();
                        table_json["tags"] = json!(tags);
                    }

                    tables.push(table_json);
                }
                Err(e) => {
                    warn!("テーブル {} の詳細取得に失敗: {}", table_name, e);
                }
            }
        }

        info!(count = tables.len(), "DynamoDB Tables一覧取得完了");
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::dynamodb_client_trait::mock::MockDynamoDBClient;
    use crate::infra::aws::dynamodb_client_trait::{
        AttributeDefinitionInfo, DynamoDBTableInfo, GSIInfo, KeySchemaElementInfo, TTLInfo,
    };
    use std::collections::HashMap;

    fn make_test_config(targets: HashMap<String, bool>) -> ScanConfig {
        ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: Some("test".to_string()),
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets: targets,
            filters: HashMap::new(),
            include_tags: true,
            assume_role_arn: None,
            assume_role_session_name: None,
        }
    }

    #[tokio::test]
    async fn test_scan_dynamodb_tables() {
        let mut mock = MockDynamoDBClient::new();
        mock.expect_list_table_names()
            .returning(|| Ok(vec!["my-table".to_string()]));
        mock.expect_describe_table()
            .withf(|name| name == "my-table")
            .returning(|_| {
                Ok(DynamoDBTableInfo {
                    table_name: "my-table".to_string(),
                    table_arn: Some(
                        "arn:aws:dynamodb:ap-northeast-1:123:table/my-table".to_string(),
                    ),
                    status: Some("ACTIVE".to_string()),
                    billing_mode: Some("PAY_PER_REQUEST".to_string()),
                    read_capacity: None,
                    write_capacity: None,
                    key_schema: vec![KeySchemaElementInfo {
                        attribute_name: "pk".to_string(),
                        key_type: "HASH".to_string(),
                    }],
                    attribute_definitions: vec![AttributeDefinitionInfo {
                        attribute_name: "pk".to_string(),
                        attribute_type: "S".to_string(),
                    }],
                    global_secondary_indexes: vec![GSIInfo {
                        index_name: "gsi-1".to_string(),
                        key_schema: vec![KeySchemaElementInfo {
                            attribute_name: "gsi1pk".to_string(),
                            key_type: "HASH".to_string(),
                        }],
                        projection_type: "ALL".to_string(),
                        non_key_attributes: vec![],
                        read_capacity: None,
                        write_capacity: None,
                    }],
                    local_secondary_indexes: vec![],
                    stream_enabled: true,
                    stream_view_type: Some("NEW_AND_OLD_IMAGES".to_string()),
                    tags: HashMap::from([("team".to_string(), "backend".to_string())]),
                })
            });
        mock.expect_describe_ttl()
            .withf(|name| name == "my-table")
            .returning(|_| {
                Ok(TTLInfo {
                    attribute_name: Some("expires_at".to_string()),
                    enabled: true,
                })
            });

        let config = make_test_config(HashMap::from([("dynamodb_tables".to_string(), true)]));
        let scanner = AwsDynamoDBScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let tables = results.get("dynamodb_tables").unwrap().as_array().unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0]["table_name"], "my-table");
        assert_eq!(tables[0]["billing_mode"], "PAY_PER_REQUEST");
        assert_eq!(tables[0]["stream_enabled"], true);
        assert_eq!(tables[0]["ttl_attribute"], "expires_at");
        assert!(tables[0]["global_secondary_indexes"].is_array());
    }

    #[tokio::test]
    async fn test_scan_dynamodb_tables_empty() {
        let mut mock = MockDynamoDBClient::new();
        mock.expect_list_table_names().never();
        mock.expect_describe_table().never();
        mock.expect_describe_ttl().never();

        let config = make_test_config(HashMap::from([("dynamodb_tables".to_string(), false)]));
        let scanner = AwsDynamoDBScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        assert!(results
            .get("dynamodb_tables")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn test_scan_table_without_optional_fields() {
        let mut mock = MockDynamoDBClient::new();
        mock.expect_list_table_names()
            .returning(|| Ok(vec!["minimal-table".to_string()]));
        mock.expect_describe_table()
            .withf(|name| name == "minimal-table")
            .returning(|_| {
                Ok(DynamoDBTableInfo {
                    table_name: "minimal-table".to_string(),
                    table_arn: None,
                    status: None,
                    billing_mode: None,
                    read_capacity: None,
                    write_capacity: None,
                    key_schema: vec![KeySchemaElementInfo {
                        attribute_name: "id".to_string(),
                        key_type: "HASH".to_string(),
                    }],
                    attribute_definitions: vec![AttributeDefinitionInfo {
                        attribute_name: "id".to_string(),
                        attribute_type: "S".to_string(),
                    }],
                    global_secondary_indexes: vec![],
                    local_secondary_indexes: vec![],
                    stream_enabled: false,
                    stream_view_type: None,
                    tags: HashMap::new(),
                })
            });
        mock.expect_describe_ttl()
            .withf(|name| name == "minimal-table")
            .returning(|_| {
                Ok(TTLInfo {
                    attribute_name: None,
                    enabled: false,
                })
            });

        let config = make_test_config(HashMap::from([("dynamodb_tables".to_string(), true)]));
        let scanner = AwsDynamoDBScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let tables = results.get("dynamodb_tables").unwrap().as_array().unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0]["table_name"], "minimal-table");

        // オプショナルフィールドが存在しないことを確認
        assert!(tables[0].get("arn").is_none());
        assert!(tables[0].get("status").is_none());
        assert!(tables[0].get("billing_mode").is_none());
        assert!(tables[0].get("stream_enabled").is_none());
        assert!(tables[0].get("ttl_attribute").is_none());
        assert!(tables[0].get("global_secondary_indexes").is_none());
        assert!(tables[0].get("tags").is_none());
    }

    #[tokio::test]
    async fn test_scan_describe_table_error() {
        let mut mock = MockDynamoDBClient::new();
        mock.expect_list_table_names()
            .returning(|| Ok(vec!["ok-table".to_string(), "error-table".to_string()]));
        mock.expect_describe_table()
            .withf(|name| name == "ok-table")
            .returning(|_| {
                Ok(DynamoDBTableInfo {
                    table_name: "ok-table".to_string(),
                    table_arn: Some("arn:aws:dynamodb:::table/ok-table".to_string()),
                    status: Some("ACTIVE".to_string()),
                    billing_mode: Some("PAY_PER_REQUEST".to_string()),
                    read_capacity: None,
                    write_capacity: None,
                    key_schema: vec![KeySchemaElementInfo {
                        attribute_name: "pk".to_string(),
                        key_type: "HASH".to_string(),
                    }],
                    attribute_definitions: vec![AttributeDefinitionInfo {
                        attribute_name: "pk".to_string(),
                        attribute_type: "S".to_string(),
                    }],
                    global_secondary_indexes: vec![],
                    local_secondary_indexes: vec![],
                    stream_enabled: false,
                    stream_view_type: None,
                    tags: HashMap::new(),
                })
            });
        mock.expect_describe_table()
            .withf(|name| name == "error-table")
            .returning(|_| Err(anyhow::anyhow!("Table not found")));
        mock.expect_describe_ttl()
            .withf(|name| name == "ok-table")
            .returning(|_| {
                Ok(TTLInfo {
                    attribute_name: None,
                    enabled: false,
                })
            });

        let config = make_test_config(HashMap::from([("dynamodb_tables".to_string(), true)]));
        let scanner = AwsDynamoDBScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        // エラーが発生してもスキャン全体は成功する（warn ログを出してスキップ）
        let scan_result = scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await;
        assert!(
            scan_result.is_ok(),
            "Scan should succeed even if one table fails"
        );

        let tables = results.get("dynamodb_tables").unwrap().as_array().unwrap();
        // エラーになったテーブルはスキップされ、成功したものだけ含まれる
        assert_eq!(
            tables.len(),
            1,
            "Only the successful table should be in results"
        );
        assert_eq!(tables[0]["table_name"], "ok-table");
    }
}
