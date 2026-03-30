//! AWS EC2スキャナー

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

use crate::infra::aws::ec2_client_trait::Ec2ClientOps;
use crate::models::ScanConfig;

/// AWS EC2スキャナー
pub struct AwsEc2Scanner<C: Ec2ClientOps> {
    config: ScanConfig,
    ec2_client: Arc<C>,
}

impl<C: Ec2ClientOps> AwsEc2Scanner<C> {
    /// 本番用・テスト用共通：クライアントを指定してスキャナーを作成
    pub fn new(config: ScanConfig, client: Arc<C>) -> Self {
        Self {
            config,
            ec2_client: client,
        }
    }

    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: C) -> Self {
        Self {
            config,
            ec2_client: Arc::new(client),
        }
    }

    /// EC2リソースをスキャンし結果をresultsに追加
    pub async fn scan_into(
        &self,
        results: &mut serde_json::Map<String, Value>,
        progress_callback: &(dyn Fn(u32, String) + Send + Sync),
        completed_targets: &AtomicUsize,
        total_targets: usize,
    ) -> Result<()> {
        let scan_targets = &self.config.scan_targets;

        if scan_targets.get("instances").copied().unwrap_or(false) {
            debug!("EC2 Instancesのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "EC2 Instancesのスキャン中...".to_string(),
            );
            let instances = self.scan_instances().await?;
            let count = instances.len();
            results.insert("instances".to_string(), Value::Array(instances));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            debug!(count, "EC2 Instancesのスキャン完了");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("EC2 Instancesのスキャン完了: {}件", count),
            );
        } else {
            results.insert("instances".to_string(), Value::Array(Vec::new()));
        }

        Ok(())
    }

    async fn scan_instances(&self) -> Result<Vec<Value>> {
        let instances_info = self.ec2_client.describe_instances().await?;
        let mut instances = Vec::new();

        for inst in instances_info {
            let mut inst_json = json!({
                "instance_id": inst.instance_id,
            });

            if let Some(v) = &inst.instance_type {
                inst_json["instance_type"] = json!(v);
            }
            if let Some(v) = &inst.state {
                inst_json["state"] = json!(v);
            }
            if let Some(v) = &inst.ami_id {
                inst_json["ami_id"] = json!(v);
            }
            if let Some(v) = &inst.vpc_id {
                inst_json["vpc_id"] = json!(v);
            }
            if let Some(v) = &inst.subnet_id {
                inst_json["subnet_id"] = json!(v);
            }
            if let Some(v) = &inst.private_ip {
                inst_json["private_ip"] = json!(v);
            }
            if let Some(v) = &inst.public_ip {
                inst_json["public_ip"] = json!(v);
            }
            if let Some(v) = &inst.key_name {
                inst_json["key_name"] = json!(v);
            }
            if let Some(v) = &inst.iam_instance_profile {
                inst_json["iam_instance_profile"] = json!(v);
            }
            if !inst.security_groups.is_empty() {
                inst_json["security_groups"] = json!(inst.security_groups);
            }
            if !inst.tags.is_empty() {
                inst_json["tags"] = json!(inst.tags);
            }

            instances.push(inst_json);
        }

        info!(count = instances.len(), "EC2インスタンススキャン完了");
        Ok(instances)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::ec2_client_trait::mock::MockEc2Client;
    use crate::infra::aws::ec2_client_trait::Ec2InstanceInfo;
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
    async fn test_scan_instances() {
        let mut mock = MockEc2Client::new();
        mock.expect_describe_instances().returning(|| {
            Ok(vec![Ec2InstanceInfo {
                instance_id: "i-12345".to_string(),
                instance_type: Some("t3.micro".to_string()),
                state: Some("running".to_string()),
                ami_id: Some("ami-12345".to_string()),
                vpc_id: Some("vpc-12345".to_string()),
                subnet_id: Some("subnet-12345".to_string()),
                private_ip: Some("10.0.1.100".to_string()),
                public_ip: None,
                key_name: Some("my-key".to_string()),
                iam_instance_profile: None,
                security_groups: vec!["sg-12345".to_string()],
                tags: HashMap::from([("Name".to_string(), "test".to_string())]),
            }])
        });

        let config = make_test_config(vec!["instances"]);
        let scanner = AwsEc2Scanner::new_with_client(config, mock);
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let instances = results.get("instances").unwrap().as_array().unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0]["instance_id"], "i-12345");
        assert_eq!(instances[0]["instance_type"], "t3.micro");
    }
}
