//! AWS ELB/ALBスキャナー

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

use crate::infra::aws::elb_client_trait::ELBClientOps;
use crate::models::ScanConfig;

/// AWS ELBスキャナー
pub struct AwsELBScanner<C: ELBClientOps> {
    config: ScanConfig,
    elb_client: Arc<C>,
}

impl<C: ELBClientOps> AwsELBScanner<C> {
    pub fn new(config: ScanConfig, client: Arc<C>) -> Self {
        Self {
            config,
            elb_client: client,
        }
    }

    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: C) -> Self {
        Self {
            config,
            elb_client: Arc::new(client),
        }
    }

    pub async fn scan_into(
        &self,
        results: &mut serde_json::Map<String, Value>,
        progress_callback: &(dyn Fn(u32, String) + Send + Sync),
        completed_targets: &AtomicUsize,
        total_targets: usize,
    ) -> Result<()> {
        let scan_targets = &self.config.scan_targets;

        // Load Balancers
        if scan_targets.get("load_balancers").copied().unwrap_or(false) {
            debug!("Load Balancersのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "Load Balancersのスキャン中...".to_string(),
            );
            let lbs = self.scan_load_balancers().await?;
            let count = lbs.len();

            // リスナーも同時取得
            let mut all_listeners = Vec::new();
            for lb in &lbs {
                if let Some(arn) = lb.get("load_balancer_arn").and_then(|v| v.as_str()) {
                    if let Ok(listeners) = self.scan_listeners(arn).await {
                        all_listeners.extend(listeners);
                    }
                }
            }

            results.insert("load_balancers".to_string(), Value::Array(lbs));
            results.insert("lb_listeners".to_string(), Value::Array(all_listeners));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("Load Balancersのスキャン完了: {}件", count),
            );
        } else {
            results.insert("load_balancers".to_string(), Value::Array(Vec::new()));
            results.insert("lb_listeners".to_string(), Value::Array(Vec::new()));
        }

        // Target Groups
        if scan_targets
            .get("lb_target_groups")
            .copied()
            .unwrap_or(false)
        {
            debug!("Target Groupsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "Target Groupsのスキャン中...".to_string(),
            );
            let tgs = self.scan_target_groups().await?;
            let count = tgs.len();
            results.insert("lb_target_groups".to_string(), Value::Array(tgs));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("Target Groupsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("lb_target_groups".to_string(), Value::Array(Vec::new()));
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

    async fn scan_load_balancers(&self) -> Result<Vec<Value>> {
        info!("Load Balancers一覧を取得中");
        let lbs = self.elb_client.describe_load_balancers().await?;
        let result: Vec<Value> = lbs
            .into_iter()
            .filter(|lb| self.apply_name_prefix_filter(&lb.name))
            .map(|lb| {
                let mut j = json!({
                    "name": lb.name,
                    "load_balancer_arn": lb.load_balancer_arn,
                });
                if let Some(dns) = &lb.dns_name {
                    j["dns_name"] = json!(dns);
                }
                if let Some(scheme) = &lb.scheme {
                    j["scheme"] = json!(scheme);
                }
                if let Some(t) = &lb.lb_type {
                    j["type"] = json!(t);
                }
                if let Some(vpc) = &lb.vpc_id {
                    j["vpc_id"] = json!(vpc);
                }
                if !lb.subnets.is_empty() {
                    j["subnets"] = json!(lb.subnets);
                }
                if !lb.security_groups.is_empty() {
                    j["security_groups"] = json!(lb.security_groups);
                }
                if !lb.tags.is_empty() {
                    let tags: Vec<Value> = lb
                        .tags
                        .iter()
                        .map(|(k, v)| json!({"key": k, "value": v}))
                        .collect();
                    j["tags"] = json!(tags);
                }
                j
            })
            .collect();
        info!(count = result.len(), "Load Balancers取得完了");
        Ok(result)
    }

    async fn scan_listeners(&self, lb_arn: &str) -> Result<Vec<Value>> {
        let listeners = self.elb_client.describe_listeners(lb_arn).await?;
        let result: Vec<Value> = listeners
            .into_iter()
            .map(|l| {
                let actions: Vec<Value> = l
                    .default_actions
                    .iter()
                    .map(|a| {
                        let mut aj = json!({"type": a.action_type});
                        if let Some(tg) = &a.target_group_arn {
                            aj["target_group_arn"] = json!(tg);
                        }
                        aj
                    })
                    .collect();

                let mut j = json!({
                    "listener_arn": l.listener_arn,
                    "load_balancer_arn": l.load_balancer_arn,
                });
                if let Some(port) = l.port {
                    j["port"] = json!(port);
                }
                if let Some(proto) = &l.protocol {
                    j["protocol"] = json!(proto);
                }
                if !actions.is_empty() {
                    j["default_actions"] = json!(actions);
                }
                j
            })
            .collect();
        Ok(result)
    }

    async fn scan_target_groups(&self) -> Result<Vec<Value>> {
        info!("Target Groups一覧を取得中");
        let tgs = self.elb_client.describe_target_groups().await?;
        let result: Vec<Value> = tgs
            .into_iter()
            .filter(|tg| self.apply_name_prefix_filter(&tg.name))
            .map(|tg| {
                let mut j = json!({
                    "name": tg.name,
                    "target_group_arn": tg.target_group_arn,
                });
                if let Some(port) = tg.port {
                    j["port"] = json!(port);
                }
                if let Some(proto) = &tg.protocol {
                    j["protocol"] = json!(proto);
                }
                if let Some(vpc) = &tg.vpc_id {
                    j["vpc_id"] = json!(vpc);
                }
                if let Some(tt) = &tg.target_type {
                    j["target_type"] = json!(tt);
                }
                // Health check
                let mut hc = serde_json::Map::new();
                if let Some(p) = &tg.health_check_path {
                    hc.insert("path".to_string(), json!(p));
                }
                if let Some(p) = &tg.health_check_port {
                    hc.insert("port".to_string(), json!(p));
                }
                if let Some(p) = &tg.health_check_protocol {
                    hc.insert("protocol".to_string(), json!(p));
                }
                if let Some(i) = tg.health_check_interval {
                    hc.insert("interval".to_string(), json!(i));
                }
                if let Some(t) = tg.health_check_timeout {
                    hc.insert("timeout".to_string(), json!(t));
                }
                if let Some(h) = tg.healthy_threshold {
                    hc.insert("healthy_threshold".to_string(), json!(h));
                }
                if let Some(u) = tg.unhealthy_threshold {
                    hc.insert("unhealthy_threshold".to_string(), json!(u));
                }
                if !hc.is_empty() {
                    j["health_check"] = Value::Object(hc);
                }
                if !tg.tags.is_empty() {
                    let tags: Vec<Value> = tg
                        .tags
                        .iter()
                        .map(|(k, v)| json!({"key": k, "value": v}))
                        .collect();
                    j["tags"] = json!(tags);
                }
                j
            })
            .collect();
        info!(count = result.len(), "Target Groups取得完了");
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::elb_client_trait::mock::MockELBClient;
    use crate::infra::aws::elb_client_trait::{
        ALBInfo, ListenerActionInfo, ListenerInfo, TargetGroupInfo,
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
    async fn test_scan_load_balancers_and_listeners() {
        let mut mock = MockELBClient::new();
        mock.expect_describe_load_balancers().returning(|| {
            Ok(vec![ALBInfo {
                load_balancer_arn:
                    "arn:aws:elasticloadbalancing:ap-northeast-1:123:loadbalancer/app/my-alb/abc"
                        .to_string(),
                name: "my-alb".to_string(),
                dns_name: Some("my-alb-123.ap-northeast-1.elb.amazonaws.com".to_string()),
                scheme: Some("internet-facing".to_string()),
                lb_type: Some("application".to_string()),
                vpc_id: Some("vpc-123".to_string()),
                subnets: vec!["subnet-1".to_string(), "subnet-2".to_string()],
                security_groups: vec!["sg-123".to_string()],
                tags: HashMap::from([("env".to_string(), "prod".to_string())]),
            }])
        });
        mock.expect_describe_listeners().returning(|_| {
            Ok(vec![ListenerInfo {
                listener_arn: "arn:aws:elasticloadbalancing:listener/abc".to_string(),
                load_balancer_arn: "arn:aws:elasticloadbalancing:loadbalancer/abc".to_string(),
                port: Some(443),
                protocol: Some("HTTPS".to_string()),
                default_actions: vec![ListenerActionInfo {
                    action_type: "forward".to_string(),
                    target_group_arn: Some("arn:aws:tg/my-tg".to_string()),
                }],
            }])
        });

        let config = make_test_config(HashMap::from([("load_balancers".to_string(), true)]));
        let scanner = AwsELBScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let lbs = results.get("load_balancers").unwrap().as_array().unwrap();
        assert_eq!(lbs.len(), 1);
        assert_eq!(lbs[0]["name"], "my-alb");

        let listeners = results.get("lb_listeners").unwrap().as_array().unwrap();
        assert_eq!(listeners.len(), 1);
        assert_eq!(listeners[0]["port"], 443);
    }

    #[tokio::test]
    async fn test_scan_target_groups() {
        let mut mock = MockELBClient::new();
        mock.expect_describe_target_groups().returning(|| {
            Ok(vec![TargetGroupInfo {
                target_group_arn: "arn:aws:tg/my-tg".to_string(),
                name: "my-target-group".to_string(),
                port: Some(8080),
                protocol: Some("HTTP".to_string()),
                vpc_id: Some("vpc-123".to_string()),
                target_type: Some("instance".to_string()),
                health_check_path: Some("/health".to_string()),
                health_check_port: Some("traffic-port".to_string()),
                health_check_protocol: Some("HTTP".to_string()),
                health_check_interval: Some(30),
                health_check_timeout: Some(5),
                healthy_threshold: Some(3),
                unhealthy_threshold: Some(2),
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(HashMap::from([("lb_target_groups".to_string(), true)]));
        let scanner = AwsELBScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let tgs = results.get("lb_target_groups").unwrap().as_array().unwrap();
        assert_eq!(tgs.len(), 1);
        assert_eq!(tgs[0]["name"], "my-target-group");
        assert!(tgs[0]["health_check"].is_object());
    }
}
