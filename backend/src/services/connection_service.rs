//! クラウド接続テストサービス
//!
//! AWS・Azureへの接続確認と、Azureサブスクリプション/リソースグループの
//! 一覧取得を担当するサービス層。依存性注入（DI）パターンにより
//! テスト時にモックに差し替え可能。

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::infra::aws::client_factory::AwsClientFactory;
use crate::infra::azure::client_factory::AzureClientFactory;
use crate::models::{AzureResourceGroup, AzureSubscription, ConnectionTestResponse};

/// クラウド接続テストのトレイト（DIとモック化のため）
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CloudConnectionTester: Send + Sync {
    async fn test_aws_connection(
        &self,
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<ConnectionTestResponse>;

    async fn test_azure_connection(
        &self,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<ConnectionTestResponse>;

    async fn list_azure_subscriptions(
        &self,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<Vec<AzureSubscription>>;

    async fn list_azure_resource_groups(
        &self,
        subscription_id: String,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<Vec<AzureResourceGroup>>;
}

/// 本番用クラウド接続テスター（AWS/Azure SDKを使用）
pub struct RealCloudConnectionTester;

#[async_trait]
impl CloudConnectionTester for RealCloudConnectionTester {
    async fn test_aws_connection(
        &self,
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<ConnectionTestResponse> {
        AwsClientFactory::test_connection(profile, assume_role_arn, assume_role_session_name).await
    }

    async fn test_azure_connection(
        &self,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<ConnectionTestResponse> {
        AzureClientFactory::test_connection(auth_method, tenant_id, service_principal_config).await
    }

    async fn list_azure_subscriptions(
        &self,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<Vec<AzureSubscription>> {
        AzureClientFactory::list_subscriptions(auth_method, tenant_id, service_principal_config)
            .await
    }

    async fn list_azure_resource_groups(
        &self,
        subscription_id: String,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<Vec<AzureResourceGroup>> {
        AzureClientFactory::list_resource_groups(
            subscription_id,
            auth_method,
            tenant_id,
            service_principal_config,
        )
        .await
    }
}

/// クラウド接続サービス（DIを通じてテスター実装を注入可能）
///
/// `CloudConnectionTester` トレイト実装を受け取り、AWS/Azure への接続確認と
/// Azureリソース一覧取得を提供する。本番では `RealCloudConnectionTester`、
/// テストでは `MockCloudConnectionTester` を注入する。
pub struct ConnectionService {
    inner: Arc<dyn CloudConnectionTester>,
}

impl ConnectionService {
    /// 新しい `ConnectionService` を生成する
    ///
    /// # Arguments
    /// * `inner` - クラウド接続テスターの実装（本番 or モック）
    pub fn new(inner: Arc<dyn CloudConnectionTester>) -> Self {
        Self { inner }
    }

    /// AWS接続テストを実行する
    ///
    /// # Arguments
    /// * `profile` - AWS名前付きプロファイル名（省略時はデフォルト認証チェーン）
    /// * `assume_role_arn` - AssumeRole対象のARN
    /// * `assume_role_session_name` - AssumeRoleセッション名
    pub async fn test_aws_connection(
        &self,
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<ConnectionTestResponse> {
        self.inner
            .test_aws_connection(profile, assume_role_arn, assume_role_session_name)
            .await
    }

    /// Azure接続テストを実行する
    ///
    /// # Arguments
    /// * `auth_method` - 認証方式（`"default"` / `"service_principal"`）
    /// * `tenant_id` - Azure Active Directory テナントID
    /// * `service_principal_config` - サービスプリンシパル認証情報（`client_id`, `client_secret`）
    pub async fn test_azure_connection(
        &self,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<ConnectionTestResponse> {
        self.inner
            .test_azure_connection(auth_method, tenant_id, service_principal_config)
            .await
    }

    /// アクセス可能なAzureサブスクリプション一覧を取得する
    ///
    /// # Arguments
    /// * `auth_method` - 認証方式
    /// * `tenant_id` - テナントID
    /// * `service_principal_config` - サービスプリンシパル認証情報
    pub async fn list_azure_subscriptions(
        &self,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<Vec<AzureSubscription>> {
        self.inner
            .list_azure_subscriptions(auth_method, tenant_id, service_principal_config)
            .await
    }

    /// 指定サブスクリプション内のリソースグループ一覧を取得する
    ///
    /// # Arguments
    /// * `subscription_id` - AzureサブスクリプションID
    /// * `auth_method` - 認証方式
    /// * `tenant_id` - テナントID
    /// * `service_principal_config` - サービスプリンシパル認証情報
    pub async fn list_azure_resource_groups(
        &self,
        subscription_id: String,
        auth_method: Option<String>,
        tenant_id: Option<String>,
        service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<Vec<AzureResourceGroup>> {
        self.inner
            .list_azure_resource_groups(
                subscription_id,
                auth_method,
                tenant_id,
                service_principal_config,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_success_response() -> ConnectionTestResponse {
        ConnectionTestResponse {
            success: true,
            message: Some("Connected".to_string()),
            account_id: Some("123456789012".to_string()),
            user_arn: Some("arn:aws:iam::123456789012:user/test".to_string()),
            subscription_name: None,
        }
    }

    #[tokio::test]
    async fn test_aws_connection_success() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_aws_connection()
            .times(1)
            .returning(|_, _, _| Ok(make_success_response()));

        let service = ConnectionService::new(Arc::new(mock));
        let result = service.test_aws_connection(None, None, None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[tokio::test]
    async fn test_aws_connection_failure() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_aws_connection()
            .times(1)
            .returning(|_, _, _| Err(anyhow::anyhow!("Connection failed")));

        let service = ConnectionService::new(Arc::new(mock));
        let result = service.test_aws_connection(None, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_azure_connection_success() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_azure_connection()
            .times(1)
            .returning(|_, _, _| {
                Ok(ConnectionTestResponse {
                    success: true,
                    message: Some("Connected".to_string()),
                    account_id: None,
                    user_arn: None,
                    subscription_name: Some("Test Subscription".to_string()),
                })
            });

        let service = ConnectionService::new(Arc::new(mock));
        let result = service.test_azure_connection(None, None, None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[tokio::test]
    async fn test_list_azure_subscriptions_success() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_list_azure_subscriptions()
            .times(1)
            .returning(|_, _, _| {
                Ok(vec![AzureSubscription {
                    subscription_id: "sub-id".to_string(),
                    display_name: "Test Sub".to_string(),
                    state: "Enabled".to_string(),
                }])
            });

        let service = ConnectionService::new(Arc::new(mock));
        let result = service.list_azure_subscriptions(None, None, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_list_azure_resource_groups_success() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_list_azure_resource_groups()
            .times(1)
            .returning(|_, _, _, _| {
                Ok(vec![AzureResourceGroup {
                    name: "test-rg".to_string(),
                    location: "eastus".to_string(),
                }])
            });

        let service = ConnectionService::new(Arc::new(mock));
        let result = service
            .list_azure_resource_groups("sub-id".to_string(), None, None, None)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_azure_connection_failure() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_azure_connection()
            .times(1)
            .returning(|_, _, _| Err(anyhow::anyhow!("Azure connection failed")));

        let service = ConnectionService::new(Arc::new(mock));
        let result = service.test_azure_connection(None, None, None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Azure connection failed"));
    }

    #[tokio::test]
    async fn test_list_azure_subscriptions_failure() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_list_azure_subscriptions()
            .times(1)
            .returning(|_, _, _| Err(anyhow::anyhow!("Subscription list failed")));

        let service = ConnectionService::new(Arc::new(mock));
        let result = service.list_azure_subscriptions(None, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_azure_resource_groups_failure() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_list_azure_resource_groups()
            .times(1)
            .returning(|_, _, _, _| Err(anyhow::anyhow!("Resource groups failed")));

        let service = ConnectionService::new(Arc::new(mock));
        let result = service
            .list_azure_resource_groups("sub-id".to_string(), None, None, None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_aws_connection_with_profile_and_role() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_aws_connection()
            .times(1)
            .returning(|profile, role_arn, session_name| {
                assert_eq!(profile, Some("prod".to_string()));
                assert_eq!(role_arn, Some("arn:aws:iam::123:role/Admin".to_string()));
                assert_eq!(session_name, Some("test-session".to_string()));
                Ok(make_success_response())
            });

        let service = ConnectionService::new(Arc::new(mock));
        let result = service
            .test_aws_connection(
                Some("prod".to_string()),
                Some("arn:aws:iam::123:role/Admin".to_string()),
                Some("test-session".to_string()),
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_azure_connection_with_service_principal() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_azure_connection().times(1).returning(
            |auth_method, tenant_id, sp_config| {
                assert_eq!(auth_method, Some("service_principal".to_string()));
                assert_eq!(tenant_id, Some("tenant-123".to_string()));
                assert!(sp_config.is_some());
                Ok(ConnectionTestResponse {
                    success: true,
                    message: Some("Connected".to_string()),
                    account_id: None,
                    user_arn: None,
                    subscription_name: Some("Sub".to_string()),
                })
            },
        );

        let mut sp_config = HashMap::new();
        sp_config.insert("client_id".to_string(), "cid".to_string());
        sp_config.insert("client_secret".to_string(), "secret".to_string());

        let service = ConnectionService::new(Arc::new(mock));
        let result = service
            .test_azure_connection(
                Some("service_principal".to_string()),
                Some("tenant-123".to_string()),
                Some(sp_config),
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_azure_subscriptions_empty_list() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_list_azure_subscriptions()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        let service = ConnectionService::new(Arc::new(mock));
        let result = service.list_azure_subscriptions(None, None, None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_azure_resource_groups_multiple() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_list_azure_resource_groups()
            .times(1)
            .returning(|_, _, _, _| {
                Ok(vec![
                    AzureResourceGroup {
                        name: "rg-1".to_string(),
                        location: "eastus".to_string(),
                    },
                    AzureResourceGroup {
                        name: "rg-2".to_string(),
                        location: "westus".to_string(),
                    },
                    AzureResourceGroup {
                        name: "rg-3".to_string(),
                        location: "japaneast".to_string(),
                    },
                ])
            });

        let service = ConnectionService::new(Arc::new(mock));
        let result = service
            .list_azure_resource_groups("sub-id".to_string(), None, None, None)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }
}
