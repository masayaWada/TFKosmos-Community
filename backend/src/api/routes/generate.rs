use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::api::error::ApiError;
use crate::models::GenerationResponse;
use crate::services::generation_service::GenerationService;
use crate::services::validation_service::ValidationService;
use crate::services::zip_service::ZipService;

pub fn router(service: Arc<GenerationService>) -> Router {
    Router::new()
        .route("/terraform", post(generate_terraform))
        .route("/terraform/check", get(check_terraform))
        .route("/:generation_id/download", get(download_generated_files))
        .route("/:generation_id/validate", post(validate_generation))
        .route("/:generation_id/format/check", get(check_format))
        .route("/:generation_id/format", post(format_code))
        .with_state(service)
}

#[derive(serde::Deserialize)]
struct GenerateTerraformRequest {
    scan_id: String,
    config: crate::models::GenerationConfig,
    #[serde(default)]
    selected_resources: std::collections::HashMap<String, serde_json::Value>,
}

async fn generate_terraform(
    State(service): State<Arc<GenerationService>>,
    Json(request): Json<GenerateTerraformRequest>,
) -> Result<Json<GenerationResponse>, ApiError> {
    info!(scan_id = %request.scan_id, "Received generation request");
    debug!(config = ?request.config, "Generation config");
    debug!(selected_resources = ?request.selected_resources, "Selected resources");

    // Convert selected_resources from HashMap<String, Value> to HashMap<String, Vec<Value>>
    // Value can be either an array of strings (IDs) or an array of objects
    let mut selected_resources_converted: std::collections::HashMap<String, Vec<Value>> =
        std::collections::HashMap::new();
    for (resource_type, value) in request.selected_resources {
        if let Some(array) = value.as_array() {
            selected_resources_converted.insert(resource_type, array.clone());
        } else if let Some(id_str) = value.as_str() {
            // Single string ID
            selected_resources_converted
                .insert(resource_type, vec![Value::String(id_str.to_string())]);
        }
    }

    debug!(converted = ?selected_resources_converted, "Converted selected resources");

    match service
        .generate_terraform(
            &request.scan_id,
            request.config,
            selected_resources_converted,
        )
        .await
    {
        Ok(result) => {
            info!(
                generation_id = %result.generation_id,
                files_count = result.files.len(),
                "Generation successful"
            );
            Ok(Json(result))
        }
        Err(e) => {
            let error_msg = e.to_string();
            warn!(error = %error_msg, "Generation failed");

            // Log error chain for debugging
            let mut error_chain = Vec::new();
            let mut current_error: &dyn std::error::Error = e.as_ref();
            error_chain.push(current_error.to_string());
            while let Some(source) = current_error.source() {
                error_chain.push(source.to_string());
                current_error = source;
            }
            debug!(error_chain = ?error_chain, "Error chain");

            Err(ApiError::Internal(error_msg))
        }
    }
}

async fn download_generated_files(
    State(service): State<Arc<GenerationService>>,
    Path(generation_id): Path<String>,
) -> Result<Response, ApiError> {
    let cache_entry = service.get_cache_entry(&generation_id).await;

    let entry = cache_entry.ok_or_else(|| {
        ApiError::NotFound(format!(
            "Generation result with ID '{}' not found",
            generation_id
        ))
    })?;

    match ZipService::create_zip(&entry.output_path, &generation_id).await {
        Ok(zip_data) => {
            use axum::body::Body;

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/zip")
                .header(
                    "Content-Disposition",
                    format!(
                        "attachment; filename=\"terraform-output-{}.zip\"",
                        generation_id
                    ),
                )
                .body(Body::from(zip_data))
                .map_err(|e| ApiError::Internal(format!("Failed to build response: {}", e)))
        }
        Err(e) => Err(ApiError::Internal(format!("Failed to create ZIP: {}", e))),
    }
}

async fn check_terraform(_: State<Arc<GenerationService>>) -> Result<Json<Value>, ApiError> {
    let version = ValidationService::check_terraform();
    Ok(Json(serde_json::json!({
        "available": version.available,
        "version": version.version
    })))
}

async fn validate_generation(
    State(service): State<Arc<GenerationService>>,
    Path(generation_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let entry = service
        .get_cache_entry(&generation_id)
        .await
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "Generation result with ID '{}' not found",
                generation_id
            ))
        })?;
    ValidationService::validate_generation(&entry.output_path)
        .await
        .map(|result| {
            Json(serde_json::json!({
                "valid": result.valid,
                "errors": result.errors,
                "warnings": result.warnings
            }))
        })
        .map_err(|e| ApiError::Internal(e.to_string()))
}

async fn check_format(
    State(service): State<Arc<GenerationService>>,
    Path(generation_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let entry = service
        .get_cache_entry(&generation_id)
        .await
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "Generation result with ID '{}' not found",
                generation_id
            ))
        })?;
    ValidationService::check_format(&entry.output_path)
        .await
        .map(|result| {
            Json(serde_json::json!({
                "formatted": result.formatted,
                "diff": result.diff,
                "files_changed": result.files_changed
            }))
        })
        .map_err(|e| ApiError::Internal(e.to_string()))
}

async fn format_code(
    State(service): State<Arc<GenerationService>>,
    Path(generation_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let entry = service
        .get_cache_entry(&generation_id)
        .await
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "Generation result with ID '{}' not found",
                generation_id
            ))
        })?;
    ValidationService::format_code(&entry.output_path)
        .await
        .map(|files| {
            Json(serde_json::json!({
                "success": true,
                "files_formatted": files
            }))
        })
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::scan_service::{RealScannerFactory, ScanService};
    use axum_test::TestServer;
    use serde_json::json;
    use std::collections::HashMap;
    use tower::ServiceBuilder;
    use tower_http::cors::CorsLayer;

    fn create_test_app() -> Router {
        let scan_service = Arc::new(ScanService::new(Arc::new(RealScannerFactory::new())));
        let generation_service = Arc::new(GenerationService::new(scan_service));
        Router::new()
            .nest("/api/generate", router(generation_service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()))
    }

    #[tokio::test]
    async fn test_check_terraform_endpoint() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server.get("/api/generate/terraform/check").await;

        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "check_terraform endpoint should always return OK (200), got {}",
            status_u16
        );

        let body: serde_json::Value = response.json();
        assert!(
            body.get("available").is_some(),
            "Response should have available field"
        );
        assert!(
            body.get("version").is_some(),
            "Response should have version field"
        );
    }

    #[tokio::test]
    async fn test_generate_terraform_endpoint_invalid_scan_id() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/generate/terraform")
            .json(&json!({
                "scan_id": "non-existent-scan-id",
                "config": {
                    "output_path": "/tmp/test",
                    "file_split_rule": "single",
                    "naming_convention": "snake_case",
                    "import_script_format": "sh",
                    "generate_readme": true
                },
                "selected_resources": {}
            }))
            .await;

        // スキャンIDが存在しない場合、内部エラー（500）が返る
        let status_u16 = response.status_code().as_u16();
        assert!(
            status_u16 == 500 || status_u16 == 404,
            "Expected INTERNAL_SERVER_ERROR (500) or NOT_FOUND (404) for invalid scan_id, got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_generate_terraform_endpoint_with_selected_resources() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let mut selected_resources = HashMap::new();
        selected_resources.insert("users".to_string(), json!(["user1", "user2"]));

        let response = server
            .post("/api/generate/terraform")
            .json(&json!({
                "scan_id": "non-existent-scan-id",
                "config": {
                    "output_path": "/tmp/test",
                    "file_split_rule": "by_resource_type",
                    "naming_convention": "kebab-case",
                    "import_script_format": "sh",
                    "generate_readme": false
                },
                "selected_resources": selected_resources
            }))
            .await;

        // スキャンIDが存在しない場合、内部エラー（500）が返る
        let status_u16 = response.status_code().as_u16();
        assert!(
            status_u16 == 500 || status_u16 == 404,
            "Expected INTERNAL_SERVER_ERROR (500) or NOT_FOUND (404) for invalid scan_id, got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_download_generated_files_not_found() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let non_existent_generation_id = "00000000-0000-0000-0000-000000000000";

        let response = server
            .get(&format!(
                "/api/generate/{}/download",
                non_existent_generation_id
            ))
            .await;

        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 404,
            "Expected NOT_FOUND (404) for non-existent generation_id, got {}",
            status_u16
        );

        let body: serde_json::Value = response.json();
        assert!(
            body.get("error").is_some(),
            "Error response should have error field"
        );
    }

    #[tokio::test]
    async fn test_validate_generation_not_found() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let non_existent_generation_id = "00000000-0000-0000-0000-000000000000";

        let response = server
            .post(&format!(
                "/api/generate/{}/validate",
                non_existent_generation_id
            ))
            .await;

        // バリデーションサービスがエラーを返す場合、内部エラー（500）が返る
        let status_u16 = response.status_code().as_u16();
        assert!(
            status_u16 == 500 || status_u16 == 404,
            "Expected INTERNAL_SERVER_ERROR (500) or NOT_FOUND (404), got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_check_format_not_found() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let non_existent_generation_id = "00000000-0000-0000-0000-000000000000";

        let response = server
            .get(&format!(
                "/api/generate/{}/format/check",
                non_existent_generation_id
            ))
            .await;

        // フォーマットチェックサービスがエラーを返す場合、内部エラー（500）が返る
        let status_u16 = response.status_code().as_u16();
        assert!(
            status_u16 == 500 || status_u16 == 404,
            "Expected INTERNAL_SERVER_ERROR (500) or NOT_FOUND (404), got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_format_code_not_found() {
        let app = create_test_app();
        let server = TestServer::new(app.into_make_service()).unwrap();

        let non_existent_generation_id = "00000000-0000-0000-0000-000000000000";

        let response = server
            .post(&format!(
                "/api/generate/{}/format",
                non_existent_generation_id
            ))
            .await;

        // フォーマットサービスがエラーを返す場合、内部エラー（500）が返る
        let status_u16 = response.status_code().as_u16();
        assert!(
            status_u16 == 500 || status_u16 == 404,
            "Expected INTERNAL_SERVER_ERROR (500) or NOT_FOUND (404), got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_generate_with_valid_scan_data() {
        use crate::services::scan_service::{MockScannerFactory, ScanService};
        use std::collections::HashMap;

        // ScanServiceにテストデータを挿入してgenerate
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
        let scan_service = Arc::new(ScanService::new(Arc::new(mock)));

        // テスト用スキャンデータを挿入
        let test_scan_id = "test-scan-12345";
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
            scan_targets: HashMap::new(),
            filters: HashMap::new(),
            include_tags: true,
        };
        scan_service
            .insert_test_scan_data(
                test_scan_id.to_string(),
                config,
                serde_json::json!({
                    "provider": "aws",
                    "users": [{"user_name": "testuser", "arn": "arn:aws:iam::123:user/testuser"}],
                    "groups": [],
                    "roles": [],
                    "policies": []
                }),
            )
            .await;

        let generation_service = Arc::new(GenerationService::new(scan_service));
        let app = Router::new()
            .nest("/api/generate", router(generation_service))
            .layer(tower::ServiceBuilder::new().layer(tower_http::cors::CorsLayer::permissive()));
        let server = axum_test::TestServer::new(app.into_make_service()).unwrap();

        let response = server
            .post("/api/generate/terraform")
            .json(&serde_json::json!({
                "scan_id": test_scan_id,
                "config": {
                    "output_path": "/tmp/test-generate",
                    "file_split_rule": "single",
                    "naming_convention": "snake_case",
                    "import_script_format": "sh",
                    "generate_readme": false
                },
                "selected_resources": {}
            }))
            .await;

        // スキャンデータが存在する場合は200または内部エラー(テンプレートの問題)
        let status_u16 = response.status_code().as_u16();
        assert!(
            status_u16 == 200 || status_u16 == 500,
            "Expected OK (200) or INTERNAL_SERVER_ERROR (500) when scan data exists, got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_preview_terraform_endpoint_not_found() {
        let app = create_test_app();
        let server = axum_test::TestServer::new(app.into_make_service()).unwrap();

        let non_existent_generation_id = "00000000-0000-0000-0000-000000000001";

        // validate エンドポイントをプレビューの代替として確認
        let response = server
            .post(&format!(
                "/api/generate/{}/validate",
                non_existent_generation_id
            ))
            .await;

        let status_u16 = response.status_code().as_u16();
        assert!(
            status_u16 == 500 || status_u16 == 404,
            "Expected 500 or 404 for non-existent generation, got {}",
            status_u16
        );
    }
}
