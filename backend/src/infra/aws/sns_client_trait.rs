//! SNSクライアント操作の抽象化トレイト

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// SNSトピック情報
#[derive(Debug, Clone)]
pub struct SNSTopicInfo {
    pub topic_arn: String,
    pub display_name: Option<String>,
    pub tags: HashMap<String, String>,
}

/// SNSサブスクリプション情報
#[derive(Debug, Clone)]
pub struct SNSSubscriptionInfo {
    pub subscription_arn: String,
    pub topic_arn: String,
    pub protocol: Option<String>,
    pub endpoint: Option<String>,
}

/// SNSクライアント操作を抽象化するトレイト
#[async_trait]
pub trait SNSClientOps: Send + Sync {
    /// トピック一覧を取得
    async fn list_topics(&self) -> Result<Vec<SNSTopicInfo>>;

    /// サブスクリプション一覧を取得
    async fn list_subscriptions(&self) -> Result<Vec<SNSSubscriptionInfo>>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub SNSClient {}

        #[async_trait]
        impl SNSClientOps for SNSClient {
            async fn list_topics(&self) -> Result<Vec<SNSTopicInfo>>;
            async fn list_subscriptions(&self) -> Result<Vec<SNSSubscriptionInfo>>;
        }
    }
}
