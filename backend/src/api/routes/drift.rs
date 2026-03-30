use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::sync::Arc;

use crate::api::error::ApiError;
use crate::models::drift::DriftDetectionRequest;
use crate::services::drift_service::DriftService;

/// ドリフト検出 API ルーター
pub fn router(service: Arc<DriftService>) -> Router {
    Router::new()
        .route("/detect", post(detect_drift))
        .route("/{drift_id}", get(get_drift_report))
        .with_state(service)
}

/// POST /api/drift/detect
///
/// Terraform state とスキャン結果を比較してドリフトを検出する
async fn detect_drift(
    State(service): State<Arc<DriftService>>,
    Json(request): Json<DriftDetectionRequest>,
) -> Result<Json<Value>, ApiError> {
    if request.scan_id.is_empty() {
        return Err(ApiError::Validation("scan_idは必須です".to_string()));
    }
    if request.state_content.is_empty() {
        return Err(ApiError::Validation("state_contentは必須です".to_string()));
    }

    match service
        .detect_drift(&request.scan_id, &request.state_content)
        .await
    {
        Ok(report) => Ok(Json(serde_json::to_value(report).unwrap())),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("見つかりません") {
                Err(ApiError::NotFound(msg))
            } else if msg.contains("パース") || msg.contains("バージョン") {
                Err(ApiError::Validation(msg))
            } else {
                Err(ApiError::Internal(msg))
            }
        }
    }
}

/// GET /api/drift/:drift_id
///
/// 保存済みのドリフトレポートを取得する
async fn get_drift_report(
    State(service): State<Arc<DriftService>>,
    Path(drift_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    match service.get_report(&drift_id).await {
        Some(report) => Ok(Json(serde_json::to_value(report).unwrap())),
        None => Err(ApiError::NotFound(format!(
            "ドリフトレポート {} が見つかりません",
            drift_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::scan_service::{MockScannerFactory, ScanService};
    use axum_test::TestServer;
    use serde_json::json;
    use std::sync::Arc;
    use tower::ServiceBuilder;
    use tower_http::cors::CorsLayer;

    fn make_state_json(resources: serde_json::Value) -> String {
        json!({
            "version": 4,
            "terraform_version": "1.5.0",
            "serial": 1,
            "lineage": "test",
            "resources": resources
        })
        .to_string()
    }

    fn create_test_scan_service() -> Arc<ScanService> {
        let mut mock = MockScannerFactory::new();
        mock.expect_run_scan()
            .returning(|_, _| Ok(json!({ "provider": "aws", "users": [] })));
        Arc::new(ScanService::new(Arc::new(mock)))
    }

    fn create_test_app(drift_service: Arc<DriftService>) -> TestServer {
        let app = axum::Router::new()
            .nest("/api/drift", router(drift_service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()));
        TestServer::new(app.into_make_service()).unwrap()
    }

    #[tokio::test]
    async fn test_detect_drift_empty_scan_id() {
        let scan_service = create_test_scan_service();
        let drift_service = Arc::new(DriftService::new(scan_service));
        let server = create_test_app(drift_service);

        let response = server
            .post("/api/drift/detect")
            .json(&json!({
                "scan_id": "",
                "state_content": make_state_json(json!([]))
            }))
            .await;

        assert_eq!(response.status_code().as_u16(), 400);
    }

    #[tokio::test]
    async fn test_detect_drift_empty_state_content() {
        let scan_service = create_test_scan_service();
        let drift_service = Arc::new(DriftService::new(scan_service));
        let server = create_test_app(drift_service);

        let response = server
            .post("/api/drift/detect")
            .json(&json!({
                "scan_id": "some-scan-id",
                "state_content": ""
            }))
            .await;

        assert_eq!(response.status_code().as_u16(), 400);
    }

    #[tokio::test]
    async fn test_detect_drift_scan_not_found() {
        let scan_service = create_test_scan_service();
        let drift_service = Arc::new(DriftService::new(scan_service));
        let server = create_test_app(drift_service);

        let response = server
            .post("/api/drift/detect")
            .json(&json!({
                "scan_id": "unknown-scan-id",
                "state_content": make_state_json(json!([]))
            }))
            .await;

        assert_eq!(response.status_code().as_u16(), 404);
    }

    #[tokio::test]
    async fn test_detect_drift_success() {
        let scan_service = create_test_scan_service();
        let test_config = crate::models::ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets: std::collections::HashMap::new(),
            filters: std::collections::HashMap::new(),
            include_tags: true,
        };
        scan_service
            .insert_test_scan_data(
                "drift-scan-1".to_string(),
                test_config,
                json!({ "buckets": [] }),
            )
            .await;

        let drift_service = Arc::new(DriftService::new(scan_service));
        let server = create_test_app(drift_service);

        let response = server
            .post("/api/drift/detect")
            .json(&json!({
                "scan_id": "drift-scan-1",
                "state_content": make_state_json(json!([]))
            }))
            .await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: Value = response.json();
        assert!(body["drift_id"].is_string());
    }

    #[tokio::test]
    async fn test_get_drift_report_not_found() {
        let scan_service = create_test_scan_service();
        let drift_service = Arc::new(DriftService::new(scan_service));
        let server = create_test_app(drift_service);

        let response = server.get("/api/drift/nonexistent-drift-id").await;

        assert_eq!(response.status_code().as_u16(), 404);
    }
}
