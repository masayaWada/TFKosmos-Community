use axum::{
    extract::{Query, State},
    response::Json,
    routing::get,
    Router,
};
use serde_json::Value;
use std::sync::Arc;

use crate::api::error::{ApiError, ErrorResponse};
use crate::services::audit_service::{AuditQuery, AuditService};

pub fn router(service: Arc<AuditService>) -> Router {
    Router::new()
        .route("/", get(list_audit_logs))
        .with_state(service)
}

#[utoipa::path(
    get,
    path = "/api/audit",
    tag = "audit",
    params(
        ("from" = Option<String>, Query, description = "開始日時（ISO 8601形式）"),
        ("to" = Option<String>, Query, description = "終了日時（ISO 8601形式）"),
        ("action" = Option<String>, Query, description = "アクション種別フィルタ"),
        ("limit" = Option<usize>, Query, description = "最大件数（デフォルト100）"),
    ),
    responses(
        (status = 200, description = "監査ログ一覧", body = Value),
        (status = 500, description = "内部エラー", body = ErrorResponse),
    )
)]
pub(crate) async fn list_audit_logs(
    State(service): State<Arc<AuditService>>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Value>, ApiError> {
    let entries = service
        .query_events(&query)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "entries": entries,
        "count": entries.len(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::audit_service::{AuditAction, AuditEntry, AuditStatus};
    use axum_test::TestServer;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tower::ServiceBuilder;
    use tower_http::cors::CorsLayer;

    fn create_test_app(tmp: &TempDir) -> TestServer {
        let service = Arc::new(AuditService::new(tmp.path().to_path_buf()));
        let app = axum::Router::new()
            .nest("/api/audit", router(service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()));
        TestServer::new(app.into_make_service()).unwrap()
    }

    fn create_test_app_with_service(service: Arc<AuditService>) -> TestServer {
        let app = axum::Router::new()
            .nest("/api/audit", router(service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()));
        TestServer::new(app.into_make_service()).unwrap()
    }

    #[tokio::test]
    async fn test_list_audit_logs_empty() {
        let tmp = TempDir::new().unwrap();
        let server = create_test_app(&tmp);

        let response = server.get("/api/audit").await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: serde_json::Value = response.json();
        assert_eq!(body["count"], 0);
        assert!(body["entries"].is_array());
        assert_eq!(body["entries"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_list_audit_logs_with_data() {
        let tmp = TempDir::new().unwrap();
        let service = Arc::new(AuditService::new(tmp.path().to_path_buf()));

        service
            .log_event(AuditEntry::new(
                AuditAction::Scan,
                AuditStatus::Success,
                "/api/scan/start",
                "POST",
            ))
            .await
            .unwrap();

        let server = create_test_app_with_service(service);
        let response = server.get("/api/audit").await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: serde_json::Value = response.json();
        assert!(body["count"].as_u64().unwrap() > 0);
        assert!(!body["entries"].as_array().unwrap().is_empty());
    }
}
