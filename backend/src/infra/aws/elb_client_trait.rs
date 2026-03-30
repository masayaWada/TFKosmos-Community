//! ELBv2クライアント操作の抽象化トレイト

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// ALB情報
#[derive(Debug, Clone)]
pub struct ALBInfo {
    pub load_balancer_arn: String,
    pub name: String,
    pub dns_name: Option<String>,
    pub scheme: Option<String>,
    pub lb_type: Option<String>,
    pub vpc_id: Option<String>,
    pub subnets: Vec<String>,
    pub security_groups: Vec<String>,
    pub tags: HashMap<String, String>,
}

/// リスナー情報
#[derive(Debug, Clone)]
pub struct ListenerInfo {
    pub listener_arn: String,
    pub load_balancer_arn: String,
    pub port: Option<i32>,
    pub protocol: Option<String>,
    pub default_actions: Vec<ListenerActionInfo>,
}

/// リスナーアクション情報
#[derive(Debug, Clone)]
pub struct ListenerActionInfo {
    pub action_type: String,
    pub target_group_arn: Option<String>,
}

/// ターゲットグループ情報
#[derive(Debug, Clone)]
pub struct TargetGroupInfo {
    pub target_group_arn: String,
    pub name: String,
    pub port: Option<i32>,
    pub protocol: Option<String>,
    pub vpc_id: Option<String>,
    pub target_type: Option<String>,
    pub health_check_path: Option<String>,
    pub health_check_port: Option<String>,
    pub health_check_protocol: Option<String>,
    pub health_check_interval: Option<i32>,
    pub health_check_timeout: Option<i32>,
    pub healthy_threshold: Option<i32>,
    pub unhealthy_threshold: Option<i32>,
    pub tags: HashMap<String, String>,
}

/// ELBv2クライアント操作を抽象化するトレイト
#[async_trait]
pub trait ELBClientOps: Send + Sync {
    /// ロードバランサー一覧を取得
    async fn describe_load_balancers(&self) -> Result<Vec<ALBInfo>>;

    /// 指定LBのリスナー一覧を取得
    async fn describe_listeners(&self, lb_arn: &str) -> Result<Vec<ListenerInfo>>;

    /// ターゲットグループ一覧を取得
    async fn describe_target_groups(&self) -> Result<Vec<TargetGroupInfo>>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub ELBClient {}

        #[async_trait]
        impl ELBClientOps for ELBClient {
            async fn describe_load_balancers(&self) -> Result<Vec<ALBInfo>>;
            async fn describe_listeners(&self, lb_arn: &str) -> Result<Vec<ListenerInfo>>;
            async fn describe_target_groups(&self) -> Result<Vec<TargetGroupInfo>>;
        }
    }
}
