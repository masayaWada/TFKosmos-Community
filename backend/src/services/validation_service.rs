use anyhow::Result;
use std::path::PathBuf;

use crate::infra::terraform::{FormatResult, TerraformCli, TerraformVersion, ValidationResult};

pub struct ValidationService;

impl ValidationService {
    /// Terraform CLIの利用可能性をチェック
    pub fn check_terraform() -> TerraformVersion {
        TerraformCli::version().unwrap_or(TerraformVersion {
            version: String::new(),
            available: false,
        })
    }

    /// 生成されたTerraformコードを検証（output_pathを直接受け取る）
    pub async fn validate_generation(output_path: &str) -> Result<ValidationResult> {
        let output_dir = PathBuf::from(output_path);

        if !output_dir.exists() {
            return Err(anyhow::anyhow!(
                "Generation output directory does not exist: {}",
                output_path
            ));
        }

        // terraform init
        TerraformCli::init(&output_dir)?;

        // terraform validate
        TerraformCli::validate(&output_dir)
    }

    /// フォーマットチェック（output_pathを直接受け取る）
    pub async fn check_format(output_path: &str) -> Result<FormatResult> {
        let output_dir = PathBuf::from(output_path);

        if !output_dir.exists() {
            return Err(anyhow::anyhow!(
                "Generation output directory does not exist: {}",
                output_path
            ));
        }

        TerraformCli::fmt_check(&output_dir)
    }

    /// 自動フォーマット（output_pathを直接受け取る）
    pub async fn format_code(output_path: &str) -> Result<Vec<String>> {
        let output_dir = PathBuf::from(output_path);

        if !output_dir.exists() {
            return Err(anyhow::anyhow!(
                "Generation output directory does not exist: {}",
                output_path
            ));
        }

        TerraformCli::fmt(&output_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::terraform::cli::terraform_cli_available;
    use std::fs;
    use std::io::Write;

    #[tokio::test]
    async fn test_check_terraform() {
        if !terraform_cli_available() {
            eprintln!("Terraform CLI not available, skipping test_check_terraform");
            return;
        }
        let result = ValidationService::check_terraform();
        assert!(result.available);
        assert!(!result.version.is_empty());
        println!("Terraform available: version {}", result.version);
    }

    #[tokio::test]
    async fn test_validate_generation_not_found() {
        let result = ValidationService::validate_generation("/non/existent/path").await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_check_format_not_found() {
        let result = ValidationService::check_format("/non/existent/path").await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_format_code_with_unformatted_file() {
        if !terraform_cli_available() {
            eprintln!(
                "Terraform CLI not available, skipping test_format_code_with_unformatted_file"
            );
            return;
        }
        // 一時的なテストディレクトリを作成
        let test_id = format!("test-format-{}", uuid::Uuid::new_v4());
        let test_dir = format!("./terraform-output/{}", test_id);
        fs::create_dir_all(&test_dir).unwrap();

        // フォーマットされていないTerraformファイルを作成
        let unformatted = r#"resource "null_resource" "test" {
triggers = {
value = "test"
}
}
"#;
        let mut file = fs::File::create(format!("{}/main.tf", test_dir)).unwrap();
        file.write_all(unformatted.as_bytes()).unwrap();

        let result = ValidationService::format_code(&test_dir).await;
        assert!(result.is_ok());
        let files_formatted = result.unwrap();
        assert!(!files_formatted.is_empty());
        println!("Formatted files: {:?}", files_formatted);

        let _ = fs::remove_dir_all(&test_dir);
    }
}
