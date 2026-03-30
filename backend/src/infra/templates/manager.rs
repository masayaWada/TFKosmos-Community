use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct TemplateManager;

impl TemplateManager {
    fn get_template_base_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // Get current working directory
        if let Ok(current_dir) = std::env::current_dir() {
            tracing::debug!(current_dir = ?current_dir, "Current working directory");

            // Try relative to current directory
            paths.push(current_dir.join("templates_user/terraform"));
            paths.push(current_dir.join("templates_default/terraform"));

            // Try relative to backend directory (if running from project root)
            paths.push(current_dir.join("backend/templates_user/terraform"));
            paths.push(current_dir.join("backend/templates_default/terraform"));

            // Try relative to executable location
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    tracing::debug!(exe_dir = ?exe_dir, "Executable directory");
                    // Go up to project root if running from target/debug or target/release
                    if exe_dir.ends_with("target/debug") || exe_dir.ends_with("target/release") {
                        if let Some(backend_dir) = exe_dir.parent().and_then(|p| p.parent()) {
                            paths.push(backend_dir.join("templates_user/terraform"));
                            paths.push(backend_dir.join("templates_default/terraform"));
                        }
                    }
                }
            }
        }

        // Also try absolute paths from common locations
        paths.push(PathBuf::from("templates_user/terraform"));
        paths.push(PathBuf::from("templates_default/terraform"));
        paths.push(PathBuf::from("backend/templates_user/terraform"));
        paths.push(PathBuf::from("backend/templates_default/terraform"));

        paths
    }

    pub async fn load_template(template_name: &str) -> Result<String> {
        tracing::debug!(template_name = %template_name, "Loading template");

        let base_paths = Self::get_template_base_paths();

        // Try user templates first, then default templates
        for base_path in &base_paths {
            // Check if this is a user template path
            if base_path.to_string_lossy().contains("templates_user") {
                let user_path = base_path.join(template_name);
                if user_path.exists() {
                    tracing::debug!(user_path = ?user_path, "Found template at user path");
                    let content = std::fs::read_to_string(&user_path).with_context(|| {
                        format!("Failed to read template from user path: {:?}", user_path)
                    })?;
                    tracing::debug!(bytes = content.len(), "Template loaded successfully");
                    return Ok(content);
                }
            }

            // Try default template path
            if base_path.to_string_lossy().contains("templates_default") {
                let default_path = base_path.join(template_name);
                if default_path.exists() {
                    tracing::debug!(default_path = ?default_path, "Found template at default path");
                    let content = std::fs::read_to_string(&default_path).with_context(|| {
                        format!(
                            "Failed to read template from default path: {:?}",
                            default_path
                        )
                    })?;
                    tracing::debug!(bytes = content.len(), "Template loaded successfully");
                    return Ok(content);
                }
            }
        }

        // If we get here, template was not found
        let searched_paths: Vec<String> = base_paths
            .iter()
            .map(|p| format!("  - {:?}/{}", p, template_name))
            .collect();

        Err(anyhow::anyhow!(
            "Template not found: {}\n\
            Searched paths:\n{}\n\
            Please ensure the template file exists.",
            template_name,
            searched_paths.join("\n")
        ))
    }

    pub async fn render_template(
        template_name: &str,
        context: &serde_json::Value,
    ) -> Result<String> {
        tracing::debug!(template_name = %template_name, "Rendering template");
        let template_content = Self::load_template(template_name).await?;

        // Use minijinja to render template
        let mut env = minijinja::Environment::new();
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);
        env.add_template(template_name, &template_content)
            .with_context(|| {
                format!("Failed to add template '{}' to environment", template_name)
            })?;

        let template = env.get_template(template_name).with_context(|| {
            format!(
                "Failed to get template '{}' from environment",
                template_name
            )
        })?;

        let rendered = template.render(context).with_context(|| {
            format!("Failed to render template '{}' with context", template_name)
        })?;

        tracing::debug!(bytes = rendered.len(), "Template rendered successfully");
        Ok(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::OnceLock;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    /// set_current_dir はプロセスグローバルなため、並行テスト実行時の競合を防ぐ
    static DIR_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn dir_lock() -> &'static Mutex<()> {
        DIR_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[tokio::test]
    async fn test_load_template_from_user_path() {
        let _lock = dir_lock().lock().await;
        // Arrange: 一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform");
        fs::create_dir_all(&user_template_dir).unwrap();

        let template_name = "test_template.tf.j2";
        let template_content = "resource \"aws_iam_user\" \"{{ resource_name }}\" {}";
        let template_path = user_template_dir.join(template_name);
        fs::write(&template_path, template_content).unwrap();

        // カレントディレクトリを一時ディレクトリに変更
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Act
        let result = TemplateManager::load_template(template_name).await;

        // 元のディレクトリに戻す
        std::env::set_current_dir(original_dir).unwrap();

        // Assert
        assert!(result.is_ok(), "Template should be loaded successfully");
        assert_eq!(result.unwrap(), template_content);
    }

    #[tokio::test]
    async fn test_load_template_from_default_path() {
        let _lock = dir_lock().lock().await;
        // Arrange: 一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let default_template_dir = temp_dir.path().join("templates_default/terraform");
        fs::create_dir_all(&default_template_dir).unwrap();

        let template_name = "test_template.tf.j2";
        let template_content = "resource \"aws_iam_user\" \"{{ resource_name }}\" {}";
        let template_path = default_template_dir.join(template_name);
        fs::write(&template_path, template_content).unwrap();

        // カレントディレクトリを一時ディレクトリに変更
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Act
        let result = TemplateManager::load_template(template_name).await;

        // 元のディレクトリに戻す
        std::env::set_current_dir(original_dir).unwrap();

        // Assert
        assert!(result.is_ok(), "Template should be loaded successfully");
        assert_eq!(result.unwrap(), template_content);
    }

    #[tokio::test]
    async fn test_load_template_user_overrides_default() {
        let _lock = dir_lock().lock().await;
        // Arrange: 一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform");
        let default_template_dir = temp_dir.path().join("templates_default/terraform");
        fs::create_dir_all(&user_template_dir).unwrap();
        fs::create_dir_all(&default_template_dir).unwrap();

        let template_name = "test_template.tf.j2";
        let user_content = "user template content";
        let default_content = "default template content";

        fs::write(user_template_dir.join(template_name), user_content).unwrap();
        fs::write(default_template_dir.join(template_name), default_content).unwrap();

        // カレントディレクトリを一時ディレクトリに変更
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Act
        let result = TemplateManager::load_template(template_name).await;

        // 元のディレクトリに戻す
        std::env::set_current_dir(original_dir).unwrap();

        // Assert: ユーザーテンプレートが優先される
        assert!(result.is_ok(), "Template should be loaded successfully");
        assert_eq!(result.unwrap(), user_content);
    }

    #[tokio::test]
    async fn test_load_template_not_found() {
        let _lock = dir_lock().lock().await;
        // Arrange: テンプレートが存在しない一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Act
        let result = TemplateManager::load_template("nonexistent_template.tf.j2").await;

        // 元のディレクトリに戻す
        std::env::set_current_dir(original_dir).unwrap();

        // Assert
        assert!(result.is_err(), "Template should not be found");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Template not found"),
            "Error message should indicate template not found"
        );
    }

    #[tokio::test]
    async fn test_load_template_with_subdirectory() {
        let _lock = dir_lock().lock().await;
        // Arrange: サブディレクトリを含むテンプレートパス
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform/aws");
        fs::create_dir_all(&user_template_dir).unwrap();

        let template_name = "aws/iam_user.tf.j2";
        let template_content = "resource \"aws_iam_user\" \"{{ resource_name }}\" {}";
        let template_path = user_template_dir.join("iam_user.tf.j2");
        fs::write(&template_path, template_content).unwrap();

        // カレントディレクトリを一時ディレクトリに変更
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Act
        let result = TemplateManager::load_template(template_name).await;

        // 元のディレクトリに戻す
        std::env::set_current_dir(original_dir).unwrap();

        // Assert
        assert!(result.is_ok(), "Template should be loaded successfully");
        assert_eq!(result.unwrap(), template_content);
    }

    #[tokio::test]
    async fn test_render_template() {
        let _lock = dir_lock().lock().await;
        // Arrange: 一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform");
        fs::create_dir_all(&user_template_dir).unwrap();

        let template_name = "test_template.tf.j2";
        let template_content = r#"resource "aws_iam_user" "{{ resource_name }}" {
  name = "{{ user.user_name }}"
}"#;
        let template_path = user_template_dir.join(template_name);
        fs::write(&template_path, template_content).unwrap();

        let context = serde_json::json!({
            "resource_name": "test_user",
            "user": {
                "user_name": "test-user"
            }
        });

        // カレントディレクトリを一時ディレクトリに変更
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Act
        let result = TemplateManager::render_template(template_name, &context).await;

        // 元のディレクトリに戻す
        std::env::set_current_dir(original_dir).unwrap();

        // Assert
        assert!(result.is_ok(), "Template should be rendered successfully");
        let rendered = result.unwrap();
        assert!(
            rendered.contains("test_user"),
            "Rendered template should contain resource_name"
        );
        assert!(
            rendered.contains("test-user"),
            "Rendered template should contain user_name"
        );
    }

    #[tokio::test]
    async fn test_render_template_with_invalid_syntax() {
        let _lock = dir_lock().lock().await;
        // Arrange: 一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let user_template_dir = temp_dir.path().join("templates_user/terraform");
        fs::create_dir_all(&user_template_dir).unwrap();

        let template_name = "test_template.tf.j2";
        let template_content = r#"resource "aws_iam_user" "{{ resource_name" {
  name = "{{ user.user_name }}"
}"#; // 閉じ括弧がない
        let template_path = user_template_dir.join(template_name);
        fs::write(&template_path, template_content).unwrap();

        let context = serde_json::json!({
            "resource_name": "test_user",
            "user": {
                "user_name": "test-user"
            }
        });

        // カレントディレクトリを一時ディレクトリに変更
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Act
        let result = TemplateManager::render_template(template_name, &context).await;

        // 元のディレクトリに戻す
        std::env::set_current_dir(original_dir).unwrap();

        // Assert
        assert!(result.is_err(), "Template with invalid syntax should fail");
    }
}
