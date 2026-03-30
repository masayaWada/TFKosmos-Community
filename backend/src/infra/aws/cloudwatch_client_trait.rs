//! CloudWatchクライアント操作の抽象化トレイト

use anyhow::Result;
use async_trait::async_trait;

/// CloudWatchアラーム情報
#[derive(Debug, Clone)]
pub struct CloudWatchAlarmInfo {
    pub alarm_name: String,
    pub alarm_arn: Option<String>,
    pub metric_name: Option<String>,
    pub namespace: Option<String>,
    pub statistic: Option<String>,
    pub period: Option<i32>,
    pub evaluation_periods: Option<i32>,
    pub threshold: Option<f64>,
    pub comparison_operator: Option<String>,
    pub alarm_actions: Vec<String>,
    pub ok_actions: Vec<String>,
    #[allow(dead_code)]
    pub insufficient_data_actions: Vec<String>,
    pub alarm_description: Option<String>,
    pub state_value: Option<String>,
}

/// CloudWatchクライアント操作を抽象化するトレイト
#[async_trait]
pub trait CloudWatchClientOps: Send + Sync {
    /// アラーム一覧を取得
    async fn describe_alarms(&self) -> Result<Vec<CloudWatchAlarmInfo>>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub CloudWatchClient {}

        #[async_trait]
        impl CloudWatchClientOps for CloudWatchClient {
            async fn describe_alarms(&self) -> Result<Vec<CloudWatchAlarmInfo>>;
        }
    }
}
