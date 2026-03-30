use axum::{extract::State, response::Json, routing::post, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;

use crate::api::error::ApiError;
use crate::api::validation::validate_aws_profile_name;
use crate::models::ConnectionTestResponse;
use crate::services::connection_service::ConnectionService;

pub fn router(service: Arc<ConnectionService>) -> Router {
    Router::new()
        .route("/aws/login", post(aws_login))
        .route("/aws/test", post(test_aws_connection))
        .route("/azure/test", post(test_azure_connection))
        .route("/azure/subscriptions", post(list_azure_subscriptions))
        .route("/azure/resource-groups", post(list_azure_resource_groups))
        .with_state(service)
}

#[derive(Deserialize)]
struct AwsLoginRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<String>,
}

#[derive(Deserialize)]
struct AwsConnectionRequest {
    /// プロバイダー識別子（APIリクエストの互換性のために保持）
    #[serde(default)]
    #[allow(dead_code)]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assume_role_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assume_role_session_name: Option<String>,
}

async fn aws_login(Json(request): Json<AwsLoginRequest>) -> Result<Json<Value>, ApiError> {
    // プロファイル名のバリデーション（コマンドインジェクション防御）
    if let Some(profile) = &request.profile {
        validate_aws_profile_name(profile)?;
    }

    // aws loginは対話的なコマンドのため、バックグラウンドで実行
    // ブラウザが開くまで少し時間がかかる可能性があるため、非同期で実行
    let mut cmd = Command::new("aws");
    cmd.arg("login");

    if let Some(profile) = &request.profile {
        cmd.arg("--profile").arg(profile);
    }

    if let Some(region) = &request.region {
        cmd.env("AWS_DEFAULT_REGION", region);
    }

    // 標準出力と標準エラー出力をキャプチャ
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // コマンドを非同期で実行（タイムアウトを設定）
    let output = tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output()).await;

    match output {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if output.status.success() {
                Ok(Json(json!({
                    "success": true,
                    "message": "aws login completed successfully. Please complete authentication in your browser.",
                    "output": stdout,
                    "stderr": stderr
                })))
            } else {
                // エラーでも、ブラウザが開いた可能性があるため、部分的な成功として扱う
                if stdout.contains("Updated profile") || stderr.contains("Updated profile") {
                    Ok(Json(json!({
                        "success": true,
                        "message": "aws login process started. Please complete authentication in your browser.",
                        "output": stdout,
                        "stderr": stderr
                    })))
                } else {
                    Err(ApiError::Validation(format!(
                        "aws login failed: {}",
                        stderr
                    )))
                }
            }
        }
        Ok(Err(e)) => Err(ApiError::Internal(format!(
            "Failed to execute aws login: {}",
            e
        ))),
        Err(_) => {
            // タイムアウト - aws loginはブラウザでの認証を待つため、タイムアウトは正常な場合がある
            Ok(Json(json!({
                "success": true,
                "message": "aws login process started. Please complete authentication in your browser. The command may still be running in the background.",
                "note": "If the browser did not open automatically, check the terminal output for the authorization URL."
            })))
        }
    }
}

#[derive(Deserialize)]
struct AzureConnectionRequest {
    /// プロバイダー識別子（APIリクエストの互換性のために保持）
    #[serde(default)]
    #[allow(dead_code)]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_principal_config: Option<HashMap<String, String>>,
}

async fn test_aws_connection(
    State(service): State<Arc<ConnectionService>>,
    Json(request): Json<AwsConnectionRequest>,
) -> Result<Json<ConnectionTestResponse>, ApiError> {
    // プロファイル名のバリデーション（コマンドインジェクション防御）
    if let Some(profile) = &request.profile {
        validate_aws_profile_name(profile)?;
    }

    service
        .test_aws_connection(
            request.profile.clone(),
            request.assume_role_arn.clone(),
            request.assume_role_session_name.clone(),
        )
        .await
        .map(Json)
        .map_err(|e| ApiError::ExternalService {
            service: "AWS".to_string(),
            message: e.to_string(),
        })
}

async fn test_azure_connection(
    State(service): State<Arc<ConnectionService>>,
    Json(request): Json<AzureConnectionRequest>,
) -> Result<Json<ConnectionTestResponse>, ApiError> {
    service
        .test_azure_connection(
            request.auth_method.clone(),
            request.tenant_id.clone(),
            request.service_principal_config.clone(),
        )
        .await
        .map(Json)
        .map_err(|e| ApiError::ExternalService {
            service: "Azure".to_string(),
            message: e.to_string(),
        })
}

/// Azure サブスクリプション一覧取得リクエスト
/// 認証情報をPOSTボディで受け取る（GETクエリパラメータにシークレットを含めないため）
#[derive(Deserialize)]
struct AzureSubscriptionsRequest {
    auth_method: Option<String>,
    tenant_id: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
}

async fn list_azure_subscriptions(
    State(service): State<Arc<ConnectionService>>,
    Json(params): Json<AzureSubscriptionsRequest>,
) -> Result<Json<Value>, ApiError> {
    let service_principal_config = if params.auth_method.as_deref() == Some("service_principal") {
        if let (Some(client_id), Some(client_secret)) = (params.client_id, params.client_secret) {
            let mut config = HashMap::new();
            config.insert("client_id".to_string(), client_id);
            config.insert("client_secret".to_string(), client_secret);
            Some(config)
        } else {
            None
        }
    } else {
        None
    };

    service
        .list_azure_subscriptions(
            params.auth_method,
            params.tenant_id,
            service_principal_config,
        )
        .await
        .map(|subscriptions| Json(json!({ "subscriptions": subscriptions })))
        .map_err(|e| ApiError::ExternalService {
            service: "Azure".to_string(),
            message: e.to_string(),
        })
}

/// Azure リソースグループ一覧取得リクエスト
/// 認証情報をPOSTボディで受け取る（GETクエリパラメータにシークレットを含めないため）
#[derive(Deserialize)]
struct AzureResourceGroupsRequest {
    subscription_id: String,
    auth_method: Option<String>,
    tenant_id: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
}

async fn list_azure_resource_groups(
    State(service): State<Arc<ConnectionService>>,
    Json(params): Json<AzureResourceGroupsRequest>,
) -> Result<Json<Value>, ApiError> {
    let service_principal_config = if params.auth_method.as_deref() == Some("service_principal") {
        if let (Some(client_id), Some(client_secret)) = (params.client_id, params.client_secret) {
            let mut config = HashMap::new();
            config.insert("client_id".to_string(), client_id);
            config.insert("client_secret".to_string(), client_secret);
            Some(config)
        } else {
            None
        }
    } else {
        None
    };

    service
        .list_azure_resource_groups(
            params.subscription_id,
            params.auth_method,
            params.tenant_id,
            service_principal_config,
        )
        .await
        .map(|resource_groups| Json(json!({ "resource_groups": resource_groups })))
        .map_err(|e| ApiError::ExternalService {
            service: "Azure".to_string(),
            message: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AzureResourceGroup, AzureSubscription};
    use crate::services::connection_service::{ConnectionService, MockCloudConnectionTester};
    use axum_test::TestServer;
    use serde_json::json;
    use tower::ServiceBuilder;
    use tower_http::cors::CorsLayer;

    fn make_success_aws_response() -> ConnectionTestResponse {
        ConnectionTestResponse {
            success: true,
            message: Some("Connection successful".to_string()),
            account_id: Some("123456789012".to_string()),
            user_arn: Some("arn:aws:iam::123456789012:user/test".to_string()),
            subscription_name: None,
        }
    }

    fn make_success_azure_response() -> ConnectionTestResponse {
        ConnectionTestResponse {
            success: true,
            message: Some("Connection successful".to_string()),
            account_id: None,
            user_arn: None,
            subscription_name: Some("Test Subscription".to_string()),
        }
    }

    fn create_test_app(service: Arc<ConnectionService>) -> axum::Router {
        axum::Router::new()
            .nest("/api/connection", router(service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()))
    }

    fn create_mock_service_all_success() -> Arc<ConnectionService> {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_aws_connection()
            .returning(|_, _, _| Ok(make_success_aws_response()));
        mock.expect_test_azure_connection()
            .returning(|_, _, _| Ok(make_success_azure_response()));
        mock.expect_list_azure_subscriptions().returning(|_, _, _| {
            Ok(vec![AzureSubscription {
                subscription_id: "sub-id".to_string(),
                display_name: "Test Sub".to_string(),
                state: "Enabled".to_string(),
            }])
        });
        mock.expect_list_azure_resource_groups()
            .returning(|_, _, _, _| {
                Ok(vec![AzureResourceGroup {
                    name: "test-rg".to_string(),
                    location: "eastus".to_string(),
                }])
            });
        Arc::new(ConnectionService::new(Arc::new(mock)))
    }

    #[tokio::test]
    async fn test_aws_connection_endpoint_success() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/aws/test")
            .json(&json!({
                "profile": null,
                "assume_role_arn": null,
                "assume_role_session_name": null
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            200,
            "Expected OK (200), got {}",
            response.status_code().as_u16()
        );

        let body: ConnectionTestResponse = response.json();
        assert!(body.success);
    }

    #[tokio::test]
    async fn test_aws_connection_endpoint_with_profile() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/aws/test")
            .json(&json!({
                "profile": "test-profile",
                "assume_role_arn": null,
                "assume_role_session_name": null
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            200,
            "Expected OK (200) for valid profile, got {}",
            response.status_code().as_u16()
        );
    }

    #[tokio::test]
    async fn test_aws_connection_rejects_invalid_profile_name() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/aws/test")
            .json(&json!({
                "profile": "profile;rm -rf /",
                "assume_role_arn": null,
                "assume_role_session_name": null
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            400,
            "Invalid profile name should return 400 Bad Request"
        );
    }

    #[tokio::test]
    async fn test_aws_login_rejects_invalid_profile_name() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/aws/login")
            .json(&json!({
                "profile": "../../etc/passwd"
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            400,
            "Invalid profile name should return 400 Bad Request"
        );
    }

    #[tokio::test]
    async fn test_azure_connection_endpoint() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/azure/test")
            .json(&json!({
                "auth_method": null,
                "tenant_id": null,
                "service_principal_config": null
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            200,
            "Expected OK (200), got {}",
            response.status_code().as_u16()
        );
    }

    #[tokio::test]
    async fn test_azure_connection_endpoint_with_service_principal() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let mut service_principal_config = HashMap::new();
        service_principal_config.insert("client_id".to_string(), "test-client-id".to_string());
        service_principal_config.insert("client_secret".to_string(), "test-secret".to_string());

        let response = server
            .post("/api/connection/azure/test")
            .json(&json!({
                "auth_method": "service_principal",
                "tenant_id": "test-tenant-id",
                "service_principal_config": service_principal_config
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            200,
            "Expected OK (200), got {}",
            response.status_code().as_u16()
        );
    }

    #[tokio::test]
    async fn test_list_azure_subscriptions_uses_post() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/azure/subscriptions")
            .json(&json!({
                "auth_method": "az_login"
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            200,
            "Expected OK (200), got {}",
            response.status_code().as_u16()
        );

        let body: serde_json::Value = response.json();
        assert!(
            body.get("subscriptions").is_some(),
            "Response should have subscriptions field"
        );
    }

    #[tokio::test]
    async fn test_list_azure_subscriptions_with_service_principal() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/azure/subscriptions")
            .json(&json!({
                "auth_method": "service_principal",
                "client_id": "test-client-id",
                "client_secret": "test-secret"
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            200,
            "Expected OK (200), got {}",
            response.status_code().as_u16()
        );
    }

    #[tokio::test]
    async fn test_list_azure_resource_groups_uses_post() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/azure/resource-groups")
            .json(&json!({
                "subscription_id": "test-subscription-id",
                "auth_method": "az_login"
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            200,
            "Expected OK (200), got {}",
            response.status_code().as_u16()
        );

        let body: serde_json::Value = response.json();
        assert!(
            body.get("resource_groups").is_some(),
            "Response should have resource_groups field"
        );
    }

    #[tokio::test]
    async fn test_list_azure_resource_groups_missing_subscription_id() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        // subscription_id なしでリクエスト → 422 Unprocessable Entity (デシリアライズエラー)
        let response = server
            .post("/api/connection/azure/resource-groups")
            .json(&json!({
                "auth_method": "az_login"
            }))
            .await;

        let status_u16 = response.status_code().as_u16();
        assert!(
            status_u16 == 400 || status_u16 == 422,
            "Missing subscription_id should return 400 or 422, got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_azure_subscriptions_get_method_not_allowed() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        // GETは許可されなくなった（POSTのみ）
        let response = server.get("/api/connection/azure/subscriptions").await;

        assert_eq!(
            response.status_code().as_u16(),
            405,
            "GET method should return 405 Method Not Allowed"
        );
    }

    #[tokio::test]
    async fn test_azure_resource_groups_get_method_not_allowed() {
        let service = create_mock_service_all_success();
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server.get("/api/connection/azure/resource-groups").await;

        assert_eq!(
            response.status_code().as_u16(),
            405,
            "GET method should return 405 Method Not Allowed"
        );
    }

    #[tokio::test]
    async fn test_aws_connection_returns_502_on_failure() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_aws_connection()
            .returning(|_, _, _| Err(anyhow::anyhow!("AWS connection failed")));

        let service = Arc::new(ConnectionService::new(Arc::new(mock)));
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/aws/test")
            .json(&json!({"profile": null, "assume_role_arn": null, "assume_role_session_name": null}))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            502,
            "Expected BAD_GATEWAY (502) on connection failure"
        );
    }

    #[tokio::test]
    async fn test_azure_connection_failure() {
        let mut mock = MockCloudConnectionTester::new();
        mock.expect_test_azure_connection()
            .returning(|_, _, _| Err(anyhow::anyhow!("Azure authentication failed")));

        let service = Arc::new(ConnectionService::new(Arc::new(mock)));
        let app = create_test_app(service);
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/connection/azure/test")
            .json(&json!({
                "auth_method": "az_login",
                "tenant_id": null,
                "service_principal_config": null
            }))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            502,
            "Expected BAD_GATEWAY (502) on Azure connection failure"
        );
        let body: serde_json::Value = response.json();
        assert!(
            body.get("error").is_some(),
            "Error response should have error field"
        );
    }
}
