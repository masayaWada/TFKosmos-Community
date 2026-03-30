use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::infra::terraform::state_parser::TfState;
use crate::models::drift::{
    ChangedField, DriftDetectionResponse, DriftItem, DriftSummary, DriftType,
};
use crate::services::scan_service::ScanService;

/// ドリフト検出サービス
///
/// Terraform state ファイルとクラウドスキャン結果を比較し、差分を検出する。
pub struct DriftService {
    scan_service: Arc<ScanService>,
    reports: Arc<RwLock<HashMap<String, DriftDetectionResponse>>>,
}

impl DriftService {
    pub fn new(scan_service: Arc<ScanService>) -> Self {
        Self {
            scan_service,
            reports: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Terraform state とスキャン結果を比較してドリフトを検出する
    pub async fn detect_drift(
        &self,
        scan_id: &str,
        state_content: &str,
    ) -> Result<DriftDetectionResponse> {
        // Parse tfstate
        let tf_state = TfState::parse(state_content)?;

        // Get scan data
        let scan_data = self
            .scan_service
            .get_scan_data(scan_id)
            .await
            .context(format!("スキャンID {} が見つかりません", scan_id))?;

        let scan_map = scan_data.as_object().cloned().unwrap_or_default();

        // Build comparison
        let state_resources = tf_state.resources_by_type();
        let mut drifts = Vec::new();
        let mut total_in_state = 0usize;
        let mut total_in_cloud = 0usize;
        let mut added = 0usize;
        let mut removed = 0usize;
        let mut changed = 0usize;
        let mut unchanged = 0usize;

        let type_mapping = build_type_mapping();

        // Check state resources against cloud
        for (tf_type, state_res_list) in &state_resources {
            for state_res in state_res_list {
                total_in_state += state_res.instances.len();
            }

            if let Some(scan_key) = type_mapping.get(tf_type.as_str()) {
                let cloud_resources = scan_map
                    .get(*scan_key)
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                for state_res in state_res_list {
                    for instance in &state_res.instances {
                        let state_id = extract_resource_id(tf_type, &instance.attributes);

                        let cloud_match = cloud_resources.iter().find(|cr| {
                            let cloud_id = extract_cloud_resource_id(tf_type, cr);
                            cloud_id == state_id
                        });

                        match cloud_match {
                            Some(cloud_res) => {
                                let changes = compare_attributes(&instance.attributes, cloud_res);
                                if changes.is_empty() {
                                    unchanged += 1;
                                } else {
                                    changed += 1;
                                    drifts.push(DriftItem {
                                        resource_type: tf_type.clone(),
                                        resource_id: state_id,
                                        drift_type: DriftType::Changed,
                                        state_attributes: Some(instance.attributes.clone()),
                                        cloud_attributes: Some(cloud_res.clone()),
                                        changed_fields: changes,
                                    });
                                }
                            }
                            None => {
                                removed += 1;
                                drifts.push(DriftItem {
                                    resource_type: tf_type.clone(),
                                    resource_id: state_id,
                                    drift_type: DriftType::Removed,
                                    state_attributes: Some(instance.attributes.clone()),
                                    cloud_attributes: None,
                                    changed_fields: vec![],
                                });
                            }
                        }
                    }
                }
            }
        }

        // Check cloud resources not in state
        let reverse_map = reverse_type_mapping();
        for (scan_key, cloud_value) in &scan_map {
            if let Some(cloud_resources) = cloud_value.as_array() {
                if let Some(tf_type) = reverse_map.get(scan_key.as_str()) {
                    total_in_cloud += cloud_resources.len();

                    let state_res_list = state_resources.get(*tf_type).cloned().unwrap_or_default();

                    for cloud_res in cloud_resources {
                        let cloud_id = extract_cloud_resource_id(tf_type, cloud_res);

                        let in_state = state_res_list.iter().any(|sr| {
                            sr.instances.iter().any(|inst| {
                                extract_resource_id(tf_type, &inst.attributes) == cloud_id
                            })
                        });

                        if !in_state {
                            added += 1;
                            drifts.push(DriftItem {
                                resource_type: tf_type.to_string(),
                                resource_id: cloud_id,
                                drift_type: DriftType::Added,
                                state_attributes: None,
                                cloud_attributes: Some(cloud_res.clone()),
                                changed_fields: vec![],
                            });
                        }
                    }
                }
            }
        }

        let drift_id = Uuid::new_v4().to_string();
        let response = DriftDetectionResponse {
            drift_id: drift_id.clone(),
            scan_id: scan_id.to_string(),
            summary: DriftSummary {
                total_in_state,
                total_in_cloud,
                added,
                removed,
                changed,
                unchanged,
            },
            drifts,
        };

        // Store report
        self.reports
            .write()
            .await
            .insert(drift_id, response.clone());

        Ok(response)
    }

    /// 保存済みのドリフトレポートを取得する
    pub async fn get_report(&self, drift_id: &str) -> Option<DriftDetectionResponse> {
        self.reports.read().await.get(drift_id).cloned()
    }

    /// テスト用: 全レポートをクリアする
    #[cfg(test)]
    #[allow(dead_code)]
    pub async fn clear_all(&self) {
        self.reports.write().await.clear();
    }
}

/// Terraform resource type -> scan data key のマッピング
fn build_type_mapping() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    // IAM
    m.insert("aws_iam_user", "users");
    m.insert("aws_iam_group", "groups");
    m.insert("aws_iam_role", "roles");
    m.insert("aws_iam_policy", "policies");
    // S3
    m.insert("aws_s3_bucket", "buckets");
    // EC2
    m.insert("aws_instance", "instances");
    // VPC
    m.insert("aws_vpc", "vpcs");
    m.insert("aws_subnet", "subnets");
    m.insert("aws_route_table", "route_tables");
    m.insert("aws_security_group", "security_groups");
    // RDS
    m.insert("aws_db_instance", "db_instances");
    // Lambda
    m.insert("aws_lambda_function", "functions");
    // DynamoDB
    m.insert("aws_dynamodb_table", "dynamodb_tables");
    m
}

/// scan data key -> Terraform resource type の逆引きマッピング
fn reverse_type_mapping() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    for (tf_type, scan_key) in build_type_mapping() {
        m.insert(scan_key, tf_type);
    }
    m
}

/// tfstate attributes からリソース識別子を抽出する
fn extract_resource_id(tf_type: &str, attributes: &Value) -> String {
    let id_field = match tf_type {
        "aws_s3_bucket" => "bucket",
        "aws_instance" => "id",
        "aws_vpc" => "id",
        "aws_subnet" => "id",
        "aws_security_group" => "id",
        "aws_iam_user" => "name",
        "aws_iam_group" => "name",
        "aws_iam_role" => "name",
        "aws_iam_policy" => "arn",
        "aws_db_instance" => "identifier",
        "aws_lambda_function" => "function_name",
        "aws_dynamodb_table" => "name",
        "aws_route_table" => "id",
        _ => "id",
    };
    attributes
        .get(id_field)
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

/// クラウドスキャンデータからリソース識別子を抽出する
fn extract_cloud_resource_id(tf_type: &str, cloud_res: &Value) -> String {
    let id_field = match tf_type {
        "aws_s3_bucket" => "name",
        "aws_instance" => "instance_id",
        "aws_vpc" => "vpc_id",
        "aws_subnet" => "subnet_id",
        "aws_security_group" => "group_id",
        "aws_iam_user" => "user_name",
        "aws_iam_group" => "group_name",
        "aws_iam_role" => "role_name",
        "aws_iam_policy" => "arn",
        "aws_db_instance" => "db_instance_identifier",
        "aws_lambda_function" => "function_name",
        "aws_dynamodb_table" => "table_name",
        "aws_route_table" => "route_table_id",
        _ => "id",
    };
    cloud_res
        .get(id_field)
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

/// state と cloud の共通キー属性を比較し、差分のあるフィールドを返す
fn compare_attributes(state_attrs: &Value, cloud_attrs: &Value) -> Vec<ChangedField> {
    let mut changes = Vec::new();

    if let (Some(state_obj), Some(cloud_obj)) = (state_attrs.as_object(), cloud_attrs.as_object()) {
        // メタデータ系フィールドは比較対象外
        let skip_fields = [
            "id",
            "arn",
            "created_at",
            "updated_at",
            "last_modified",
            "creation_date",
        ];

        for (key, state_val) in state_obj {
            if skip_fields.contains(&key.as_str()) {
                continue;
            }
            if let Some(cloud_val) = cloud_obj.get(key) {
                if state_val != cloud_val {
                    changes.push(ChangedField {
                        field: key.clone(),
                        state_value: state_val.clone(),
                        cloud_value: cloud_val.clone(),
                    });
                }
            }
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ScanConfig;
    use crate::services::scan_service::{MockScannerFactory, ScanService};

    fn create_test_scan_service() -> Arc<ScanService> {
        let mut mock = MockScannerFactory::new();
        mock.expect_run_scan().returning(|_, _| {
            Ok(serde_json::json!({
                "provider": "aws",
                "users": [],
            }))
        });
        Arc::new(ScanService::new(Arc::new(mock)))
    }

    fn test_scan_config() -> ScanConfig {
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
            scan_targets: std::collections::HashMap::new(),
            filters: std::collections::HashMap::new(),
            include_tags: true,
        }
    }

    fn make_state_json(resources: Value) -> String {
        serde_json::json!({
            "version": 4,
            "terraform_version": "1.5.0",
            "serial": 1,
            "lineage": "test",
            "resources": resources
        })
        .to_string()
    }

    #[tokio::test]
    async fn test_detect_drift_no_drift() {
        let scan_service = create_test_scan_service();
        let scan_data = serde_json::json!({
            "buckets": [
                { "name": "my-bucket", "acl": "private" }
            ]
        });
        scan_service
            .insert_test_scan_data("scan-1".to_string(), test_scan_config(), scan_data)
            .await;

        let drift_service = DriftService::new(scan_service);

        let state = make_state_json(serde_json::json!([
            {
                "mode": "managed",
                "type": "aws_s3_bucket",
                "name": "my_bucket",
                "provider": "aws",
                "instances": [
                    { "attributes": { "bucket": "my-bucket", "acl": "private" } }
                ]
            }
        ]));

        let result = drift_service.detect_drift("scan-1", &state).await.unwrap();
        assert_eq!(result.summary.unchanged, 1);
        assert_eq!(result.summary.added, 0);
        assert_eq!(result.summary.removed, 0);
        assert_eq!(result.summary.changed, 0);
        assert!(result.drifts.is_empty());
    }

    #[tokio::test]
    async fn test_detect_drift_added() {
        let scan_service = create_test_scan_service();
        let scan_data = serde_json::json!({
            "buckets": [
                { "name": "bucket-a" },
                { "name": "bucket-b" }
            ]
        });
        scan_service
            .insert_test_scan_data("scan-2".to_string(), test_scan_config(), scan_data)
            .await;

        let drift_service = DriftService::new(scan_service);

        // State only has bucket-a
        let state = make_state_json(serde_json::json!([
            {
                "mode": "managed",
                "type": "aws_s3_bucket",
                "name": "a",
                "provider": "aws",
                "instances": [
                    { "attributes": { "bucket": "bucket-a" } }
                ]
            }
        ]));

        let result = drift_service.detect_drift("scan-2", &state).await.unwrap();
        assert_eq!(result.summary.added, 1, "bucket-b should be added");
        let added_items: Vec<_> = result
            .drifts
            .iter()
            .filter(|d| d.drift_type == DriftType::Added)
            .collect();
        assert_eq!(added_items.len(), 1);
        assert_eq!(added_items[0].resource_id, "bucket-b");
    }

    #[tokio::test]
    async fn test_detect_drift_removed() {
        let scan_service = create_test_scan_service();
        // Cloud has no buckets
        let scan_data = serde_json::json!({
            "buckets": []
        });
        scan_service
            .insert_test_scan_data("scan-3".to_string(), test_scan_config(), scan_data)
            .await;

        let drift_service = DriftService::new(scan_service);

        let state = make_state_json(serde_json::json!([
            {
                "mode": "managed",
                "type": "aws_s3_bucket",
                "name": "old",
                "provider": "aws",
                "instances": [
                    { "attributes": { "bucket": "deleted-bucket" } }
                ]
            }
        ]));

        let result = drift_service.detect_drift("scan-3", &state).await.unwrap();
        assert_eq!(result.summary.removed, 1);
        let removed: Vec<_> = result
            .drifts
            .iter()
            .filter(|d| d.drift_type == DriftType::Removed)
            .collect();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].resource_id, "deleted-bucket");
    }

    #[tokio::test]
    async fn test_detect_drift_changed() {
        let scan_service = create_test_scan_service();
        let scan_data = serde_json::json!({
            "buckets": [
                { "name": "my-bucket", "acl": "public-read" }
            ]
        });
        scan_service
            .insert_test_scan_data("scan-4".to_string(), test_scan_config(), scan_data)
            .await;

        let drift_service = DriftService::new(scan_service);

        let state = make_state_json(serde_json::json!([
            {
                "mode": "managed",
                "type": "aws_s3_bucket",
                "name": "b",
                "provider": "aws",
                "instances": [
                    { "attributes": { "bucket": "my-bucket", "acl": "private" } }
                ]
            }
        ]));

        let result = drift_service.detect_drift("scan-4", &state).await.unwrap();
        assert_eq!(result.summary.changed, 1);
        let changed: Vec<_> = result
            .drifts
            .iter()
            .filter(|d| d.drift_type == DriftType::Changed)
            .collect();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].changed_fields.len(), 1);
        assert_eq!(changed[0].changed_fields[0].field, "acl");
    }

    #[tokio::test]
    async fn test_detect_drift_scan_not_found() {
        let scan_service = create_test_scan_service();
        let drift_service = DriftService::new(scan_service);

        let state = make_state_json(serde_json::json!([]));
        let result = drift_service.detect_drift("nonexistent", &state).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("見つかりません"));
    }

    #[tokio::test]
    async fn test_get_report() {
        let scan_service = create_test_scan_service();
        let scan_data = serde_json::json!({ "buckets": [] });
        scan_service
            .insert_test_scan_data("scan-5".to_string(), test_scan_config(), scan_data)
            .await;

        let drift_service = DriftService::new(scan_service);

        let state = make_state_json(serde_json::json!([]));
        let result = drift_service.detect_drift("scan-5", &state).await.unwrap();
        let drift_id = result.drift_id.clone();

        // Should be retrievable
        let report = drift_service.get_report(&drift_id).await;
        assert!(report.is_some());
        assert_eq!(report.unwrap().scan_id, "scan-5");

        // Unknown ID should return None
        assert!(drift_service.get_report("unknown-id").await.is_none());
    }

    #[test]
    fn test_extract_resource_id() {
        let attrs = serde_json::json!({ "bucket": "test-bucket", "acl": "private" });
        assert_eq!(extract_resource_id("aws_s3_bucket", &attrs), "test-bucket");

        let attrs = serde_json::json!({ "id": "i-123", "instance_type": "t3.micro" });
        assert_eq!(extract_resource_id("aws_instance", &attrs), "i-123");

        let attrs = serde_json::json!({ "name": "admin" });
        assert_eq!(extract_resource_id("aws_iam_user", &attrs), "admin");

        let attrs = serde_json::json!({ "function_name": "my-func" });
        assert_eq!(
            extract_resource_id("aws_lambda_function", &attrs),
            "my-func"
        );

        // Unknown type falls back to "id"
        let attrs = serde_json::json!({ "id": "x" });
        assert_eq!(extract_resource_id("aws_unknown", &attrs), "x");

        // Missing field returns "unknown"
        let attrs = serde_json::json!({});
        assert_eq!(extract_resource_id("aws_s3_bucket", &attrs), "unknown");
    }

    #[test]
    fn test_extract_cloud_resource_id() {
        let res = serde_json::json!({ "name": "cloud-bucket" });
        assert_eq!(
            extract_cloud_resource_id("aws_s3_bucket", &res),
            "cloud-bucket"
        );

        let res = serde_json::json!({ "instance_id": "i-abc" });
        assert_eq!(extract_cloud_resource_id("aws_instance", &res), "i-abc");

        let res = serde_json::json!({ "user_name": "alice" });
        assert_eq!(extract_cloud_resource_id("aws_iam_user", &res), "alice");

        let res = serde_json::json!({ "table_name": "orders" });
        assert_eq!(
            extract_cloud_resource_id("aws_dynamodb_table", &res),
            "orders"
        );

        // Missing field returns "unknown"
        let res = serde_json::json!({});
        assert_eq!(extract_cloud_resource_id("aws_s3_bucket", &res), "unknown");
    }
}
