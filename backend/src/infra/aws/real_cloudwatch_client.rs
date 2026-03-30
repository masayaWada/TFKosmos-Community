//! AWS SDK CloudWatchクライアントの本番実装

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_cloudwatch::Client as CloudWatchClient;

use super::cloudwatch_client_trait::{CloudWatchAlarmInfo, CloudWatchClientOps};

pub struct RealCloudWatchClient {
    client: CloudWatchClient,
}

impl RealCloudWatchClient {
    pub fn new(client: CloudWatchClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CloudWatchClientOps for RealCloudWatchClient {
    async fn describe_alarms(&self) -> Result<Vec<CloudWatchAlarmInfo>> {
        let mut alarms = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.describe_alarms();
            if let Some(ref token) = next_token {
                req = req.next_token(token.clone());
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to describe alarms: {}", e))?;

            for alarm in output.metric_alarms() {
                alarms.push(CloudWatchAlarmInfo {
                    alarm_name: alarm.alarm_name().unwrap_or_default().to_string(),
                    alarm_arn: alarm.alarm_arn().map(|s| s.to_string()),
                    metric_name: alarm.metric_name().map(|s| s.to_string()),
                    namespace: alarm.namespace().map(|s| s.to_string()),
                    statistic: alarm.statistic().map(|s| s.as_str().to_string()),
                    period: alarm.period(),
                    evaluation_periods: alarm.evaluation_periods(),
                    threshold: alarm.threshold(),
                    comparison_operator: alarm
                        .comparison_operator()
                        .map(|c| c.as_str().to_string()),
                    alarm_actions: alarm
                        .alarm_actions()
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    ok_actions: alarm.ok_actions().iter().map(|s| s.to_string()).collect(),
                    insufficient_data_actions: alarm
                        .insufficient_data_actions()
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    alarm_description: alarm.alarm_description().map(|s| s.to_string()),
                    state_value: alarm.state_value().map(|s| s.as_str().to_string()),
                });
            }

            next_token = output.next_token().map(|s| s.to_string());
            if next_token.is_none() {
                break;
            }
        }

        Ok(alarms)
    }
}
