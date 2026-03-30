use axum::{
    extract::{Path, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        Json,
    },
    routing::{get, post},
    Router,
};
use futures::stream::Stream;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::api::error::ApiError;
use crate::services::scan_service::{ScanProgressEvent, ScanService};

pub fn router(service: Arc<ScanService>) -> Router {
    Router::new()
        .route("/aws", post(scan_aws))
        .route("/azure", post(scan_azure))
        .route("/aws/stream", post(scan_aws_stream))
        .route("/azure/stream", post(scan_azure_stream))
        .route("/:scan_id/status", get(get_scan_status))
        .with_state(service)
}

#[derive(serde::Deserialize)]
struct ScanRequest {
    config: crate::models::ScanConfig,
}

async fn scan_aws(
    State(service): State<Arc<ScanService>>,
    Json(request): Json<ScanRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut config = request.config;
    config.provider = "aws".to_string();

    match service.start_scan(config).await {
        Ok(scan_id) => Ok(Json(json!({
            "scan_id": scan_id,
            "status": "in_progress"
        }))),
        Err(e) => Err(ApiError::ExternalService {
            service: "AWS".to_string(),
            message: e.to_string(),
        }),
    }
}

async fn scan_azure(
    State(service): State<Arc<ScanService>>,
    Json(request): Json<ScanRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut config = request.config;
    config.provider = "azure".to_string();

    match service.start_scan(config).await {
        Ok(scan_id) => Ok(Json(json!({
            "scan_id": scan_id,
            "status": "in_progress"
        }))),
        Err(e) => Err(ApiError::ExternalService {
            service: "Azure".to_string(),
            message: e.to_string(),
        }),
    }
}

/// AWSスキャンをSSEストリーミングで実行
async fn scan_aws_stream(
    State(service): State<Arc<ScanService>>,
    Json(request): Json<ScanRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let mut config = request.config;
    config.provider = "aws".to_string();

    match service.start_scan_stream(config).await {
        Ok(rx) => {
            let stream = create_sse_stream(rx);
            Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
        }
        Err(e) => Err(ApiError::ExternalService {
            service: "AWS".to_string(),
            message: e.to_string(),
        }),
    }
}

/// AzureスキャンをSSEストリーミングで実行
async fn scan_azure_stream(
    State(service): State<Arc<ScanService>>,
    Json(request): Json<ScanRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let mut config = request.config;
    config.provider = "azure".to_string();

    match service.start_scan_stream(config).await {
        Ok(rx) => {
            let stream = create_sse_stream(rx);
            Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
        }
        Err(e) => Err(ApiError::ExternalService {
            service: "Azure".to_string(),
            message: e.to_string(),
        }),
    }
}

/// ReceiverStreamからSSEイベントストリームを作成
fn create_sse_stream(
    rx: tokio::sync::mpsc::Receiver<ScanProgressEvent>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    ReceiverStream::new(rx).map(|event| {
        let event_type = event.event_type.clone();
        let json_data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
        Ok(Event::default().event(event_type).data(json_data))
    })
}

async fn get_scan_status(
    State(service): State<Arc<ScanService>>,
    Path(scan_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    match service.get_scan_result(&scan_id).await {
        Some(result) => {
            let mut response = json!({
                "scan_id": scan_id,
                "status": result.status,
                "progress": result.progress.unwrap_or(0),
                "message": result.message.clone().unwrap_or_else(|| "Scan in progress".to_string()),
            });
            if let Some(summary) = result.summary {
                response["summary"] = json!(summary);
            }
            Ok(Json(response))
        }
        None => Err(ApiError::NotFound(format!(
            "Scan with ID '{}' not found",
            scan_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::scan_service::MockScannerFactory;
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use serde_json::json;
    use std::collections::HashMap;
    use tower::ServiceBuilder;
    use tower_http::cors::CorsLayer;

    fn create_mock_service_success() -> Arc<ScanService> {
        let mut mock = MockScannerFactory::new();
        mock.expect_run_scan().returning(|_, _| {
            Ok(serde_json::json!({
                "provider": "aws",
                "users": [],
                "groups": [],
                "roles": [],
                "policies": []
            }))
        });
        Arc::new(ScanService::new(Arc::new(mock)))
    }

    fn create_test_app() -> Router {
        let service = create_mock_service_success();
        Router::new()
            .nest("/api/scan", router(service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()))
    }

    #[tokio::test]
    async fn test_scan_aws_endpoint() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let mut scan_targets = HashMap::new();
        scan_targets.insert("users".to_string(), true);
        scan_targets.insert("groups".to_string(), true);

        let response = server
            .post("/api/scan/aws")
            .json(&json!({
                "config": {
                    "provider": "aws",
                    "profile": null,
                    "scan_targets": scan_targets,
                    "filters": {}
                }
            }))
            .await;

        assert_eq!(
            response.status_code(),
            StatusCode::OK,
            "scan_aws should return 200 OK"
        );
        let body: serde_json::Value = response.json();
        assert!(
            body.get("scan_id").is_some(),
            "Response should have scan_id"
        );
        assert_eq!(
            body.get("status").and_then(|s| s.as_str()),
            Some("in_progress")
        );
    }

    #[tokio::test]
    async fn test_scan_aws_endpoint_with_profile() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let mut scan_targets = HashMap::new();
        scan_targets.insert("users".to_string(), true);

        let response = server
            .post("/api/scan/aws")
            .json(&json!({
                "config": {
                    "provider": "aws",
                    "profile": "test-profile",
                    "scan_targets": scan_targets,
                    "filters": {}
                }
            }))
            .await;

        assert_eq!(
            response.status_code(),
            StatusCode::OK,
            "scan_aws with profile should return 200 OK"
        );
    }

    #[tokio::test]
    async fn test_scan_azure_endpoint() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let mut scan_targets = HashMap::new();
        scan_targets.insert("role_definitions".to_string(), true);
        scan_targets.insert("role_assignments".to_string(), true);

        let response = server
            .post("/api/scan/azure")
            .json(&json!({
                "config": {
                    "provider": "azure",
                    "subscription_id": "test-subscription-id",
                    "auth_method": "az_login",
                    "scan_targets": scan_targets,
                    "filters": {}
                }
            }))
            .await;

        assert_eq!(
            response.status_code(),
            StatusCode::OK,
            "scan_azure should return 200 OK"
        );
        let body: serde_json::Value = response.json();
        assert!(
            body.get("scan_id").is_some(),
            "Response should have scan_id"
        );
        assert_eq!(
            body.get("status").and_then(|s| s.as_str()),
            Some("in_progress")
        );
    }

    #[tokio::test]
    async fn test_get_scan_status_not_found() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .get("/api/scan/00000000-0000-0000-0000-000000000000/status")
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            404,
            "Expected NOT_FOUND for non-existent scan_id"
        );
        let body: serde_json::Value = response.json();
        assert!(body.get("error").is_some());
    }

    #[tokio::test]
    async fn test_start_scan_stream_aws() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let mut scan_targets = HashMap::new();
        scan_targets.insert("users".to_string(), true);

        let response = server
            .post("/api/scan/aws/stream")
            .json(&json!({
                "config": {
                    "provider": "aws",
                    "scan_targets": scan_targets,
                    "filters": {}
                }
            }))
            .await;

        // SSE ストリームエンドポイントは 200 OK を返す
        assert_eq!(
            response.status_code().as_u16(),
            200,
            "scan_aws/stream should return 200 OK, got {}",
            response.status_code().as_u16()
        );

        // Content-Type が text/event-stream であることを確認
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.contains("text/event-stream"),
            "Expected Content-Type text/event-stream, got: {}",
            content_type
        );
    }

    #[tokio::test]
    async fn test_get_scan_status_after_start() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let mut scan_targets = HashMap::new();
        scan_targets.insert("users".to_string(), true);

        // スキャンを開始（常に成功）
        let start_response = server
            .post("/api/scan/aws")
            .json(&json!({
                "config": {
                    "provider": "aws",
                    "scan_targets": scan_targets,
                    "filters": {}
                }
            }))
            .await;

        assert_eq!(start_response.status_code(), StatusCode::OK);
        let start_body: serde_json::Value = start_response.json();
        let scan_id = start_body
            .get("scan_id")
            .and_then(|id| id.as_str())
            .unwrap();

        // ステータスを取得（スキャンはin_progressまたはcompleted）
        let status_response = server.get(&format!("/api/scan/{}/status", scan_id)).await;

        assert_eq!(
            status_response.status_code(),
            StatusCode::OK,
            "Scan status should be found after starting"
        );
        let status_body: serde_json::Value = status_response.json();
        assert!(status_body.get("scan_id").is_some());
        assert!(status_body.get("status").is_some());
    }
}
