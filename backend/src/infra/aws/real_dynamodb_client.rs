//! AWS SDK DynamoDBクライアントの本番実装

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_dynamodb::Client as DynamoDBClient;
use std::collections::HashMap;

use super::dynamodb_client_trait::{
    AttributeDefinitionInfo, DynamoDBClientOps, DynamoDBTableInfo, GSIInfo, KeySchemaElementInfo,
    LSIInfo, TTLInfo,
};

/// AWS SDK DynamoDBクライアントをラップした本番実装
pub struct RealDynamoDBClient {
    client: DynamoDBClient,
}

impl RealDynamoDBClient {
    pub fn new(client: DynamoDBClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl DynamoDBClientOps for RealDynamoDBClient {
    async fn list_table_names(&self) -> Result<Vec<String>> {
        let mut table_names = Vec::new();
        let mut last_evaluated: Option<String> = None;

        loop {
            let mut req = self.client.list_tables();
            if let Some(ref token) = last_evaluated {
                req = req.exclusive_start_table_name(token.clone());
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to list DynamoDB tables: {}", e))?;

            table_names.extend(output.table_names().iter().cloned());

            last_evaluated = output.last_evaluated_table_name().map(|s| s.to_string());
            if last_evaluated.is_none() {
                break;
            }
        }

        Ok(table_names)
    }

    async fn describe_table(&self, table_name: &str) -> Result<DynamoDBTableInfo> {
        let output = self
            .client
            .describe_table()
            .table_name(table_name)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe table {}: {}", table_name, e))?;

        let table = output
            .table()
            .ok_or_else(|| anyhow!("Table description not found for {}", table_name))?;

        let key_schema: Vec<KeySchemaElementInfo> = table
            .key_schema()
            .iter()
            .map(|ks| KeySchemaElementInfo {
                attribute_name: ks.attribute_name().to_string(),
                key_type: ks.key_type().as_str().to_string(),
            })
            .collect();

        let attribute_definitions: Vec<AttributeDefinitionInfo> = table
            .attribute_definitions()
            .iter()
            .map(|ad| AttributeDefinitionInfo {
                attribute_name: ad.attribute_name().to_string(),
                attribute_type: ad.attribute_type().as_str().to_string(),
            })
            .collect();

        let billing_mode = table
            .billing_mode_summary()
            .and_then(|b| b.billing_mode())
            .map(|m| m.as_str().to_string());

        let (read_capacity, write_capacity) = table
            .provisioned_throughput()
            .map(|pt| (pt.read_capacity_units(), pt.write_capacity_units()))
            .unwrap_or((None, None));

        let global_secondary_indexes: Vec<GSIInfo> = table
            .global_secondary_indexes()
            .iter()
            .map(|gsi| {
                let gsi_key_schema: Vec<KeySchemaElementInfo> = gsi
                    .key_schema()
                    .iter()
                    .map(|ks| KeySchemaElementInfo {
                        attribute_name: ks.attribute_name().to_string(),
                        key_type: ks.key_type().as_str().to_string(),
                    })
                    .collect();

                let projection = gsi.projection();
                let projection_type = projection
                    .map(|p| {
                        p.projection_type()
                            .map(|t| t.as_str().to_string())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let non_key_attributes = projection
                    .map(|p| {
                        p.non_key_attributes()
                            .iter()
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();

                let (gsi_read, gsi_write) = gsi
                    .provisioned_throughput()
                    .map(|pt| (pt.read_capacity_units(), pt.write_capacity_units()))
                    .unwrap_or((None, None));

                GSIInfo {
                    index_name: gsi.index_name().unwrap_or_default().to_string(),
                    key_schema: gsi_key_schema,
                    projection_type,
                    non_key_attributes,
                    read_capacity: gsi_read,
                    write_capacity: gsi_write,
                }
            })
            .collect();

        let local_secondary_indexes: Vec<LSIInfo> = table
            .local_secondary_indexes()
            .iter()
            .map(|lsi| {
                let lsi_key_schema: Vec<KeySchemaElementInfo> = lsi
                    .key_schema()
                    .iter()
                    .map(|ks| KeySchemaElementInfo {
                        attribute_name: ks.attribute_name().to_string(),
                        key_type: ks.key_type().as_str().to_string(),
                    })
                    .collect();

                let projection = lsi.projection();
                let projection_type = projection
                    .map(|p| {
                        p.projection_type()
                            .map(|t| t.as_str().to_string())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let non_key_attributes = projection
                    .map(|p| {
                        p.non_key_attributes()
                            .iter()
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();

                LSIInfo {
                    index_name: lsi.index_name().unwrap_or_default().to_string(),
                    key_schema: lsi_key_schema,
                    projection_type,
                    non_key_attributes,
                }
            })
            .collect();

        let stream_enabled = table
            .stream_specification()
            .map(|s| s.stream_enabled())
            .unwrap_or(false);
        let stream_view_type = table
            .stream_specification()
            .and_then(|s| s.stream_view_type())
            .map(|t| t.as_str().to_string());

        // タグ取得
        let tags = if let Some(arn) = table.table_arn() {
            match self
                .client
                .list_tags_of_resource()
                .resource_arn(arn)
                .send()
                .await
            {
                Ok(t) => t
                    .tags()
                    .iter()
                    .map(|tag| (tag.key().to_string(), tag.value().to_string()))
                    .collect(),
                Err(_) => HashMap::new(),
            }
        } else {
            HashMap::new()
        };

        Ok(DynamoDBTableInfo {
            table_name: table.table_name().unwrap_or_default().to_string(),
            table_arn: table.table_arn().map(|s| s.to_string()),
            status: table.table_status().map(|s| s.as_str().to_string()),
            billing_mode,
            read_capacity,
            write_capacity,
            key_schema,
            attribute_definitions,
            global_secondary_indexes,
            local_secondary_indexes,
            stream_enabled,
            stream_view_type,
            tags,
        })
    }

    async fn describe_ttl(&self, table_name: &str) -> Result<TTLInfo> {
        match self
            .client
            .describe_time_to_live()
            .table_name(table_name)
            .send()
            .await
        {
            Ok(output) => {
                let desc = output.time_to_live_description();
                let enabled = desc
                    .and_then(|d| d.time_to_live_status())
                    .map(|s| s.as_str() == "ENABLED")
                    .unwrap_or(false);
                let attribute_name = desc.and_then(|d| d.attribute_name()).map(|s| s.to_string());
                Ok(TTLInfo {
                    attribute_name,
                    enabled,
                })
            }
            Err(_) => Ok(TTLInfo {
                attribute_name: None,
                enabled: false,
            }),
        }
    }
}
