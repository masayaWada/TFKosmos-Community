use axum::{
    extract::{Path, State},
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::api::error::{ApiError, ErrorResponse};
use crate::models::ScanConfig;
use crate::services::config_management_service::ConfigManagementService;

pub fn router(service: Arc<ConfigManagementService>) -> Router {
    Router::new()
        .route("/export", post(export_config))
        .route("/import", post(import_config))
        .route("/saved", get(list_saved_configs))
        .route("/save/:name", post(save_config))
        .route("/load/:name", get(load_config))
        .route("/delete/:name", delete(delete_config))
        .with_state(service)
}

#[utoipa::path(
    post,
    path = "/api/config/export",
    tag = "config",
    request_body = ScanConfig,
    responses(
        (status = 200, description = "設定のJSON形式エクスポート成功", body = Value),
        (status = 500, description = "内部エラー", body = ErrorResponse),
    )
)]
pub(crate) async fn export_config(
    State(service): State<Arc<ConfigManagementService>>,
    Json(config): Json<ScanConfig>,
) -> Result<Json<Value>, ApiError> {
    let json = service
        .export_json(&config)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "format": "json", "content": json })))
}

#[utoipa::path(
    post,
    path = "/api/config/import",
    tag = "config",
    request_body = Value,
    responses(
        (status = 200, description = "設定のインポート成功", body = ScanConfig),
        (status = 400, description = "バリデーションエラー", body = ErrorResponse),
    )
)]
pub(crate) async fn import_config(
    State(service): State<Arc<ConfigManagementService>>,
    Json(body): Json<Value>,
) -> Result<Json<ScanConfig>, ApiError> {
    let content = body
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::Validation("Missing 'content' field".to_string()))?;

    let config = service
        .import_json(content)
        .map_err(|e| ApiError::Validation(format!("Invalid config: {}", e)))?;
    Ok(Json(config))
}

#[utoipa::path(
    get,
    path = "/api/config/saved",
    tag = "config",
    responses(
        (status = 200, description = "保存済み設定一覧", body = Value),
        (status = 500, description = "内部エラー", body = ErrorResponse),
    )
)]
pub(crate) async fn list_saved_configs(
    State(service): State<Arc<ConfigManagementService>>,
) -> Result<Json<Value>, ApiError> {
    let configs = service
        .list_saved_configs()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "configs": configs })))
}

#[utoipa::path(
    post,
    path = "/api/config/save/{name}",
    tag = "config",
    params(
        ("name" = String, Path, description = "設定名")
    ),
    request_body = ScanConfig,
    responses(
        (status = 200, description = "設定の保存成功", body = Value),
        (status = 500, description = "内部エラー", body = ErrorResponse),
    )
)]
pub(crate) async fn save_config(
    State(service): State<Arc<ConfigManagementService>>,
    Path(name): Path<String>,
    Json(config): Json<ScanConfig>,
) -> Result<Json<Value>, ApiError> {
    service
        .save_config(&name, &config)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "message": "Config saved", "name": name })))
}

#[utoipa::path(
    get,
    path = "/api/config/load/{name}",
    tag = "config",
    params(
        ("name" = String, Path, description = "設定名")
    ),
    responses(
        (status = 200, description = "設定の読み込み成功", body = ScanConfig),
        (status = 404, description = "設定が見つからない", body = ErrorResponse),
    )
)]
pub(crate) async fn load_config(
    State(service): State<Arc<ConfigManagementService>>,
    Path(name): Path<String>,
) -> Result<Json<ScanConfig>, ApiError> {
    let config = service
        .load_config(&name)
        .map_err(|e| ApiError::NotFound(format!("Config not found: {}", e)))?;
    Ok(Json(config))
}

#[utoipa::path(
    delete,
    path = "/api/config/delete/{name}",
    tag = "config",
    params(
        ("name" = String, Path, description = "設定名")
    ),
    responses(
        (status = 200, description = "設定の削除成功", body = Value),
        (status = 500, description = "内部エラー", body = ErrorResponse),
    )
)]
pub(crate) async fn delete_config(
    State(service): State<Arc<ConfigManagementService>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    service
        .delete_config(&name)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "message": "Config deleted", "name": name })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use serde_json::json;
    use tempfile::TempDir;
    use tower::ServiceBuilder;
    use tower_http::cors::CorsLayer;

    fn make_test_scan_config() -> serde_json::Value {
        json!({
            "provider": "aws",
            "account_id": "123456789012",
            "profile": "default",
            "subscription_id": null,
            "tenant_id": null,
            "auth_method": null,
            "service_principal_config": null,
            "scope_type": null,
            "scope_value": null,
            "scan_targets": {"users": true, "roles": true},
            "filters": {},
            "include_tags": true,
            "assume_role_arn": null,
            "assume_role_session_name": null
        })
    }

    fn create_test_app(tmp: &TempDir) -> TestServer {
        let service = Arc::new(ConfigManagementService::new(tmp.path().to_path_buf()));
        let app = Router::new()
            .nest("/api/config", router(service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()));
        TestServer::new(app.into_make_service()).unwrap()
    }

    #[tokio::test]
    async fn test_export_config() {
        let tmp = TempDir::new().unwrap();
        let server = create_test_app(&tmp);

        let response = server
            .post("/api/config/export")
            .json(&make_test_scan_config())
            .await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: Value = response.json();
        assert_eq!(body["format"], "json");
        assert!(body["content"].is_string());
    }

    #[tokio::test]
    async fn test_import_config_success() {
        let tmp = TempDir::new().unwrap();
        let server = create_test_app(&tmp);

        let content = serde_json::to_string(&make_test_scan_config()).unwrap();
        let response = server
            .post("/api/config/import")
            .json(&json!({ "content": content }))
            .await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: Value = response.json();
        assert_eq!(body["provider"], "aws");
    }

    #[tokio::test]
    async fn test_import_config_missing_content() {
        let tmp = TempDir::new().unwrap();
        let server = create_test_app(&tmp);

        let response = server.post("/api/config/import").json(&json!({})).await;

        assert_eq!(response.status_code().as_u16(), 400);
    }

    #[tokio::test]
    async fn test_list_saved_configs_empty() {
        let tmp = TempDir::new().unwrap();
        let server = create_test_app(&tmp);

        let response = server.get("/api/config/saved").await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: Value = response.json();
        assert!(body["configs"].is_array());
        assert_eq!(body["configs"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_save_and_load_config() {
        let tmp = TempDir::new().unwrap();
        let server = create_test_app(&tmp);

        let save_response = server
            .post("/api/config/save/my-config")
            .json(&make_test_scan_config())
            .await;
        assert_eq!(save_response.status_code().as_u16(), 200);

        let load_response = server.get("/api/config/load/my-config").await;
        assert_eq!(load_response.status_code().as_u16(), 200);
        let body: Value = load_response.json();
        assert_eq!(body["provider"], "aws");
    }

    #[tokio::test]
    async fn test_load_config_not_found() {
        let tmp = TempDir::new().unwrap();
        let server = create_test_app(&tmp);

        let response = server.get("/api/config/load/nonexistent").await;

        assert_eq!(response.status_code().as_u16(), 404);
    }

    #[tokio::test]
    async fn test_delete_config() {
        let tmp = TempDir::new().unwrap();
        let server = create_test_app(&tmp);

        server
            .post("/api/config/save/to-delete")
            .json(&make_test_scan_config())
            .await;

        let response = server.delete("/api/config/delete/to-delete").await;
        assert_eq!(response.status_code().as_u16(), 200);
        let body: Value = response.json();
        assert_eq!(body["message"], "Config deleted");
    }
}
