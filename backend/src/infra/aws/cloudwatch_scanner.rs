//! AWS CloudWatch/SNSスキャナー

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

use crate::infra::aws::cloudwatch_client_trait::CloudWatchClientOps;
use crate::infra::aws::sns_client_trait::SNSClientOps;
use crate::models::ScanConfig;

/// AWS CloudWatch/SNSスキャナー
pub struct AwsCloudWatchSNSScanner<CW: CloudWatchClientOps, SN: SNSClientOps> {
    config: ScanConfig,
    cw_client: Arc<CW>,
    sns_client: Arc<SN>,
}

impl<CW: CloudWatchClientOps, SN: SNSClientOps> AwsCloudWatchSNSScanner<CW, SN> {
    pub fn new(config: ScanConfig, cw_client: Arc<CW>, sns_client: Arc<SN>) -> Self {
        Self {
            config,
            cw_client,
            sns_client,
        }
    }

    #[cfg(test)]
    pub fn new_with_clients(config: ScanConfig, cw_client: CW, sns_client: SN) -> Self {
        Self {
            config,
            cw_client: Arc::new(cw_client),
            sns_client: Arc::new(sns_client),
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

        // CloudWatch Alarms
        if scan_targets
            .get("cloudwatch_alarms")
            .copied()
            .unwrap_or(false)
        {
            debug!("CloudWatch Alarmsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "CloudWatch Alarmsのスキャン中...".to_string(),
            );
            let alarms = self.scan_alarms().await?;
            let count = alarms.len();
            results.insert("cloudwatch_alarms".to_string(), Value::Array(alarms));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("CloudWatch Alarmsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("cloudwatch_alarms".to_string(), Value::Array(Vec::new()));
        }

        // SNS Topics
        if scan_targets.get("sns_topics").copied().unwrap_or(false) {
            debug!("SNS Topicsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "SNS Topicsのスキャン中...".to_string(),
            );
            let topics = self.scan_topics().await?;
            let count = topics.len();
            results.insert("sns_topics".to_string(), Value::Array(topics));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("SNS Topicsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("sns_topics".to_string(), Value::Array(Vec::new()));
        }

        // SNS Subscriptions
        if scan_targets
            .get("sns_subscriptions")
            .copied()
            .unwrap_or(false)
        {
            debug!("SNS Subscriptionsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "SNS Subscriptionsのスキャン中...".to_string(),
            );
            let subs = self.scan_subscriptions().await?;
            let count = subs.len();
            results.insert("sns_subscriptions".to_string(), Value::Array(subs));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("SNS Subscriptionsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("sns_subscriptions".to_string(), Value::Array(Vec::new()));
        }

        Ok(())
    }

    async fn scan_alarms(&self) -> Result<Vec<Value>> {
        info!("CloudWatch Alarms一覧を取得中");
        let alarms = self.cw_client.describe_alarms().await?;
        let result: Vec<Value> = alarms
            .into_iter()
            .map(|a| {
                let mut j = json!({"alarm_name": a.alarm_name});
                if let Some(arn) = &a.alarm_arn {
                    j["alarm_arn"] = json!(arn);
                }
                if let Some(m) = &a.metric_name {
                    j["metric_name"] = json!(m);
                }
                if let Some(ns) = &a.namespace {
                    j["namespace"] = json!(ns);
                }
                if let Some(s) = &a.statistic {
                    j["statistic"] = json!(s);
                }
                if let Some(p) = a.period {
                    j["period"] = json!(p);
                }
                if let Some(ep) = a.evaluation_periods {
                    j["evaluation_periods"] = json!(ep);
                }
                if let Some(t) = a.threshold {
                    j["threshold"] = json!(t);
                }
                if let Some(co) = &a.comparison_operator {
                    j["comparison_operator"] = json!(co);
                }
                if !a.alarm_actions.is_empty() {
                    j["alarm_actions"] = json!(a.alarm_actions);
                }
                if !a.ok_actions.is_empty() {
                    j["ok_actions"] = json!(a.ok_actions);
                }
                if let Some(d) = &a.alarm_description {
                    j["alarm_description"] = json!(d);
                }
                if let Some(sv) = &a.state_value {
                    j["state_value"] = json!(sv);
                }
                j
            })
            .collect();
        info!(count = result.len(), "CloudWatch Alarms取得完了");
        Ok(result)
    }

    async fn scan_topics(&self) -> Result<Vec<Value>> {
        info!("SNS Topics一覧を取得中");
        let topics = self.sns_client.list_topics().await?;
        let result: Vec<Value> = topics
            .into_iter()
            .map(|t| {
                let mut j = json!({"topic_arn": t.topic_arn});
                if let Some(dn) = &t.display_name {
                    j["display_name"] = json!(dn);
                }
                if !t.tags.is_empty() {
                    let tags: Vec<Value> = t
                        .tags
                        .iter()
                        .map(|(k, v)| json!({"key": k, "value": v}))
                        .collect();
                    j["tags"] = json!(tags);
                }
                j
            })
            .collect();
        info!(count = result.len(), "SNS Topics取得完了");
        Ok(result)
    }

    async fn scan_subscriptions(&self) -> Result<Vec<Value>> {
        info!("SNS Subscriptions一覧を取得中");
        let subs = self.sns_client.list_subscriptions().await?;
        let result: Vec<Value> = subs
            .into_iter()
            .filter(|s| s.subscription_arn != "PendingConfirmation")
            .map(|s| {
                let mut j = json!({
                    "subscription_arn": s.subscription_arn,
                    "topic_arn": s.topic_arn,
                });
                if let Some(p) = &s.protocol {
                    j["protocol"] = json!(p);
                }
                if let Some(e) = &s.endpoint {
                    j["endpoint"] = json!(e);
                }
                j
            })
            .collect();
        info!(count = result.len(), "SNS Subscriptions取得完了");
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::cloudwatch_client_trait::mock::MockCloudWatchClient;
    use crate::infra::aws::cloudwatch_client_trait::CloudWatchAlarmInfo;
    use crate::infra::aws::sns_client_trait::mock::MockSNSClient;
    use crate::infra::aws::sns_client_trait::SNSTopicInfo;
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
    async fn test_scan_cloudwatch_alarms() {
        let mut cw_mock = MockCloudWatchClient::new();
        cw_mock.expect_describe_alarms().returning(|| {
            Ok(vec![CloudWatchAlarmInfo {
                alarm_name: "high-cpu".to_string(),
                alarm_arn: Some("arn:aws:cloudwatch:ap-northeast-1:123:alarm:high-cpu".to_string()),
                metric_name: Some("CPUUtilization".to_string()),
                namespace: Some("AWS/EC2".to_string()),
                statistic: Some("Average".to_string()),
                period: Some(300),
                evaluation_periods: Some(2),
                threshold: Some(80.0),
                comparison_operator: Some("GreaterThanThreshold".to_string()),
                alarm_actions: vec!["arn:aws:sns:ap-northeast-1:123:alerts".to_string()],
                ok_actions: vec![],
                insufficient_data_actions: vec![],
                alarm_description: Some("High CPU alarm".to_string()),
                state_value: Some("OK".to_string()),
            }])
        });
        let mut sns_mock = MockSNSClient::new();
        sns_mock.expect_list_topics().never();
        sns_mock.expect_list_subscriptions().never();

        let config = make_test_config(HashMap::from([("cloudwatch_alarms".to_string(), true)]));
        let scanner = AwsCloudWatchSNSScanner::new_with_clients(config, cw_mock, sns_mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let alarms = results
            .get("cloudwatch_alarms")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(alarms.len(), 1);
        assert_eq!(alarms[0]["alarm_name"], "high-cpu");
        assert_eq!(alarms[0]["threshold"], 80.0);
    }

    #[tokio::test]
    async fn test_scan_sns_topics() {
        let mut cw_mock = MockCloudWatchClient::new();
        cw_mock.expect_describe_alarms().never();
        let mut sns_mock = MockSNSClient::new();
        sns_mock.expect_list_topics().returning(|| {
            Ok(vec![SNSTopicInfo {
                topic_arn: "arn:aws:sns:ap-northeast-1:123:alerts".to_string(),
                display_name: Some("Alerts".to_string()),
                tags: HashMap::from([("env".to_string(), "prod".to_string())]),
            }])
        });
        sns_mock.expect_list_subscriptions().never();

        let config = make_test_config(HashMap::from([("sns_topics".to_string(), true)]));
        let scanner = AwsCloudWatchSNSScanner::new_with_clients(config, cw_mock, sns_mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let topics = results.get("sns_topics").unwrap().as_array().unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0]["display_name"], "Alerts");
    }

    #[tokio::test]
    async fn test_scan_sns_subscriptions() {
        use crate::infra::aws::sns_client_trait::SNSSubscriptionInfo;

        let mut cw_mock = MockCloudWatchClient::new();
        cw_mock.expect_describe_alarms().never();
        let mut sns_mock = MockSNSClient::new();
        sns_mock.expect_list_topics().never();
        sns_mock.expect_list_subscriptions().returning(|| {
            Ok(vec![
                SNSSubscriptionInfo {
                    subscription_arn: "arn:aws:sns:ap-northeast-1:123:alerts:sub-id-1".to_string(),
                    topic_arn: "arn:aws:sns:ap-northeast-1:123:alerts".to_string(),
                    protocol: Some("email".to_string()),
                    endpoint: Some("test@example.com".to_string()),
                },
                // PendingConfirmation のものはフィルタされる
                SNSSubscriptionInfo {
                    subscription_arn: "PendingConfirmation".to_string(),
                    topic_arn: "arn:aws:sns:ap-northeast-1:123:alerts".to_string(),
                    protocol: Some("email".to_string()),
                    endpoint: Some("pending@example.com".to_string()),
                },
            ])
        });

        let config = make_test_config(HashMap::from([("sns_subscriptions".to_string(), true)]));
        let scanner = AwsCloudWatchSNSScanner::new_with_clients(config, cw_mock, sns_mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let subs = results
            .get("sns_subscriptions")
            .unwrap()
            .as_array()
            .unwrap();
        // PendingConfirmation はフィルタされるため1件のみ
        assert_eq!(subs.len(), 1);
        assert_eq!(
            subs[0]["subscription_arn"],
            "arn:aws:sns:ap-northeast-1:123:alerts:sub-id-1"
        );
        assert_eq!(subs[0]["protocol"], "email");
        assert_eq!(subs[0]["endpoint"], "test@example.com");
    }

    #[tokio::test]
    async fn test_scan_sns_topics_with_no_display_name() {
        let mut cw_mock = MockCloudWatchClient::new();
        cw_mock.expect_describe_alarms().never();
        let mut sns_mock = MockSNSClient::new();
        sns_mock.expect_list_topics().returning(|| {
            Ok(vec![SNSTopicInfo {
                topic_arn: "arn:aws:sns:ap-northeast-1:123:no-name".to_string(),
                display_name: None,
                tags: HashMap::new(),
            }])
        });
        sns_mock.expect_list_subscriptions().never();

        let config = make_test_config(HashMap::from([("sns_topics".to_string(), true)]));
        let scanner = AwsCloudWatchSNSScanner::new_with_clients(config, cw_mock, sns_mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let topics = results.get("sns_topics").unwrap().as_array().unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(
            topics[0]["topic_arn"],
            "arn:aws:sns:ap-northeast-1:123:no-name"
        );
        // display_name が None の場合はフィールドなし
        assert!(topics[0].get("display_name").is_none());
    }
}
