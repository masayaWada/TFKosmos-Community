//! Terraform コード生成サービス
//!
//! スキャン結果から Terraform の `.tf` ファイルとインポートスクリプトを
//! 生成し、生成結果を TTL付きのインメモリキャッシュに保持する。
//! 生成処理には [`TerraformGenerator`]、ファイルI/Oには [`FileService`] を使用する。

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::infra::generators::terraform::TerraformGenerator;
use crate::infra::terraform::cli::TerraformCli;
use crate::models::{GenerationConfig, GenerationResponse};
use crate::services::file_service::FileService;
use crate::services::preview_service::PreviewService;
use crate::services::scan_service::ScanService;

/// 生成キャッシュのTTL（1時間）
const GENERATION_CACHE_TTL: Duration = Duration::from_secs(3600);

// In-memory cache for generation results (in production, use Redis or database)
type GenerationCache = Arc<RwLock<HashMap<String, GenerationCacheEntry>>>;

#[derive(Clone)]
pub struct GenerationCacheEntry {
    pub output_path: String,
    #[allow(dead_code)]
    pub files: Vec<String>,
    /// エントリ作成時刻（TTL管理用）
    pub created_at: Instant,
}

/// Terraform コード生成サービス
///
/// スキャン済みリソースデータを入力として、Terraform の `.tf` ファイルと
/// インポートスクリプト（`.sh` / `.ps1`）を生成する。生成結果は
/// [`GENERATION_CACHE_TTL`] に従ってインメモリキャッシュに保持される。
pub struct GenerationService {
    scan_service: Arc<ScanService>,
    cache: GenerationCache,
}

impl GenerationService {
    /// 新しい `GenerationService` を生成する
    ///
    /// # Arguments
    /// * `scan_service` - スキャンデータ取得に使用するサービス
    pub fn new(scan_service: Arc<ScanService>) -> Self {
        Self {
            scan_service,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Terraform ファイルを生成する
    ///
    /// 指定されたスキャン結果から Terraform の `.tf` ファイルとインポートスクリプトを
    /// 生成し、出力ディレクトリに書き込む。生成後に `terraform fmt` を実行してフォーマットする。
    ///
    /// # Arguments
    /// * `scan_id` - 生成元スキャンのID
    /// * `config` - 出力パス・ファイル分割ルール・命名規則などの生成設定
    /// * `selected_resources` - 生成対象リソースのマップ（リソースタイプ → リソース配列）
    ///
    /// # Errors
    /// - スキャンデータが見つからない場合
    /// - 出力ディレクトリの作成に失敗した場合
    /// - テンプレートレンダリングに失敗した場合
    pub async fn generate_terraform(
        &self,
        scan_id: &str,
        config: GenerationConfig,
        selected_resources: HashMap<String, Vec<Value>>,
    ) -> Result<GenerationResponse> {
        tracing::info!(scan_id = %scan_id, "Starting Terraform generation");
        tracing::debug!(
            scan_id = %scan_id,
            output_path = %config.output_path,
            file_split_rule = %config.file_split_rule,
            naming_convention = %config.naming_convention,
            "Generation config"
        );
        tracing::debug!(scan_id = %scan_id, ?selected_resources, "Selected resources");

        // Get scan data
        let scan_data = self
            .scan_service
            .get_scan_data(scan_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Scan not found: {}", scan_id))?;

        tracing::debug!(scan_id = %scan_id, provider = ?scan_data.get("provider"), "Scan data retrieved");

        // Generate Terraform code
        let generation_id = Uuid::new_v4().to_string();

        // 出力パスを解決してディレクトリを作成
        let output_path = FileService::resolve_output_path(&config.output_path, &generation_id);
        tracing::debug!(scan_id = %scan_id, output_path = ?output_path, "Creating output directory");
        FileService::ensure_directory(&output_path)
            .with_context(|| format!("Failed to create output directory: {:?}", output_path))?;
        tracing::debug!(scan_id = %scan_id, output_path = ?output_path, "Output directory created successfully");

        tracing::debug!(scan_id = %scan_id, "Calling TerraformGenerator::generate");
        let files =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await
                .context("Failed to generate Terraform files")?;

        tracing::info!(scan_id = %scan_id, file_count = files.len(), "Terraform files generated");

        // Generate import script
        tracing::debug!(scan_id = %scan_id, "Generating import script");
        let import_script_path = TerraformGenerator::generate_import_script(
            &scan_data,
            &config,
            &selected_resources,
            &output_path,
        )
        .await
        .context("Failed to generate import script")?;

        if let Some(ref script_path) = import_script_path {
            tracing::debug!(scan_id = %scan_id, script_path = %script_path, "Import script generated");
        } else {
            tracing::debug!(scan_id = %scan_id, "No import script generated (no resources to import)");
        }

        // Format generated Terraform files
        tracing::debug!(scan_id = %scan_id, "Formatting Terraform files with terraform fmt");
        match TerraformCli::fmt(&output_path) {
            Ok(formatted_files) => {
                if !formatted_files.is_empty() {
                    tracing::debug!(scan_id = %scan_id, file_count = formatted_files.len(), "Formatted Terraform files");
                } else {
                    tracing::debug!(scan_id = %scan_id, "All files were already formatted");
                }
            }
            Err(e) => {
                tracing::warn!(scan_id = %scan_id, error = %e, "Failed to format Terraform files; run 'terraform fmt' manually if needed");
            }
        }

        // プレビュー生成
        tracing::debug!(scan_id = %scan_id, file_count = files.len(), "Generating preview");
        let preview =
            PreviewService::generate_preview(&output_path, &files, import_script_path.as_deref());
        tracing::debug!(scan_id = %scan_id, preview_file_count = preview.len(), "Preview generation complete");

        let response = GenerationResponse {
            generation_id: generation_id.clone(),
            output_path: output_path.to_string_lossy().to_string(),
            files: files.clone(),
            import_script_path,
            preview: Some(preview),
        };

        // キャッシュに結果を保存
        self.cache.write().await.insert(
            generation_id,
            GenerationCacheEntry {
                output_path: response.output_path.clone(),
                files,
                created_at: Instant::now(),
            },
        );

        Ok(response)
    }

    /// キャッシュエントリを取得する（TTL切れの場合はNoneを返す）
    pub async fn get_cache_entry(&self, generation_id: &str) -> Option<GenerationCacheEntry> {
        let cache = self.cache.read().await;
        cache.get(generation_id).and_then(|entry| {
            if entry.created_at.elapsed() > GENERATION_CACHE_TTL {
                None
            } else {
                Some(entry.clone())
            }
        })
    }

    /// テスト用: 全生成キャッシュをクリアする
    #[cfg(test)]
    pub async fn clear_all(&self) {
        self.cache.write().await.clear();
    }

    /// 期限切れの生成キャッシュエントリを削除する
    pub async fn cleanup_expired_generations(&self) {
        let mut cache = self.cache.write().await;
        let before = cache.len();
        cache.retain(|_, v| v.created_at.elapsed() <= GENERATION_CACHE_TTL);
        let removed = before - cache.len();
        if removed > 0 {
            tracing::info!(
                removed_count = removed,
                "Cleaned up expired generation cache entries"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::terraform::cli::terraform_cli_available;
    use crate::models::ScanConfig;
    use crate::services::scan_service::{RealScannerFactory, ScanService};
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_scan_service() -> Arc<ScanService> {
        Arc::new(ScanService::new(Arc::new(RealScannerFactory::new())))
    }

    fn create_test_generation_service(scan_service: Arc<ScanService>) -> GenerationService {
        GenerationService::new(scan_service)
    }

    // テストデータの作成ヘルパー
    fn create_test_scan_data() -> Value {
        json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "test-user",
                    "arn": "arn:aws:iam::123456789012:user/test-user",
                    "user_id": "AIDAEXAMPLE",
                    "create_date": "2023-01-01T00:00:00Z"
                }
            ],
            "groups": [
                {
                    "group_name": "test-group",
                    "arn": "arn:aws:iam::123456789012:group/test-group",
                    "group_id": "AGPAEXAMPLE",
                    "create_date": "2023-01-01T00:00:00Z"
                }
            ],
            "roles": [
                {
                    "role_name": "test-role",
                    "arn": "arn:aws:iam::123456789012:role/test-role",
                    "role_id": "AROAEXAMPLE",
                    "create_date": "2023-01-01T00:00:00Z",
                    "assume_role_policy_document": "{\"Version\":\"2012-10-17\",\"Statement\":[{\"Effect\":\"Allow\",\"Principal\":{\"Service\":\"lambda.amazonaws.com\"},\"Action\":\"sts:AssumeRole\"}]}"
                }
            ],
            "policies": [
                {
                    "policy_name": "test-policy",
                    "arn": "arn:aws:iam::123456789012:policy/test-policy",
                    "policy_id": "ANPAEXAMPLE",
                    "description": "Test policy",
                    "create_date": "2023-01-01T00:00:00Z",
                    "policy_document": {
                        "Version": "2012-10-17",
                        "Statement": [
                            {
                                "Sid": "TestStatement",
                                "Effect": "Allow",
                                "Action": ["s3:GetObject"],
                                "Resource": ["arn:aws:s3:::test-bucket/*"]
                            }
                        ]
                    }
                }
            ]
        })
    }

    fn create_test_config(output_path: &str) -> GenerationConfig {
        GenerationConfig {
            output_path: output_path.to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            generate_readme: true,
            import_script_format: "sh".to_string(),
            selected_resources: HashMap::new(),
        }
    }

    // ========================================
    // generate_terraform のテスト
    // ========================================

    #[tokio::test]
    async fn test_generate_terraform_success() {
        if !terraform_cli_available() {
            eprintln!("Terraform CLI not available, skipping test_generate_terraform_success");
            return;
        }
        let scan_id = "test-scan-id";
        let scan_data = create_test_scan_data();

        let config = ScanConfig {
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

        let scan_service = create_test_scan_service();
        scan_service
            .insert_test_scan_data(scan_id.to_string(), config, scan_data)
            .await;
        let gen_service = create_test_generation_service(scan_service);

        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        let gen_config = create_test_config(output_path);
        let selected_resources = HashMap::new();

        let result = gen_service
            .generate_terraform(scan_id, gen_config, selected_resources)
            .await;

        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response.generation_id.is_empty());
        assert!(!response.files.is_empty());
        assert!(response.preview.is_some());

        for file in &response.files {
            let file_path = PathBuf::from(&response.output_path).join(file);
            assert!(
                file_path.exists(),
                "Generated file should exist: {:?}",
                file_path
            );
        }
    }

    #[tokio::test]
    async fn test_generate_terraform_scan_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        let gen_config = create_test_config(output_path);
        let selected_resources = HashMap::new();

        let scan_service = create_test_scan_service();
        let gen_service = create_test_generation_service(scan_service);

        let result = gen_service
            .generate_terraform("nonexistent-scan-id", gen_config, selected_resources)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Scan not found"));
    }

    #[tokio::test]
    async fn test_generate_terraform_with_selected_resources() {
        if !terraform_cli_available() {
            eprintln!(
                "Terraform CLI not available, skipping test_generate_terraform_with_selected_resources"
            );
            return;
        }
        let scan_id = "test-scan-selected";
        let scan_data = create_test_scan_data();

        let config = ScanConfig {
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

        let scan_service = create_test_scan_service();
        scan_service
            .insert_test_scan_data(scan_id.to_string(), config, scan_data)
            .await;
        let gen_service = create_test_generation_service(scan_service);

        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        let gen_config = create_test_config(output_path);

        let mut selected_resources = HashMap::new();
        selected_resources.insert("users".to_string(), vec![json!("test-user")]);

        let result = gen_service
            .generate_terraform(scan_id, gen_config, selected_resources)
            .await;

        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response.files.is_empty());

        let users_file_exists = response.files.iter().any(|f| f.contains("users"));
        assert!(users_file_exists, "Users file should be generated");
    }

    #[tokio::test]
    async fn test_generate_terraform_preview_generation() {
        if !terraform_cli_available() {
            eprintln!(
                "Terraform CLI not available, skipping test_generate_terraform_preview_generation"
            );
            return;
        }
        let scan_id = "test-scan-preview";
        let scan_data = create_test_scan_data();

        let config = ScanConfig {
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

        let scan_service = create_test_scan_service();
        scan_service
            .insert_test_scan_data(scan_id.to_string(), config, scan_data)
            .await;
        let gen_service = create_test_generation_service(scan_service);

        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        let gen_config = create_test_config(output_path);
        let selected_resources = HashMap::new();

        let result = gen_service
            .generate_terraform(scan_id, gen_config, selected_resources)
            .await;

        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.preview.is_some());

        let preview = response.preview.unwrap();
        assert!(!preview.is_empty());

        for (file_name, content) in preview {
            assert!(
                content.len() <= 1003,
                "Preview for {} should be <= 1000 chars + '...'",
                file_name
            );
        }
    }

    #[tokio::test]
    async fn test_generate_terraform_with_readme() {
        if !terraform_cli_available() {
            eprintln!("Terraform CLI not available, skipping test_generate_terraform_with_readme");
            return;
        }
        let scan_id = "test-scan-readme";
        let scan_data = create_test_scan_data();

        let config = ScanConfig {
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

        let scan_service = create_test_scan_service();
        scan_service
            .insert_test_scan_data(scan_id.to_string(), config, scan_data)
            .await;
        let gen_service = create_test_generation_service(scan_service);

        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        let mut gen_config = create_test_config(output_path);
        gen_config.generate_readme = true;

        let selected_resources = HashMap::new();

        let result = gen_service
            .generate_terraform(scan_id, gen_config, selected_resources)
            .await;

        assert!(result.is_ok());

        let response = result.unwrap();

        let readme_exists = response.files.iter().any(|f| f == "README.md");
        assert!(readme_exists, "README.md should be generated");

        let readme_path = PathBuf::from(&response.output_path).join("README.md");
        assert!(readme_path.exists(), "README.md file should exist");
    }

    #[tokio::test]
    async fn test_generate_terraform_import_script_generation() {
        if !terraform_cli_available() {
            eprintln!(
                "Terraform CLI not available, skipping test_generate_terraform_import_script_generation"
            );
            return;
        }
        let scan_id = "test-scan-import";
        let scan_data = create_test_scan_data();

        let config = ScanConfig {
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

        let scan_service = create_test_scan_service();
        scan_service
            .insert_test_scan_data(scan_id.to_string(), config, scan_data)
            .await;
        let gen_service = create_test_generation_service(scan_service);

        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        let gen_config = create_test_config(output_path);
        let selected_resources = HashMap::new();

        let result = gen_service
            .generate_terraform(scan_id, gen_config, selected_resources)
            .await;

        assert!(result.is_ok());

        let response = result.unwrap();

        assert!(response.import_script_path.is_some());

        let import_script_path = response.import_script_path.unwrap();
        assert_eq!(import_script_path, "import.sh");

        let script_path = PathBuf::from(&response.output_path).join(&import_script_path);
        assert!(script_path.exists(), "Import script should exist");

        let script_content = std::fs::read_to_string(script_path).unwrap();
        assert!(script_content.contains("#!/bin/bash"));
        assert!(script_content.contains("terraform import"));
    }

    #[tokio::test]
    async fn test_clear_all_removes_all_cache_entries() {
        let scan_service = create_test_scan_service();
        let gen_service = create_test_generation_service(scan_service);

        // 手動でキャッシュエントリを挿入
        gen_service.cache.write().await.insert(
            "gen-1".to_string(),
            GenerationCacheEntry {
                output_path: "/tmp/test".to_string(),
                files: vec!["main.tf".to_string()],
                created_at: Instant::now(),
            },
        );

        assert!(gen_service.get_cache_entry("gen-1").await.is_some());

        gen_service.clear_all().await;

        assert!(gen_service.get_cache_entry("gen-1").await.is_none());
    }
}
