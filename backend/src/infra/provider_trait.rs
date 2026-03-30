//! クラウドプロバイダー抽象化トレイト
//!
//! 新規プロバイダー（GCP等）の追加を容易にするための共通インターフェース。
//! 各プロバイダーはこのトレイトを実装し、スキャンとテンプレート定義を提供する。

use anyhow::Result;
use async_trait::async_trait;

use crate::models::ScanConfig;

/// Terraform テンプレートとリソースタイプのマッピング
pub struct ResourceTemplate {
    pub resource_type: &'static str,
    pub template_path: &'static str,
    #[allow(dead_code)]
    pub provider: &'static str,
}

/// クラウドプロバイダーのスキャンとテンプレート提供を抽象化するトレイト
#[async_trait]
pub trait CloudProviderScanner: Send + Sync {
    /// プロバイダー識別子を返す（"aws", "azure", "gcp" 等）
    fn provider_name(&self) -> &'static str;

    /// このプロバイダーの Terraform テンプレート一覧を返す
    fn get_templates(&self) -> Vec<ResourceTemplate>;

    /// リソーススキャンを実行する
    async fn scan(
        &self,
        config: ScanConfig,
        callback: Box<dyn Fn(u32, String) + Send + Sync>,
    ) -> Result<serde_json::Value>;
}
