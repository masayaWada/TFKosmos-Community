use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::api::error::ApiError;
use crate::models::{DependencyGraph, ResourceListResponse};
use crate::services::dependency_service::DependencyService;
use crate::services::resource_service::{ResourceError, ResourceService};

pub fn router(service: Arc<ResourceService>) -> Router {
    Router::new()
        .route("/:scan_id", get(get_resources))
        .route("/:scan_id/query", post(query_resources))
        .route("/:scan_id/dependencies", get(get_dependencies))
        .route("/:scan_id/select", post(select_resources))
        .route("/:scan_id/select", get(get_selected_resources))
        .with_state(service)
}

#[derive(Deserialize)]
struct GetResourcesQuery {
    #[serde(rename = "type")]
    resource_type: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    filter: Option<String>,
}

async fn get_resources(
    State(service): State<Arc<ResourceService>>,
    Path(scan_id): Path<String>,
    Query(params): Query<GetResourcesQuery>,
) -> Result<Json<ResourceListResponse>, ApiError> {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(50);

    let filter_conditions = if let Some(filter_str) = params.filter {
        serde_json::from_str(&filter_str).ok()
    } else {
        None
    };

    service
        .get_resources(
            &scan_id,
            params.resource_type.as_deref(),
            page,
            page_size,
            filter_conditions,
        )
        .await
        .map(Json)
        .map_err(|e| match e {
            ResourceError::ScanNotFound(_) => {
                ApiError::NotFound(format!("Scan with ID '{}' not found", scan_id))
            }
            _ => ApiError::Internal(e.to_string()),
        })
}

#[derive(Deserialize)]
struct SelectResourcesRequest {
    selections: std::collections::HashMap<String, Vec<serde_json::Value>>,
}

async fn select_resources(
    State(service): State<Arc<ResourceService>>,
    Path(scan_id): Path<String>,
    Json(request): Json<SelectResourcesRequest>,
) -> Result<Json<Value>, ApiError> {
    service
        .update_selection(&scan_id, request.selections)
        .await
        .map(|result| Json(json!(result)))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

async fn get_selected_resources(
    State(service): State<Arc<ResourceService>>,
    Path(scan_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    service
        .get_selection(&scan_id)
        .await
        .map(|selections| {
            Json(json!({
                "selections": selections
            }))
        })
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(Deserialize)]
struct QueryResourcesRequest {
    query: String,
    #[serde(rename = "type")]
    resource_type: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn query_resources(
    State(service): State<Arc<ResourceService>>,
    Path(scan_id): Path<String>,
    Json(request): Json<QueryResourcesRequest>,
) -> Result<Json<ResourceListResponse>, ApiError> {
    let page = request.page.unwrap_or(1);
    let page_size = request.page_size.unwrap_or(50);

    service
        .query_resources(
            &scan_id,
            &request.query,
            request.resource_type.as_deref(),
            page,
            page_size,
        )
        .await
        .map(Json)
        .map_err(|e| match e {
            ResourceError::ScanNotFound(msg) => ApiError::NotFound(msg),
            ResourceError::QuerySyntaxError(msg) | ResourceError::QueryParseError(msg) => {
                ApiError::Validation(msg)
            }
            _ => ApiError::Internal(e.to_string()),
        })
}

#[derive(Deserialize)]
struct GetDependenciesQuery {
    root_id: Option<String>,
}

async fn get_dependencies(
    State(service): State<Arc<ResourceService>>,
    Path(scan_id): Path<String>,
    Query(params): Query<GetDependenciesQuery>,
) -> Result<Json<DependencyGraph>, ApiError> {
    DependencyService::get_dependencies(service.scan_service(), &scan_id, params.root_id.as_deref())
        .await
        .map(Json)
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") {
                ApiError::NotFound(msg)
            } else {
                ApiError::Internal(msg)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use serde_json::json;

    use crate::models::ScanConfig;
    use crate::services::scan_service::{RealScannerFactory, ScanService};

    fn make_scan_config() -> ScanConfig {
        ScanConfig {
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
            scan_targets: Default::default(),
            filters: Default::default(),
            include_tags: true,
        }
    }

    fn create_test_services() -> (Arc<ScanService>, Arc<ResourceService>) {
        let scan_service = Arc::new(ScanService::new(Arc::new(RealScannerFactory::new())));
        let resource_service = Arc::new(ResourceService::new(scan_service.clone()));
        (scan_service, resource_service)
    }

    fn create_test_app(resource_service: Arc<ResourceService>) -> TestServer {
        let app = Router::new().nest("/api/resources", router(resource_service));
        TestServer::new(app.into_make_service()).unwrap()
    }

    #[tokio::test]
    async fn test_get_resources_not_found() {
        let (_, resource_service) = create_test_services();
        let server = create_test_app(resource_service);

        let response = server.get("/api/resources/nonexistent-scan-id").await;

        assert_eq!(
            response.status_code().as_u16(),
            404,
            "存在しないscan_idは404を返すべき"
        );
    }

    #[tokio::test]
    async fn test_get_resources_success() {
        let (scan_service, resource_service) = create_test_services();
        let scan_id = "test-resources-scan-001";
        let scan_data = json!({
            "provider": "aws",
            "users": [
                {"name": "alice", "arn": "arn:aws:iam::123:user/alice"},
                {"name": "bob", "arn": "arn:aws:iam::123:user/bob"}
            ]
        });
        scan_service
            .insert_test_scan_data(scan_id.to_string(), make_scan_config(), scan_data)
            .await;

        let server = create_test_app(resource_service);
        let response = server.get(&format!("/api/resources/{}", scan_id)).await;

        assert_eq!(
            response.status_code().as_u16(),
            200,
            "存在するscan_idは200を返すべき"
        );
        let body: serde_json::Value = response.json();
        assert!(body.get("resources").is_some());
        assert!(body.get("total").is_some());
    }

    #[tokio::test]
    async fn test_get_resources_with_type_filter() {
        let (scan_service, resource_service) = create_test_services();
        let scan_id = "test-resources-scan-002";
        let scan_data = json!({
            "provider": "aws",
            "users": [
                {"name": "alice", "arn": "arn:aws:iam::123:user/alice"}
            ],
            "roles": [
                {"name": "AdminRole", "arn": "arn:aws:iam::123:role/AdminRole"}
            ]
        });
        scan_service
            .insert_test_scan_data(scan_id.to_string(), make_scan_config(), scan_data)
            .await;

        let server = create_test_app(resource_service);
        let response = server
            .get(&format!("/api/resources/{}?type=users", scan_id))
            .await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: serde_json::Value = response.json();
        let total = body.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        assert_eq!(total, 1, "usersフィルタでは1件のみ返すべき");
    }

    #[tokio::test]
    async fn test_query_resources_not_found() {
        let (_, resource_service) = create_test_services();
        let server = create_test_app(resource_service);

        let response = server
            .post("/api/resources/nonexistent-scan/query")
            .json(&json!({"query": "name == \"alice\""}))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            404,
            "存在しないscan_idへのqueryは404を返すべき"
        );
    }

    #[tokio::test]
    async fn test_query_resources_invalid_syntax() {
        let (scan_service, resource_service) = create_test_services();
        let scan_id = "test-resources-scan-003";
        let scan_data = json!({
            "provider": "aws",
            "users": [{"name": "alice"}]
        });
        scan_service
            .insert_test_scan_data(scan_id.to_string(), make_scan_config(), scan_data)
            .await;

        let server = create_test_app(resource_service);
        let response = server
            .post(&format!("/api/resources/{}/query", scan_id))
            .json(&json!({"query": "name === invalid @@@"}))
            .await;

        assert_eq!(
            response.status_code().as_u16(),
            400,
            "不正なクエリ構文は400を返すべき"
        );
    }

    #[tokio::test]
    async fn test_select_and_get_resources() {
        let (_, resource_service) = create_test_services();
        let scan_id = "test-resources-scan-004";
        let server = create_test_app(resource_service);

        // 選択を保存
        let response = server
            .post(&format!("/api/resources/{}/select", scan_id))
            .json(&json!({
                "selections": {
                    "users": ["alice", "bob"]
                }
            }))
            .await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: serde_json::Value = response.json();
        assert_eq!(
            body.get("selected_count").and_then(|v| v.as_u64()),
            Some(2),
            "selected_countは2であるべき"
        );

        // 選択を取得
        let response = server
            .get(&format!("/api/resources/{}/select", scan_id))
            .await;

        assert_eq!(response.status_code().as_u16(), 200);
        let body: serde_json::Value = response.json();
        assert!(body.get("selections").is_some());
    }
}
