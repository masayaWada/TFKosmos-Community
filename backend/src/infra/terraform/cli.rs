use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformVersion {
    pub version: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatResult {
    pub formatted: bool,
    pub diff: Option<String>,
    pub files_changed: Vec<String>,
}

pub struct TerraformCli;

impl TerraformCli {
    /// Terraformのバージョンを取得
    pub fn version() -> Result<TerraformVersion> {
        let output = Command::new("terraform")
            .arg("version")
            .arg("-json")
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // JSONパースしてバージョン抽出
                let version = serde_json::from_str::<serde_json::Value>(&stdout)
                    .ok()
                    .and_then(|v| v.get("terraform_version")?.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "unknown".to_string());

                Ok(TerraformVersion {
                    version,
                    available: true,
                })
            }
            _ => Ok(TerraformVersion {
                version: String::new(),
                available: false,
            }),
        }
    }

    /// terraform init を実行
    pub fn init(working_dir: &Path) -> Result<()> {
        let output = Command::new("terraform")
            .current_dir(working_dir)
            .arg("init")
            .arg("-backend=false")
            .arg("-input=false")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("terraform init failed: {}", stderr));
        }

        Ok(())
    }

    /// terraform validate を実行
    pub fn validate(working_dir: &Path) -> Result<ValidationResult> {
        let output = Command::new("terraform")
            .current_dir(working_dir)
            .arg("validate")
            .arg("-json")
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let result: serde_json::Value =
            serde_json::from_str(&stdout).unwrap_or_else(|_| serde_json::json!({"valid": false}));

        let valid = result
            .get("valid")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if let Some(diagnostics) = result.get("diagnostics").and_then(|d| d.as_array()) {
            for diag in diagnostics {
                let severity = diag.get("severity").and_then(|s| s.as_str()).unwrap_or("");
                let summary = diag.get("summary").and_then(|s| s.as_str()).unwrap_or("");
                let detail = diag.get("detail").and_then(|s| s.as_str()).unwrap_or("");

                let message = if detail.is_empty() {
                    summary.to_string()
                } else {
                    format!("{}: {}", summary, detail)
                };

                match severity {
                    "error" => errors.push(message),
                    "warning" => warnings.push(message),
                    _ => {}
                }
            }
        }

        Ok(ValidationResult {
            valid,
            errors,
            warnings,
        })
    }

    /// terraform fmt -check を実行
    pub fn fmt_check(working_dir: &Path) -> Result<FormatResult> {
        let output = Command::new("terraform")
            .current_dir(working_dir)
            .arg("fmt")
            .arg("-check")
            .arg("-diff")
            .arg("-recursive")
            .output()?;

        let formatted = output.status.success();
        let diff = if !formatted {
            Some(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            None
        };

        // 変更が必要なファイルをリストアップ
        let files_changed: Vec<String> = if !formatted {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.starts_with("---") || line.starts_with("+++"))
                .filter_map(|line| line.split_whitespace().nth(1).map(|s| s.to_string()))
                .collect()
        } else {
            vec![]
        };

        Ok(FormatResult {
            formatted,
            diff,
            files_changed,
        })
    }

    /// terraform fmt を実行（自動修正）
    pub fn fmt(working_dir: &Path) -> Result<Vec<String>> {
        let output = Command::new("terraform")
            .current_dir(working_dir)
            .arg("fmt")
            .arg("-recursive")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("terraform fmt failed: {}", stderr));
        }

        let files_formatted: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(files_formatted)
    }
}

/// テスト用: `terraform` が PATH にあり実行可能か（未導入環境では Terraform 連携テストをスキップする）
#[cfg(test)]
pub(crate) fn terraform_cli_available() -> bool {
    Command::new("terraform")
        .args(["version", "-json"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_terraform_version() {
        if !super::terraform_cli_available() {
            eprintln!("terraform CLI not available, skipping test_terraform_version");
            return;
        }
        let result = TerraformCli::version();
        assert!(result.is_ok());
        let version_info = result.unwrap();
        assert!(version_info.available);
        assert!(!version_info.version.is_empty());
        println!("Terraform version: {}", version_info.version);
    }

    #[test]
    fn test_terraform_init_with_valid_config() {
        if !super::terraform_cli_available() {
            eprintln!(
                "terraform CLI not available, skipping test_terraform_init_with_valid_config"
            );
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // 最小限の有効な Terraform 設定を作成
        let config = r#"
terraform {
  required_version = ">= 1.0"
}
"#;
        let mut file = fs::File::create(temp_path.join("main.tf")).unwrap();
        file.write_all(config.as_bytes()).unwrap();

        let result = TerraformCli::init(temp_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_terraform_validate_valid_config() {
        if !super::terraform_cli_available() {
            eprintln!("terraform CLI not available, skipping test_terraform_validate_valid_config");
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // 有効な Terraform 設定を作成
        let config = r#"
terraform {
  required_version = ">= 1.0"
}

resource "null_resource" "test" {
  triggers = {
    always_run = timestamp()
  }
}
"#;
        let mut file = fs::File::create(temp_path.join("main.tf")).unwrap();
        file.write_all(config.as_bytes()).unwrap();

        // まず init を実行
        TerraformCli::init(temp_path).unwrap();

        // validate を実行
        let result = TerraformCli::validate(temp_path);
        assert!(result.is_ok());
        let validation_result = result.unwrap();
        assert!(validation_result.valid);
        assert_eq!(validation_result.errors.len(), 0);
    }

    #[test]
    fn test_terraform_validate_invalid_config() {
        if !super::terraform_cli_available() {
            eprintln!(
                "terraform CLI not available, skipping test_terraform_validate_invalid_config"
            );
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // 無効な Terraform 設定を作成（構文エラー）
        let config = r#"
resource "null_resource" "test" {
  invalid_block {
    # This is intentionally invalid
  }
"#;
        let mut file = fs::File::create(temp_path.join("main.tf")).unwrap();
        file.write_all(config.as_bytes()).unwrap();

        // init を実行
        let _ = TerraformCli::init(temp_path);

        // validate を実行（エラーが期待される）
        let result = TerraformCli::validate(temp_path);
        assert!(result.is_ok());
        let validation_result = result.unwrap();
        assert!(!validation_result.valid);
        assert!(!validation_result.errors.is_empty());
    }

    #[test]
    fn test_terraform_fmt_check_formatted() {
        if !super::terraform_cli_available() {
            eprintln!("terraform CLI not available, skipping test_terraform_fmt_check_formatted");
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // フォーマット済みの設定を作成
        let config = r#"resource "null_resource" "test" {
  triggers = {
    always_run = timestamp()
  }
}
"#;
        let mut file = fs::File::create(temp_path.join("main.tf")).unwrap();
        file.write_all(config.as_bytes()).unwrap();

        let result = TerraformCli::fmt_check(temp_path);
        assert!(result.is_ok());
        let format_result = result.unwrap();
        assert!(format_result.formatted);
        assert!(format_result.diff.is_none());
        assert_eq!(format_result.files_changed.len(), 0);
    }

    #[test]
    fn test_terraform_fmt_check_unformatted() {
        if !super::terraform_cli_available() {
            eprintln!("terraform CLI not available, skipping test_terraform_fmt_check_unformatted");
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // フォーマットされていない設定を作成
        let config = r#"resource "null_resource" "test" {
triggers = {
always_run = timestamp()
}
}
"#;
        let mut file = fs::File::create(temp_path.join("main.tf")).unwrap();
        file.write_all(config.as_bytes()).unwrap();

        let result = TerraformCli::fmt_check(temp_path);
        assert!(result.is_ok());
        let format_result = result.unwrap();
        assert!(!format_result.formatted);
        assert!(format_result.diff.is_some());
    }

    #[test]
    fn test_terraform_fmt_auto_format() {
        if !super::terraform_cli_available() {
            eprintln!("terraform CLI not available, skipping test_terraform_fmt_auto_format");
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // フォーマットされていない設定を作成
        let config = r#"resource "null_resource" "test" {
triggers = {
always_run = timestamp()
}
}
"#;
        let mut file = fs::File::create(temp_path.join("main.tf")).unwrap();
        file.write_all(config.as_bytes()).unwrap();

        // 自動フォーマットを実行
        let result = TerraformCli::fmt(temp_path);
        assert!(result.is_ok());
        let files_formatted = result.unwrap();
        assert_eq!(files_formatted.len(), 1);
        assert!(files_formatted[0].contains("main.tf"));

        // 再度チェックするとフォーマット済みのはず
        let check_result = TerraformCli::fmt_check(temp_path);
        assert!(check_result.is_ok());
        let format_result = check_result.unwrap();
        assert!(format_result.formatted);
    }
}
