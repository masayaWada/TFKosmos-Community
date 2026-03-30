use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use std::sync::Arc;

use crate::services::audit_service::{AuditAction, AuditEntry, AuditService, AuditStatus};

/// 監査対象のパスとアクションのマッピング
fn classify_request(path: &str, method: &str) -> Option<AuditAction> {
    if method != "POST" && method != "DELETE" {
        return None;
    }

    if path.starts_with("/api/scan") && method == "POST" {
        Some(AuditAction::Scan)
    } else if path.starts_with("/api/generate") && method == "POST" {
        Some(AuditAction::Generate)
    } else if path.starts_with("/api/export") && method == "POST" {
        Some(AuditAction::Export)
    } else if path.starts_with("/api/drift") && method == "POST" {
        Some(AuditAction::DriftDetect)
    } else if path.starts_with("/api/config/save") && method == "POST" {
        Some(AuditAction::ConfigSave)
    } else if path.starts_with("/api/config/delete") && method == "DELETE" {
        Some(AuditAction::ConfigDelete)
    } else {
        None
    }
}

/// 監査ログミドルウェア
///
/// POST/DELETEリクエストのうち、scan/generate/export/drift/config操作を自動的にログに記録する。
pub async fn audit_middleware(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().to_string();

    // 監査対象か判定
    let action = classify_request(&path, &method);

    // AuditServiceの取得
    let audit_service = request.extensions().get::<Arc<AuditService>>().cloned();

    // リクエストを処理
    let response = next.run(request).await;

    // 監査対象の場合のみログ記録
    if let (Some(action), Some(service)) = (action, audit_service) {
        let status = if response.status().is_success() {
            AuditStatus::Success
        } else {
            AuditStatus::Failure
        };

        let entry =
            AuditEntry::new(action, status, &path, &method).with_details(serde_json::json!({
                "response_status": response.status().as_u16(),
            }));

        // 非同期でログ記録（レスポンスの遅延を防ぐ）
        tokio::spawn(async move {
            if let Err(e) = service.log_event(entry).await {
                tracing::warn!("Failed to write audit log: {}", e);
            }
        });
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_scan_request() {
        assert_eq!(
            classify_request("/api/scan/start", "POST"),
            Some(AuditAction::Scan)
        );
    }

    #[test]
    fn test_classify_generate_request() {
        assert_eq!(
            classify_request("/api/generate/abc123", "POST"),
            Some(AuditAction::Generate)
        );
    }

    #[test]
    fn test_classify_export_request() {
        assert_eq!(
            classify_request("/api/export/abc123", "POST"),
            Some(AuditAction::Export)
        );
    }

    #[test]
    fn test_classify_drift_request() {
        assert_eq!(
            classify_request("/api/drift/detect", "POST"),
            Some(AuditAction::DriftDetect)
        );
    }

    #[test]
    fn test_classify_config_save() {
        assert_eq!(
            classify_request("/api/config/save/myconfig", "POST"),
            Some(AuditAction::ConfigSave)
        );
    }

    #[test]
    fn test_classify_config_delete() {
        assert_eq!(
            classify_request("/api/config/delete/myconfig", "DELETE"),
            Some(AuditAction::ConfigDelete)
        );
    }

    #[test]
    fn test_classify_get_request_ignored() {
        assert_eq!(classify_request("/api/scan/abc123", "GET"), None);
    }

    #[test]
    fn test_classify_unrelated_path() {
        assert_eq!(classify_request("/api/connection/test", "POST"), None);
    }

    #[test]
    fn test_classify_put_request_ignored() {
        // PUT は監査対象外
        assert_eq!(classify_request("/api/scan/start", "PUT"), None);
    }

    #[test]
    fn test_classify_patch_request_ignored() {
        assert_eq!(classify_request("/api/generate/abc", "PATCH"), None);
    }

    #[test]
    fn test_classify_delete_request_for_non_config_ignored() {
        // DELETE でも config/delete 以外は None
        assert_eq!(classify_request("/api/scan/abc123", "DELETE"), None);
        assert_eq!(classify_request("/api/generate/abc123", "DELETE"), None);
    }

    #[test]
    fn test_classify_scan_with_query_params() {
        // クエリパラメータが付いたパスも正しく分類されるか
        assert_eq!(
            classify_request("/api/scan/start?stream=true", "POST"),
            Some(AuditAction::Scan)
        );
    }

    #[test]
    fn test_classify_drift_post() {
        assert_eq!(
            classify_request("/api/drift", "POST"),
            Some(AuditAction::DriftDetect)
        );
    }

    #[test]
    fn test_classify_export_post() {
        assert_eq!(
            classify_request("/api/export", "POST"),
            Some(AuditAction::Export)
        );
    }

    #[test]
    fn test_classify_config_save_post() {
        assert_eq!(
            classify_request("/api/config/save", "POST"),
            Some(AuditAction::ConfigSave)
        );
    }

    #[test]
    fn test_classify_config_delete_delete() {
        assert_eq!(
            classify_request("/api/config/delete", "DELETE"),
            Some(AuditAction::ConfigDelete)
        );
    }

    #[test]
    fn test_classify_config_save_get_ignored() {
        // GET の config/save は監査対象外
        assert_eq!(classify_request("/api/config/save/name", "GET"), None);
    }

    #[test]
    fn test_classify_root_path() {
        assert_eq!(classify_request("/", "POST"), None);
    }

    #[test]
    fn test_classify_empty_path() {
        assert_eq!(classify_request("", "POST"), None);
    }

    #[tokio::test]
    async fn test_audit_middleware_logs_request() {
        use axum::middleware;
        use axum::routing::post;
        use axum::Router;
        use axum_test::TestServer;

        let audit_service = Arc::new(crate::services::audit_service::AuditService::new(
            std::env::temp_dir(),
        ));

        let app = Router::new()
            .route(
                "/api/scan/start",
                post(|| async { axum::http::StatusCode::OK }),
            )
            .layer(middleware::from_fn(audit_middleware))
            .layer(axum::Extension(audit_service));

        let server = TestServer::new(app.into_make_service()).unwrap();
        let response = server.post("/api/scan/start").await;
        assert_eq!(response.status_code().as_u16(), 200);
    }
}
