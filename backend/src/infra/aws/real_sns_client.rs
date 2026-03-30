//! AWS SDK SNSクライアントの本番実装

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_sns::Client as SnsClient;
use std::collections::HashMap;

use super::sns_client_trait::{SNSClientOps, SNSSubscriptionInfo, SNSTopicInfo};

pub struct RealSNSClient {
    client: SnsClient,
}

impl RealSNSClient {
    pub fn new(client: SnsClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SNSClientOps for RealSNSClient {
    async fn list_topics(&self) -> Result<Vec<SNSTopicInfo>> {
        let mut topics = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.list_topics();
            if let Some(ref token) = next_token {
                req = req.next_token(token.clone());
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to list SNS topics: {}", e))?;

            for topic in output.topics() {
                if let Some(arn) = topic.topic_arn() {
                    // タグ取得
                    let tags = match self
                        .client
                        .list_tags_for_resource()
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
                    };

                    // 属性取得（DisplayName）
                    let display_name = match self
                        .client
                        .get_topic_attributes()
                        .topic_arn(arn)
                        .send()
                        .await
                    {
                        Ok(attrs) => attrs
                            .attributes()
                            .and_then(|a| a.get("DisplayName"))
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string()),
                        Err(_) => None,
                    };

                    topics.push(SNSTopicInfo {
                        topic_arn: arn.to_string(),
                        display_name,
                        tags,
                    });
                }
            }

            next_token = output.next_token().map(|s| s.to_string());
            if next_token.is_none() {
                break;
            }
        }

        Ok(topics)
    }

    async fn list_subscriptions(&self) -> Result<Vec<SNSSubscriptionInfo>> {
        let mut subscriptions = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.list_subscriptions();
            if let Some(ref token) = next_token {
                req = req.next_token(token.clone());
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to list SNS subscriptions: {}", e))?;

            for sub in output.subscriptions() {
                subscriptions.push(SNSSubscriptionInfo {
                    subscription_arn: sub.subscription_arn().unwrap_or_default().to_string(),
                    topic_arn: sub.topic_arn().unwrap_or_default().to_string(),
                    protocol: sub.protocol().map(|s| s.to_string()),
                    endpoint: sub.endpoint().map(|s| s.to_string()),
                });
            }

            next_token = output.next_token().map(|s| s.to_string());
            if next_token.is_none() {
                break;
            }
        }

        Ok(subscriptions)
    }
}
