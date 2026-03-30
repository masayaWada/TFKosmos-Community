//! AWS SDK RDSクライアントの本番実装

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_rds::Client as RdsClient;
use std::collections::HashMap;

use super::rds_client_trait::{
    RdsClientOps, RdsInstanceInfo, RdsParameterGroupInfo, RdsParameterInfo, RdsSubnetGroupInfo,
};

/// AWS SDK RDSクライアントをラップした本番実装
pub struct RealRdsClient {
    client: RdsClient,
}

impl RealRdsClient {
    pub fn new(client: RdsClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl RdsClientOps for RealRdsClient {
    async fn describe_db_instances(&self) -> Result<Vec<RdsInstanceInfo>> {
        let mut instances = Vec::new();
        let mut paginator = self.client.describe_db_instances().into_paginator().send();

        while let Some(page) = paginator.next().await {
            let page = page.map_err(|e| anyhow!("Failed to describe DB instances: {}", e))?;

            for db in page.db_instances() {
                // タグを取得
                let tags: HashMap<String, String> = db
                    .tag_list()
                    .iter()
                    .filter_map(|t| {
                        let key = t.key()?.to_string();
                        let value = t.value()?.to_string();
                        Some((key, value))
                    })
                    .collect();

                instances.push(RdsInstanceInfo {
                    db_instance_identifier: db
                        .db_instance_identifier()
                        .unwrap_or_default()
                        .to_string(),
                    db_instance_class: db.db_instance_class().map(|s| s.to_string()),
                    engine: db.engine().map(|s| s.to_string()),
                    engine_version: db.engine_version().map(|s| s.to_string()),
                    status: db.db_instance_status().map(|s| s.to_string()),
                    allocated_storage: db.allocated_storage(),
                    storage_type: db.storage_type().map(|s| s.to_string()),
                    multi_az: db.multi_az().unwrap_or(false),
                    publicly_accessible: db.publicly_accessible().unwrap_or(false),
                    vpc_id: db
                        .db_subnet_group()
                        .and_then(|g| g.vpc_id())
                        .map(|s| s.to_string()),
                    db_subnet_group_name: db
                        .db_subnet_group()
                        .and_then(|g| g.db_subnet_group_name())
                        .map(|s| s.to_string()),
                    availability_zone: db.availability_zone().map(|s| s.to_string()),
                    endpoint: db
                        .endpoint()
                        .and_then(|e| e.address())
                        .map(|s| s.to_string()),
                    port: db.endpoint().and_then(|e| e.port()),
                    master_username: db.master_username().map(|s| s.to_string()),
                    security_groups: db
                        .vpc_security_groups()
                        .iter()
                        .filter_map(|sg| sg.vpc_security_group_id().map(|s| s.to_string()))
                        .collect(),
                    parameter_group_name: db
                        .db_parameter_groups()
                        .first()
                        .and_then(|pg| pg.db_parameter_group_name())
                        .map(|s| s.to_string()),
                    backup_retention_period: db.backup_retention_period(),
                    storage_encrypted: db.storage_encrypted().unwrap_or(false),
                    tags,
                });
            }
        }

        Ok(instances)
    }

    async fn describe_db_subnet_groups(&self) -> Result<Vec<RdsSubnetGroupInfo>> {
        let mut groups = Vec::new();
        let mut paginator = self
            .client
            .describe_db_subnet_groups()
            .into_paginator()
            .send();

        while let Some(page) = paginator.next().await {
            let page = page.map_err(|e| anyhow!("Failed to describe DB subnet groups: {}", e))?;

            for group in page.db_subnet_groups() {
                let subnet_ids = group
                    .subnets()
                    .iter()
                    .filter_map(|s| s.subnet_identifier().map(|id| id.to_string()))
                    .collect();

                groups.push(RdsSubnetGroupInfo {
                    db_subnet_group_name: group
                        .db_subnet_group_name()
                        .unwrap_or_default()
                        .to_string(),
                    description: group.db_subnet_group_description().map(|s| s.to_string()),
                    vpc_id: group.vpc_id().map(|s| s.to_string()),
                    subnet_ids,
                    status: group.subnet_group_status().map(|s| s.to_string()),
                    tags: HashMap::new(),
                });
            }
        }

        Ok(groups)
    }

    async fn describe_db_parameter_groups(&self) -> Result<Vec<RdsParameterGroupInfo>> {
        let mut groups = Vec::new();
        let mut paginator = self
            .client
            .describe_db_parameter_groups()
            .into_paginator()
            .send();

        while let Some(page) = paginator.next().await {
            let page =
                page.map_err(|e| anyhow!("Failed to describe DB parameter groups: {}", e))?;

            for group in page.db_parameter_groups() {
                groups.push(RdsParameterGroupInfo {
                    db_parameter_group_name: group
                        .db_parameter_group_name()
                        .unwrap_or_default()
                        .to_string(),
                    family: group.db_parameter_group_family().map(|s| s.to_string()),
                    description: group.description().map(|s| s.to_string()),
                    tags: HashMap::new(),
                });
            }
        }

        Ok(groups)
    }

    async fn describe_db_parameters(
        &self,
        parameter_group_name: &str,
    ) -> Result<Vec<RdsParameterInfo>> {
        let mut params = Vec::new();
        let mut paginator = self
            .client
            .describe_db_parameters()
            .db_parameter_group_name(parameter_group_name)
            .into_paginator()
            .send();

        while let Some(page) = paginator.next().await {
            let page = page.map_err(|e| anyhow!("Failed to describe DB parameters: {}", e))?;

            for param in page.parameters() {
                // user変更パラメータのみ取得
                if param.source().unwrap_or_default() == "user" {
                    if let Some(value) = param.parameter_value() {
                        params.push(RdsParameterInfo {
                            name: param.parameter_name().unwrap_or_default().to_string(),
                            value: value.to_string(),
                            apply_method: param.apply_method().map(|m| m.as_str().to_string()),
                        });
                    }
                }
            }
        }

        Ok(params)
    }
}
