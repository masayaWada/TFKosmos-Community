//! AWS SDK ELBv2クライアントの本番実装

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_elasticloadbalancingv2::Client as ELBClient;
use std::collections::HashMap;

use super::elb_client_trait::{
    ALBInfo, ELBClientOps, ListenerActionInfo, ListenerInfo, TargetGroupInfo,
};

/// AWS SDK ELBv2クライアントをラップした本番実装
pub struct RealELBClient {
    client: ELBClient,
}

impl RealELBClient {
    pub fn new(client: ELBClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ELBClientOps for RealELBClient {
    async fn describe_load_balancers(&self) -> Result<Vec<ALBInfo>> {
        let mut lbs = Vec::new();
        let mut marker: Option<String> = None;

        loop {
            let mut req = self.client.describe_load_balancers();
            if let Some(ref m) = marker {
                req = req.marker(m.clone());
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to describe load balancers: {}", e))?;

            for lb in output.load_balancers() {
                let subnets: Vec<String> = lb
                    .availability_zones()
                    .iter()
                    .filter_map(|az| az.subnet_id().map(|s| s.to_string()))
                    .collect();

                // タグ取得
                let tags = if let Some(arn) = lb.load_balancer_arn() {
                    match self.client.describe_tags().resource_arns(arn).send().await {
                        Ok(t) => t
                            .tag_descriptions()
                            .first()
                            .map(|td| {
                                td.tags()
                                    .iter()
                                    .map(|tag| {
                                        (
                                            tag.key().unwrap_or_default().to_string(),
                                            tag.value().unwrap_or_default().to_string(),
                                        )
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        Err(_) => HashMap::new(),
                    }
                } else {
                    HashMap::new()
                };

                lbs.push(ALBInfo {
                    load_balancer_arn: lb.load_balancer_arn().unwrap_or_default().to_string(),
                    name: lb.load_balancer_name().unwrap_or_default().to_string(),
                    dns_name: lb.dns_name().map(|s| s.to_string()),
                    scheme: lb.scheme().map(|s| s.as_str().to_string()),
                    lb_type: lb.r#type().map(|t| t.as_str().to_string()),
                    vpc_id: lb.vpc_id().map(|s| s.to_string()),
                    subnets,
                    security_groups: lb.security_groups().iter().map(|s| s.to_string()).collect(),
                    tags,
                });
            }

            marker = output.next_marker().map(|s| s.to_string());
            if marker.is_none() {
                break;
            }
        }

        Ok(lbs)
    }

    async fn describe_listeners(&self, lb_arn: &str) -> Result<Vec<ListenerInfo>> {
        let output = self
            .client
            .describe_listeners()
            .load_balancer_arn(lb_arn)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe listeners: {}", e))?;

        let listeners = output
            .listeners()
            .iter()
            .map(|l| {
                let default_actions = l
                    .default_actions()
                    .iter()
                    .map(|a| ListenerActionInfo {
                        action_type: a
                            .r#type()
                            .map(|t| t.as_str().to_string())
                            .unwrap_or_default(),
                        target_group_arn: a.target_group_arn().map(|s| s.to_string()),
                    })
                    .collect();

                ListenerInfo {
                    listener_arn: l.listener_arn().unwrap_or_default().to_string(),
                    load_balancer_arn: lb_arn.to_string(),
                    port: l.port(),
                    protocol: l.protocol().map(|p| p.as_str().to_string()),
                    default_actions,
                }
            })
            .collect();

        Ok(listeners)
    }

    async fn describe_target_groups(&self) -> Result<Vec<TargetGroupInfo>> {
        let mut tgs = Vec::new();
        let mut marker: Option<String> = None;

        loop {
            let mut req = self.client.describe_target_groups();
            if let Some(ref m) = marker {
                req = req.marker(m.clone());
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to describe target groups: {}", e))?;

            for tg in output.target_groups() {
                tgs.push(TargetGroupInfo {
                    target_group_arn: tg.target_group_arn().unwrap_or_default().to_string(),
                    name: tg.target_group_name().unwrap_or_default().to_string(),
                    port: tg.port(),
                    protocol: tg.protocol().map(|p| p.as_str().to_string()),
                    vpc_id: tg.vpc_id().map(|s| s.to_string()),
                    target_type: tg.target_type().map(|t| t.as_str().to_string()),
                    health_check_path: tg.health_check_path().map(|s| s.to_string()),
                    health_check_port: tg.health_check_port().map(|s| s.to_string()),
                    health_check_protocol: tg
                        .health_check_protocol()
                        .map(|p| p.as_str().to_string()),
                    health_check_interval: tg.health_check_interval_seconds(),
                    health_check_timeout: tg.health_check_timeout_seconds(),
                    healthy_threshold: tg.healthy_threshold_count(),
                    unhealthy_threshold: tg.unhealthy_threshold_count(),
                    tags: HashMap::new(),
                });
            }

            marker = output.next_marker().map(|s| s.to_string());
            if marker.is_none() {
                break;
            }
        }

        Ok(tgs)
    }
}
