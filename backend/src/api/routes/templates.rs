use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::api::error::ApiError;
use crate::api::validation::validate_template_name;
use crate::models::TemplateValidationResponse;
use crate::services::template_service::TemplateService;

pub fn router(service: Arc<TemplateService>) -> Router {
    Router::new()
        .route("/", get(list_templates))
        // Use wildcard pattern to handle template names with slashes (e.g., aws/cleanup_access_key.tf.j2)
        // Note: preview and validate routes use a different path structure since catch-all must be at the end
        .route("/preview/*template_name", post(preview_template))
        .route("/validate/*template_name", post(validate_template))
        .route("/*template_name", get(get_template))
        .route("/*template_name", put(update_template))
        .route("/*template_name", post(create_template))
        .route("/*template_name", delete(delete_template))
        .with_state(service)
}

async fn list_templates(
    State(service): State<Arc<TemplateService>>,
) -> Result<Json<Value>, ApiError> {
    service
        .list_templates()
        .await
        .map(|templates| Json(json!({ "templates": templates })))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(Deserialize)]
struct GetTemplateQuery {
    source: Option<String>,
}

async fn get_template(
    State(service): State<Arc<TemplateService>>,
    Path(template_name): Path<String>,
    Query(params): Query<GetTemplateQuery>,
) -> Result<Json<Value>, ApiError> {
    validate_template_name(&template_name)?;
    service
        .get_template(&template_name, params.source.as_deref())
        .await
        .map(Json)
        .map_err(|e| ApiError::NotFound(format!("Template '{}' not found: {}", template_name, e)))
}

#[derive(serde::Deserialize)]
struct CreateTemplateRequest {
    content: String,
}

async fn create_template(
    State(service): State<Arc<TemplateService>>,
    Path(template_name): Path<String>,
    Json(request): Json<CreateTemplateRequest>,
) -> Result<Json<Value>, ApiError> {
    validate_template_name(&template_name)?;
    service
        .create_template(&template_name, &request.content)
        .await
        .map(|_| Json(json!({ "message": "Template created successfully" })))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

async fn update_template(
    State(service): State<Arc<TemplateService>>,
    Path(template_name): Path<String>,
    Json(request): Json<CreateTemplateRequest>,
) -> Result<Json<Value>, ApiError> {
    validate_template_name(&template_name)?;
    service
        .create_template(&template_name, &request.content)
        .await
        .map(|_| Json(json!({ "message": "Template updated successfully" })))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

async fn delete_template(
    State(service): State<Arc<TemplateService>>,
    Path(template_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    validate_template_name(&template_name)?;
    service
        .delete_template(&template_name)
        .await
        .map(|_| Json(json!({ "message": "Template deleted successfully" })))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(serde::Deserialize)]
struct PreviewTemplateRequest {
    content: String,
    #[serde(default)]
    context: Option<serde_json::Value>,
}

async fn preview_template(
    State(service): State<Arc<TemplateService>>,
    Path(template_name): Path<String>,
    Json(request): Json<PreviewTemplateRequest>,
) -> Result<Json<Value>, ApiError> {
    validate_template_name(&template_name)?;
    service
        .preview_template(&template_name, &request.content, request.context)
        .await
        .map(|preview| Json(json!({ "preview": preview })))
        .map_err(|e| ApiError::Validation(format!("Template preview failed: {}", e)))
}

#[derive(serde::Deserialize)]
struct ValidateTemplateRequest {
    content: String,
}

async fn validate_template(
    State(service): State<Arc<TemplateService>>,
    Path(template_name): Path<String>,
    Json(request): Json<ValidateTemplateRequest>,
) -> Result<Json<TemplateValidationResponse>, ApiError> {
    validate_template_name(&template_name)?;
    service
        .validate_template(&template_name, &request.content)
        .await
        .map(Json)
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tower::ServiceBuilder;
    use tower_http::cors::CorsLayer;

    fn create_test_app(base_dir: PathBuf) -> axum_test::TestServer {
        let service = Arc::new(TemplateService::new(base_dir));
        let app = Router::new()
            .nest("/api/templates", router(service))
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()));
        TestServer::new(app.into_make_service()).unwrap()
    }

    #[tokio::test]
    async fn test_list_templates_endpoint() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let default_template_dir = temp_dir.path().join("templates_default/terraform/aws");
        fs::create_dir_all(&default_template_dir).unwrap();
        fs::write(
            default_template_dir.join("iam_user.tf.j2"),
            "default template",
        )
        .unwrap();

        let server = create_test_app(temp_dir.path().to_path_buf());

        // Act
        let response = server.get("/api/templates").await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "List templates endpoint should return OK (200), got {}",
            status_u16
        );
        let body: serde_json::Value = response.json();
        assert!(
            body.get("templates").is_some(),
            "Response should have templates field"
        );
        let templates = body.get("templates").unwrap().as_array().unwrap();
        assert!(!templates.is_empty(), "Should have at least one template");
    }

    #[tokio::test]
    async fn test_get_template_endpoint() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform");
        fs::create_dir_all(&user_template_dir).unwrap();

        let template_name = "test_template.tf.j2";
        let template_content = "test content";
        fs::write(user_template_dir.join(template_name), template_content).unwrap();

        let server = create_test_app(temp_dir.path().to_path_buf());

        // Act
        let encoded_name = urlencoding::encode(template_name);
        let response = server
            .get(&format!("/api/templates/{}?source=user", encoded_name))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "Get template endpoint should return OK (200), got {}",
            status_u16
        );
        let body: serde_json::Value = response.json();
        assert_eq!(
            body.get("source").and_then(|v| v.as_str()),
            Some("user"),
            "Source should be user"
        );
        assert_eq!(
            body.get("content").and_then(|v| v.as_str()),
            Some(template_content),
            "Content should match"
        );
    }

    #[tokio::test]
    async fn test_get_template_not_found() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_app(temp_dir.path().to_path_buf());

        // Act
        let encoded_name = urlencoding::encode("nonexistent_template.tf.j2");
        let response = server
            .get(&format!("/api/templates/{}", encoded_name))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 404,
            "Get template endpoint should return NOT_FOUND (404), got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_create_template_endpoint() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform");
        fs::create_dir_all(&user_template_dir).unwrap();

        let server = create_test_app(temp_dir.path().to_path_buf());

        let template_name = "new_template.tf.j2";
        let template_content = "new template content";

        // Act
        let encoded_name = urlencoding::encode(template_name);
        let response = server
            .post(&format!("/api/templates/{}", encoded_name))
            .json(&json!({
                "content": template_content
            }))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "Create template endpoint should return OK (200), got {}",
            status_u16
        );
        let body: serde_json::Value = response.json();
        assert!(
            body.get("message").is_some(),
            "Response should have message field"
        );

        // テンプレートファイルが作成されたことを確認
        let template_path = user_template_dir.join(template_name);
        assert!(template_path.exists(), "Template file should exist");
        let saved_content = fs::read_to_string(&template_path).unwrap();
        assert_eq!(
            saved_content, template_content,
            "Saved content should match"
        );
    }

    #[tokio::test]
    async fn test_update_template_endpoint() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform");
        fs::create_dir_all(&user_template_dir).unwrap();

        let template_name = "existing_template.tf.j2";
        fs::write(user_template_dir.join(template_name), "old content").unwrap();

        let server = create_test_app(temp_dir.path().to_path_buf());
        let new_content = "updated content";

        // Act
        let encoded_name = urlencoding::encode(template_name);
        let response = server
            .put(&format!("/api/templates/{}", encoded_name))
            .json(&json!({
                "content": new_content
            }))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "Update template endpoint should return OK (200), got {}",
            status_u16
        );

        // テンプレートファイルが更新されたことを確認
        let template_path = user_template_dir.join(template_name);
        let saved_content = fs::read_to_string(&template_path).unwrap();
        assert_eq!(saved_content, new_content, "Content should be updated");
    }

    #[tokio::test]
    async fn test_delete_template_endpoint() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform");
        fs::create_dir_all(&user_template_dir).unwrap();

        let template_name = "template_to_delete.tf.j2";
        let template_path = user_template_dir.join(template_name);
        fs::write(&template_path, "content").unwrap();

        let server = create_test_app(temp_dir.path().to_path_buf());

        // Act
        let encoded_name = urlencoding::encode(template_name);
        let response = server
            .delete(&format!("/api/templates/{}", encoded_name))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "Delete template endpoint should return OK (200), got {}",
            status_u16
        );
        let body: serde_json::Value = response.json();
        assert!(
            body.get("message").is_some(),
            "Response should have message field"
        );

        // テンプレートファイルが削除されたことを確認
        assert!(!template_path.exists(), "Template file should be deleted");
    }

    #[tokio::test]
    async fn test_delete_template_not_found() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_app(temp_dir.path().to_path_buf());

        // Act
        let encoded_name = urlencoding::encode("nonexistent_template.tf.j2");
        let response = server
            .delete(&format!("/api/templates/{}", encoded_name))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 500,
            "Delete template endpoint should return INTERNAL_SERVER_ERROR (500), got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_preview_template_endpoint() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_app(temp_dir.path().to_path_buf());

        let template_name = "iam_user.tf.j2";
        let template_content = r#"resource "aws_iam_user" "{{ resource_name }}" {
  name = "{{ user.user_name }}"
}"#;

        // Act
        let encoded_name = urlencoding::encode(template_name);
        let response = server
            .post(&format!("/api/templates/preview/{}", encoded_name))
            .json(&json!({
                "content": template_content,
                "context": {
                    "resource_name": "test_user",
                    "user": {
                        "user_name": "test-user"
                    }
                }
            }))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "Preview template endpoint should return OK (200), got {}",
            status_u16
        );
        let body: serde_json::Value = response.json();
        assert!(
            body.get("preview").is_some(),
            "Response should have preview field"
        );
        let preview = body.get("preview").unwrap().as_str().unwrap();
        assert!(
            preview.contains("test_user"),
            "Preview should contain resource_name"
        );
        assert!(
            preview.contains("test-user"),
            "Preview should contain user_name"
        );
    }

    #[tokio::test]
    async fn test_validate_template_endpoint_valid() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_app(temp_dir.path().to_path_buf());

        let template_name = "iam_user.tf.j2";
        let template_content = r#"resource "aws_iam_user" "{{ resource_name }}" {
  name = "{{ user.user_name }}"
}"#;

        // Act
        let encoded_name = urlencoding::encode(template_name);
        let response = server
            .post(&format!("/api/templates/validate/{}", encoded_name))
            .json(&json!({
                "content": template_content
            }))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "Validate template endpoint should return OK (200), got {}",
            status_u16
        );
        let body: serde_json::Value = response.json();
        assert_eq!(
            body.get("valid").and_then(|v| v.as_bool()),
            Some(true),
            "Template should be valid"
        );
        assert_eq!(
            body.get("errors").and_then(|v| v.as_array()).unwrap().len(),
            0,
            "Should have no errors"
        );
    }

    #[tokio::test]
    async fn test_get_template_path_traversal_rejected() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_app(temp_dir.path().to_path_buf());

        let response = server.get("/api/templates/..%2F..%2Fetc%2Fpasswd").await;

        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 400,
            "Path traversal attempt should return 400 Bad Request, got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_create_template_path_traversal_rejected() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_app(temp_dir.path().to_path_buf());

        let response = server
            .post("/api/templates/..%2Fmalicious.tf.j2")
            .json(&json!({ "content": "malicious content" }))
            .await;

        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 400,
            "Path traversal attempt should return 400 Bad Request, got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_delete_template_path_traversal_rejected() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_app(temp_dir.path().to_path_buf());

        let response = server
            .delete("/api/templates/..%2F..%2Fimportant_file")
            .await;

        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 400,
            "Path traversal attempt should return 400 Bad Request, got {}",
            status_u16
        );
    }

    #[tokio::test]
    async fn test_validate_template_endpoint_invalid() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_app(temp_dir.path().to_path_buf());

        let template_name = "iam_user.tf.j2";
        let template_content = r#"resource "aws_iam_user" "{{ resource_name" {
  name = "{{ user.user_name }}"
}"#; // 閉じ括弧がない

        // Act
        let encoded_name = urlencoding::encode(template_name);
        let response = server
            .post(&format!("/api/templates/validate/{}", encoded_name))
            .json(&json!({
                "content": template_content
            }))
            .await;

        // Assert
        let status_u16 = response.status_code().as_u16();
        assert_eq!(
            status_u16, 200,
            "Validate template endpoint should return OK (200), got {}",
            status_u16
        );
        let body: serde_json::Value = response.json();
        assert_eq!(
            body.get("valid").and_then(|v| v.as_bool()),
            Some(false),
            "Template should be invalid"
        );
        assert!(
            !body
                .get("errors")
                .and_then(|v| v.as_array())
                .unwrap()
                .is_empty(),
            "Should have errors"
        );
    }
}
