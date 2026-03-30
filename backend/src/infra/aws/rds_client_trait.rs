//! RDSクライアント操作の抽象化トレイト

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// RDSインスタンス情報
#[derive(Debug, Clone)]
pub struct RdsInstanceInfo {
    pub db_instance_identifier: String,
    pub db_instance_class: Option<String>,
    pub engine: Option<String>,
    pub engine_version: Option<String>,
    pub status: Option<String>,
    pub allocated_storage: Option<i32>,
    pub storage_type: Option<String>,
    pub multi_az: bool,
    pub publicly_accessible: bool,
    pub vpc_id: Option<String>,
    pub db_subnet_group_name: Option<String>,
    pub availability_zone: Option<String>,
    pub endpoint: Option<String>,
    pub port: Option<i32>,
    pub master_username: Option<String>,
    pub security_groups: Vec<String>,
    pub parameter_group_name: Option<String>,
    pub backup_retention_period: Option<i32>,
    pub storage_encrypted: bool,
    pub tags: HashMap<String, String>,
}

/// RDSサブネットグループ情報
#[derive(Debug, Clone)]
pub struct RdsSubnetGroupInfo {
    pub db_subnet_group_name: String,
    pub description: Option<String>,
    pub vpc_id: Option<String>,
    pub subnet_ids: Vec<String>,
    pub status: Option<String>,
    pub tags: HashMap<String, String>,
}

/// RDSパラメータグループ情報
#[derive(Debug, Clone)]
pub struct RdsParameterGroupInfo {
    pub db_parameter_group_name: String,
    pub family: Option<String>,
    pub description: Option<String>,
    pub tags: HashMap<String, String>,
}

/// RDSパラメータ情報
#[derive(Debug, Clone)]
pub struct RdsParameterInfo {
    pub name: String,
    pub value: String,
    pub apply_method: Option<String>,
}

/// RDSクライアント操作を抽象化するトレイト
#[async_trait]
pub trait RdsClientOps: Send + Sync {
    /// DBインスタンス一覧を取得
    async fn describe_db_instances(&self) -> Result<Vec<RdsInstanceInfo>>;

    /// DBサブネットグループ一覧を取得
    async fn describe_db_subnet_groups(&self) -> Result<Vec<RdsSubnetGroupInfo>>;

    /// DBパラメータグループ一覧を取得
    async fn describe_db_parameter_groups(&self) -> Result<Vec<RdsParameterGroupInfo>>;

    /// パラメータグループのパラメータを取得（変更されたもののみ）
    async fn describe_db_parameters(
        &self,
        parameter_group_name: &str,
    ) -> Result<Vec<RdsParameterInfo>>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub RdsClient {}

        #[async_trait]
        impl RdsClientOps for RdsClient {
            async fn describe_db_instances(&self) -> Result<Vec<RdsInstanceInfo>>;
            async fn describe_db_subnet_groups(&self) -> Result<Vec<RdsSubnetGroupInfo>>;
            async fn describe_db_parameter_groups(&self) -> Result<Vec<RdsParameterGroupInfo>>;
            async fn describe_db_parameters(&self, parameter_group_name: &str) -> Result<Vec<RdsParameterInfo>>;
        }
    }
}
