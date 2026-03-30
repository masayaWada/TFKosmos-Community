//! Lambdaクライアント操作の抽象化トレイト

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// Lambda関数情報
#[derive(Debug, Clone)]
pub struct LambdaFunctionInfo {
    pub function_name: String,
    pub function_arn: Option<String>,
    pub runtime: Option<String>,
    pub handler: Option<String>,
    pub role: Option<String>,
    pub description: Option<String>,
    pub memory_size: Option<i32>,
    pub timeout: Option<i32>,
    pub code_size: Option<i64>,
    pub last_modified: Option<String>,
    pub environment: HashMap<String, String>,
    pub vpc_subnet_ids: Vec<String>,
    pub vpc_security_group_ids: Vec<String>,
    pub layers: Vec<String>,
    pub tags: HashMap<String, String>,
}

/// Lambdaレイヤー情報
#[derive(Debug, Clone)]
pub struct LambdaLayerInfo {
    pub layer_name: String,
    pub layer_arn: Option<String>,
    pub latest_version: Option<i64>,
    pub description: Option<String>,
    pub compatible_runtimes: Vec<String>,
}

/// Lambdaクライアント操作を抽象化するトレイト
#[async_trait]
pub trait LambdaClientOps: Send + Sync {
    /// Lambda関数一覧を取得
    async fn list_functions(&self) -> Result<Vec<LambdaFunctionInfo>>;

    /// Lambdaレイヤー一覧を取得
    async fn list_layers(&self) -> Result<Vec<LambdaLayerInfo>>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub LambdaClient {}

        #[async_trait]
        impl LambdaClientOps for LambdaClient {
            async fn list_functions(&self) -> Result<Vec<LambdaFunctionInfo>>;
            async fn list_layers(&self) -> Result<Vec<LambdaLayerInfo>>;
        }
    }
}
