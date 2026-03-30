use axum::{
    extract::{Path, State},
    response::Json,
    routing::post,
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::api::error::{ApiError, ErrorResponse};
use crate::services::export_service::ExportService;
use crate::services::resource_service::ResourceService;

pub fn router(resource_service: Arc<ResourceService>) -> Router {
    Router::new()
        .route("/:scan_id", post(export_resources))
        .with_state(resource_service)
}

#[derive(Deserialize, ToSchema)]
pub struct ExportRequest {
    /// エクスポート形式（"json" または "csv"）
    format: String,
    /// エクスポートするリソースタイプ（空の場合は全リソース）
    #[serde(default)]
    resource_types: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/api/export/{scan_id}",
    tag = "export",
    params(
        ("scan_id" = String, Path, description = "スキャンID")
    ),
    request_body = ExportRequest,
    responses(
        (status = 200, description = "エクスポート成功", body = Value),
        (status = 400, description = "バリデーションエラー", body = ErrorResponse),
        (status = 404, description = "スキャンが見つからない", body = ErrorResponse),
        (status = 500, description = "内部エラー", body = ErrorResponse),
    )
)]
pub(crate) async fn export_resources(
    State(service): State<Arc<ResourceService>>,
    Path(scan_id): Path<String>,
    Json(request): Json<ExportRequest>,
) -> Result<Json<Value>, ApiError> {
    let scan_data = service
        .scan_service()
        .get_scan_data(&scan_id)
        .await
        .ok_or_else(|| ApiError::NotFound(format!("Scan not found: {}", scan_id)))?;

    let content = match request.format.as_str() {
        "csv" => ExportService::export_csv(&scan_data, &request.resource_types)
            .map_err(|e| ApiError::Internal(e.to_string()))?,
        "json" => ExportService::export_json(&scan_data, &request.resource_types)
            .map_err(|e| ApiError::Internal(e.to_string()))?,
        _ => {
            return Err(ApiError::Validation(format!(
                "Unsupported export format: {}. Supported values: csv, json",
                request.format
            )));
        }
    };

    Ok(Json(json!({
        "format": request.format,
        "content": content,
    })))
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

    fn create_test_app(resource_service: Arc<ResourceService>) -> TestServer {
        let app = axum::Router::new()
            .nest("/api/export", router(resource_service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()));
        TestServer::new(app.into_make_service()).unwrap()
    }

    async fn make_resource_service_with_scan(
        scan_id: &str,
        data: serde_json::Value,
    ) -> Arc<ResourceService> {
        let mut mock = MockScannerFactory::new();
        mock.expect_run_scan()
            .returning(|_, _| Ok(json!({ "provider": "aws" })));
        let scan_service = Arc::new(ScanService::new(Arc::new(mock)));

        let config = crate::models::ScanConfig {
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
            .insert_test_scan_data(scan_id.to_string(), config, data)
            .await;

        Arc::new(ResourceService::new(scan_service))
    }

    #[tokio::test]
    async fn test_export_json_success() {
        let resource_service = make_resource_service_with_scan(
            "export-scan-1",
            json!({ "provider": "aws", "users": [{"user_name": "alice"}] }),
        )
        .await;
        let server = create_test_app(resource_service);

        let response = server
            .post("/api/export/export-scan-1")
            .json(&json!({ "format": "json", "resource_types": [] }))
            .await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: Value = response.json();
        assert_eq!(body["format"], "json");
        assert!(body["content"].is_string());
    }

    #[tokio::test]
    async fn test_export_csv_success() {
        let resource_service = make_resource_service_with_scan(
            "export-scan-2",
            json!({ "provider": "aws", "users": [{"user_name": "bob"}] }),
        )
        .await;
        let server = create_test_app(resource_service);

        let response = server
            .post("/api/export/export-scan-2")
            .json(&json!({ "format": "csv", "resource_types": [] }))
            .await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: Value = response.json();
        assert_eq!(body["format"], "csv");
    }

    #[tokio::test]
    async fn test_export_unsupported_format() {
        let resource_service =
            make_resource_service_with_scan("export-scan-3", json!({ "provider": "aws" })).await;
        let server = create_test_app(resource_service);

        let response = server
            .post("/api/export/export-scan-3")
            .json(&json!({ "format": "xml", "resource_types": [] }))
            .await;

        assert_eq!(response.status_code().as_u16(), 400);
    }

    #[tokio::test]
    async fn test_export_scan_not_found() {
        let mut mock = MockScannerFactory::new();
        mock.expect_run_scan()
            .returning(|_, _| Ok(json!({ "provider": "aws" })));
        let scan_service = Arc::new(ScanService::new(Arc::new(mock)));
        let resource_service = Arc::new(ResourceService::new(scan_service));
        let server = create_test_app(resource_service);

        let response = server
            .post("/api/export/unknown-scan-id")
            .json(&json!({ "format": "json", "resource_types": [] }))
            .await;

        assert_eq!(response.status_code().as_u16(), 404);
    }
}
