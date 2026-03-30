//! Azureクライアント操作の抽象化トレイト
//!
//! このモジュールは、Azure CLI および Azure REST API 操作を抽象化し、
//! テスト時にモック実装を注入できるようにします。

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde_json::Value;

/// Azureクライアント操作を抽象化するトレイト
///
/// このトレイトを実装することで、本番用のAzure CLI/APIクライアントと
/// テスト用のモッククライアントを切り替えることができます。
#[async_trait]
pub trait AzureClientOps: Send + Sync {
    /// Azure CLIコマンドを実行してJSONを取得
    async fn execute_az_command(&self, args: Vec<String>) -> Result<Value>;

    /// 認証トークンを取得
    async fn get_auth_token(&self, scope: &str) -> Option<String>;

    /// HTTPクライアントを取得
    fn get_http_client(&self) -> Option<HttpClient>;

    /// Principal IDから表示名を取得
    async fn get_principal_display_name(
        &self,
        principal_id: &str,
        principal_type: Option<String>,
        token: &str,
    ) -> Option<String>;

    /// Role Definition IDから表示名を取得
    async fn get_role_display_name(
        &self,
        role_definition_id: &str,
        subscription_id: Option<String>,
        token: &str,
    ) -> Option<String>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub AzureClient {}

        #[async_trait]
        impl AzureClientOps for AzureClient {
            async fn execute_az_command(&self, args: Vec<String>) -> Result<Value>;
            async fn get_auth_token(&self, scope: &str) -> Option<String>;
            fn get_http_client(&self) -> Option<HttpClient>;
            async fn get_principal_display_name(
                &self,
                principal_id: &str,
                principal_type: Option<String>,
                token: &str,
            ) -> Option<String>;
            async fn get_role_display_name(
                &self,
                role_definition_id: &str,
                subscription_id: Option<String>,
                token: &str,
            ) -> Option<String>;
        }
    }
}
