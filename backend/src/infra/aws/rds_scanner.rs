//! AWS RDSスキャナー

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

use crate::infra::aws::rds_client_trait::RdsClientOps;
use crate::models::ScanConfig;

/// AWS RDSスキャナー
pub struct AwsRdsScanner<C: RdsClientOps> {
    config: ScanConfig,
    rds_client: Arc<C>,
}

impl<C: RdsClientOps> AwsRdsScanner<C> {
    /// 本番用・テスト用共通：クライアントを指定してスキャナーを作成
    pub fn new(config: ScanConfig, client: Arc<C>) -> Self {
        Self {
            config,
            rds_client: client,
        }
    }

    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: C) -> Self {
        Self {
            config,
            rds_client: Arc::new(client),
        }
    }

    /// RDSリソースをスキャンし結果をresultsに追加
    pub async fn scan_into(
        &self,
        results: &mut serde_json::Map<String, Value>,
        progress_callback: &(dyn Fn(u32, String) + Send + Sync),
        completed_targets: &AtomicUsize,
        total_targets: usize,
    ) -> Result<()> {
        let scan_targets = &self.config.scan_targets;

        // DB Instances
        if scan_targets.get("db_instances").copied().unwrap_or(false) {
            debug!("RDS DB Instancesのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "RDS DB Instancesのスキャン中...".to_string(),
            );
            let instances = self.scan_db_instances().await?;
            let count = instances.len();
            results.insert("db_instances".to_string(), Value::Array(instances));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("RDS DB Instancesのスキャン完了: {}件", count),
            );
        } else {
            results.insert("db_instances".to_string(), Value::Array(Vec::new()));
        }

        // DB Subnet Groups
        if scan_targets
            .get("db_subnet_groups")
            .copied()
            .unwrap_or(false)
        {
            debug!("RDS DB Subnet Groupsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "RDS DB Subnet Groupsのスキャン中...".to_string(),
            );
            let groups = self.scan_db_subnet_groups().await?;
            let count = groups.len();
            results.insert("db_subnet_groups".to_string(), Value::Array(groups));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("RDS DB Subnet Groupsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("db_subnet_groups".to_string(), Value::Array(Vec::new()));
        }

        // Parameter Groups
        if scan_targets
            .get("db_parameter_groups")
            .copied()
            .unwrap_or(false)
        {
            debug!("RDS Parameter Groupsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "RDS Parameter Groupsのスキャン中...".to_string(),
            );
            let groups = self.scan_db_parameter_groups().await?;
            let count = groups.len();
            results.insert("db_parameter_groups".to_string(), Value::Array(groups));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("RDS Parameter Groupsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("db_parameter_groups".to_string(), Value::Array(Vec::new()));
        }

        Ok(())
    }

    async fn scan_db_instances(&self) -> Result<Vec<Value>> {
        let instances_info = self.rds_client.describe_db_instances().await?;
        let instances: Vec<Value> = instances_info
            .into_iter()
            .map(|inst| {
                let mut j = json!({
                    "db_instance_identifier": inst.db_instance_identifier,
                    "multi_az": inst.multi_az,
                    "publicly_accessible": inst.publicly_accessible,
                    "storage_encrypted": inst.storage_encrypted,
                });
                if let Some(v) = &inst.db_instance_class {
                    j["db_instance_class"] = json!(v);
                }
                if let Some(v) = &inst.engine {
                    j["engine"] = json!(v);
                }
                if let Some(v) = &inst.engine_version {
                    j["engine_version"] = json!(v);
                }
                if let Some(v) = &inst.status {
                    j["status"] = json!(v);
                }
                if let Some(v) = inst.allocated_storage {
                    j["allocated_storage"] = json!(v);
                }
                if let Some(v) = &inst.storage_type {
                    j["storage_type"] = json!(v);
                }
                if let Some(v) = &inst.vpc_id {
                    j["vpc_id"] = json!(v);
                }
                if let Some(v) = &inst.db_subnet_group_name {
                    j["db_subnet_group_name"] = json!(v);
                }
                if let Some(v) = &inst.availability_zone {
                    j["availability_zone"] = json!(v);
                }
                if let Some(v) = &inst.endpoint {
                    j["endpoint"] = json!(v);
                }
                if let Some(v) = inst.port {
                    j["port"] = json!(v);
                }
                if let Some(v) = &inst.master_username {
                    j["master_username"] = json!(v);
                }
                if !inst.security_groups.is_empty() {
                    j["security_groups"] = json!(inst.security_groups);
                }
                if let Some(v) = &inst.parameter_group_name {
                    j["parameter_group_name"] = json!(v);
                }
                if let Some(v) = inst.backup_retention_period {
                    j["backup_retention_period"] = json!(v);
                }
                if !inst.tags.is_empty() {
                    j["tags"] = json!(inst.tags);
                }
                j
            })
            .collect();
        info!(count = instances.len(), "RDS DBインスタンススキャン完了");
        Ok(instances)
    }

    async fn scan_db_subnet_groups(&self) -> Result<Vec<Value>> {
        let groups_info = self.rds_client.describe_db_subnet_groups().await?;
        let groups: Vec<Value> = groups_info
            .into_iter()
            .map(|g| {
                let mut j = json!({
                    "db_subnet_group_name": g.db_subnet_group_name,
                    "subnet_ids": g.subnet_ids,
                });
                if let Some(v) = &g.description {
                    j["description"] = json!(v);
                }
                if let Some(v) = &g.vpc_id {
                    j["vpc_id"] = json!(v);
                }
                if let Some(v) = &g.status {
                    j["status"] = json!(v);
                }
                if !g.tags.is_empty() {
                    j["tags"] = json!(g.tags);
                }
                j
            })
            .collect();
        Ok(groups)
    }

    async fn scan_db_parameter_groups(&self) -> Result<Vec<Value>> {
        let groups_info = self.rds_client.describe_db_parameter_groups().await?;
        let mut groups = Vec::new();

        for pg in groups_info {
            let params = self
                .rds_client
                .describe_db_parameters(&pg.db_parameter_group_name)
                .await
                .unwrap_or_default();

            let params_json: Vec<Value> = params
                .into_iter()
                .map(|p| {
                    let mut j = json!({
                        "name": p.name,
                        "value": p.value,
                    });
                    if let Some(v) = &p.apply_method {
                        j["apply_method"] = json!(v);
                    }
                    j
                })
                .collect();

            let mut j = json!({
                "db_parameter_group_name": pg.db_parameter_group_name,
            });
            if let Some(v) = &pg.family {
                j["family"] = json!(v);
            }
            if let Some(v) = &pg.description {
                j["description"] = json!(v);
            }
            if !params_json.is_empty() {
                j["parameters"] = json!(params_json);
            }
            if !pg.tags.is_empty() {
                j["tags"] = json!(pg.tags);
            }

            groups.push(j);
        }

        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::rds_client_trait::mock::MockRdsClient;
    use crate::infra::aws::rds_client_trait::{RdsInstanceInfo, RdsSubnetGroupInfo};
    use std::collections::HashMap;

    fn make_test_config(targets: Vec<&str>) -> ScanConfig {
        let mut scan_targets = HashMap::new();
        for t in targets {
            scan_targets.insert(t.to_string(), true);
        }
        ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets,
            filters: HashMap::new(),
            include_tags: true,
        }
    }

    #[tokio::test]
    async fn test_scan_db_instances() {
        let mut mock = MockRdsClient::new();
        mock.expect_describe_db_instances().returning(|| {
            Ok(vec![RdsInstanceInfo {
                db_instance_identifier: "mydb".to_string(),
                db_instance_class: Some("db.t3.micro".to_string()),
                engine: Some("postgres".to_string()),
                engine_version: Some("15.4".to_string()),
                status: Some("available".to_string()),
                allocated_storage: Some(20),
                storage_type: Some("gp3".to_string()),
                multi_az: false,
                publicly_accessible: false,
                vpc_id: Some("vpc-12345".to_string()),
                db_subnet_group_name: Some("my-sg".to_string()),
                availability_zone: Some("ap-northeast-1a".to_string()),
                endpoint: Some("mydb.example.rds.amazonaws.com".to_string()),
                port: Some(5432),
                master_username: Some("admin".to_string()),
                security_groups: vec!["sg-12345".to_string()],
                parameter_group_name: Some("default.postgres15".to_string()),
                backup_retention_period: Some(7),
                storage_encrypted: true,
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["db_instances"]);
        let scanner = AwsRdsScanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let instances = results.get("db_instances").unwrap().as_array().unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0]["db_instance_identifier"], "mydb");
        assert_eq!(instances[0]["engine"], "postgres");
    }

    #[tokio::test]
    async fn test_scan_db_subnet_groups() {
        let mut mock = MockRdsClient::new();
        mock.expect_describe_db_subnet_groups().returning(|| {
            Ok(vec![RdsSubnetGroupInfo {
                db_subnet_group_name: "my-subnet-group".to_string(),
                description: Some("Test group".to_string()),
                vpc_id: Some("vpc-12345".to_string()),
                subnet_ids: vec!["subnet-1".to_string(), "subnet-2".to_string()],
                status: Some("Complete".to_string()),
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["db_subnet_groups"]);
        let scanner = AwsRdsScanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let groups = results.get("db_subnet_groups").unwrap().as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["db_subnet_group_name"], "my-subnet-group");
    }

    #[tokio::test]
    async fn test_scan_db_parameter_groups() {
        use crate::infra::aws::rds_client_trait::{RdsParameterGroupInfo, RdsParameterInfo};

        let mut mock = MockRdsClient::new();
        mock.expect_describe_db_parameter_groups().returning(|| {
            Ok(vec![RdsParameterGroupInfo {
                db_parameter_group_name: "my-pg".to_string(),
                family: Some("postgres15".to_string()),
                description: Some("My parameter group".to_string()),
                tags: HashMap::new(),
            }])
        });
        mock.expect_describe_db_parameters()
            .withf(|name| name == "my-pg")
            .returning(|_| {
                Ok(vec![RdsParameterInfo {
                    name: "max_connections".to_string(),
                    value: "200".to_string(),
                    apply_method: Some("pending-reboot".to_string()),
                }])
            });

        let config = make_test_config(vec!["db_parameter_groups"]);
        let scanner = AwsRdsScanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let groups = results
            .get("db_parameter_groups")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["db_parameter_group_name"], "my-pg");
        assert_eq!(groups[0]["family"], "postgres15");
        let params = groups[0]["parameters"].as_array().unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0]["name"], "max_connections");
    }

    #[tokio::test]
    async fn test_scan_db_subnet_groups_with_tags() {
        let mut mock = MockRdsClient::new();
        mock.expect_describe_db_subnet_groups().returning(|| {
            let mut tags = HashMap::new();
            tags.insert("env".to_string(), "prod".to_string());
            tags.insert("team".to_string(), "infra".to_string());
            Ok(vec![RdsSubnetGroupInfo {
                db_subnet_group_name: "prod-subnet-group".to_string(),
                description: Some("Production subnet group".to_string()),
                vpc_id: Some("vpc-prod123".to_string()),
                subnet_ids: vec![
                    "subnet-a".to_string(),
                    "subnet-b".to_string(),
                    "subnet-c".to_string(),
                ],
                status: Some("Complete".to_string()),
                tags,
            }])
        });

        let config = make_test_config(vec!["db_subnet_groups"]);
        let scanner = AwsRdsScanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let groups = results.get("db_subnet_groups").unwrap().as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["db_subnet_group_name"], "prod-subnet-group");
        assert_eq!(groups[0]["vpc_id"], "vpc-prod123");
        // タグが含まれていることを確認
        assert!(groups[0]["tags"].is_object(), "Tags should be present");
        let subnet_ids = groups[0]["subnet_ids"].as_array().unwrap();
        assert_eq!(subnet_ids.len(), 3);
    }
}
