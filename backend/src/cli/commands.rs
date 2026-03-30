use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;

use crate::cli::OutputFormat;
use crate::infra::terraform::state_parser::TfState;
use crate::models::ScanConfig;
use crate::services::drift_service::DriftService;
use crate::services::scan_service::{RealScannerFactory, ScanService};

/// Load ScanConfig from a TOML or JSON file
pub fn load_scan_config(path: &Path) -> Result<ScanConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("設定ファイルの読み込みに失敗: {}", path.display()))?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "toml" => toml::from_str(&content).with_context(|| "TOML設定ファイルのパースに失敗"),
        "json" => serde_json::from_str(&content).with_context(|| "JSON設定ファイルのパースに失敗"),
        _ => anyhow::bail!(
            "サポートされていない設定ファイル形式: {} (.toml or .json のみ)",
            ext
        ),
    }
}

/// Execute scan command
pub async fn run_scan(config_path: &Path, output: &OutputFormat) -> Result<()> {
    let config = load_scan_config(config_path)?;

    let scan_service = Arc::new(ScanService::new(Arc::new(RealScannerFactory::new())));

    eprintln!("スキャンを開始... (プロバイダー: {})", config.provider);

    let scan_id = scan_service.start_scan(config).await?;

    // Wait for scan to complete
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        if let Some(result) = scan_service.get_scan_result(&scan_id).await {
            // Check if scan is still in progress
            if result.status == "scanning" {
                if let Some(progress) = result.progress {
                    eprintln!("進捗: {}%", progress);
                }
                continue;
            }

            match output {
                OutputFormat::Json => {
                    // Get the raw scan data for JSON output
                    if let Some(data) = scan_service.get_scan_data(&scan_id).await {
                        println!("{}", serde_json::to_string_pretty(&data)?);
                    }
                }
                OutputFormat::Table => {
                    eprintln!("スキャンID: {}", scan_id);
                    eprintln!("ステータス: {}", result.status);
                    if let Some(summary) = &result.summary {
                        for (key, value) in summary {
                            eprintln!("  {}: {}", key, value);
                        }
                    }
                }
                OutputFormat::Quiet => {
                    // Only print scan_id for scripting
                    println!("{}", scan_id);
                }
            }
            break;
        }
    }

    Ok(())
}

/// Execute drift command
pub async fn run_drift(scan_id: &str, state_file: &Path, output: &OutputFormat) -> Result<()> {
    let state_content = std::fs::read_to_string(state_file)
        .with_context(|| format!("stateファイルの読み込みに失敗: {}", state_file.display()))?;

    // Validate state file is parseable
    TfState::parse(&state_content)?;

    let scan_service = Arc::new(ScanService::new(Arc::new(RealScannerFactory::new())));
    let drift_service = Arc::new(DriftService::new(scan_service));

    eprintln!("ドリフト検出を開始...");

    let report = drift_service.detect_drift(scan_id, &state_content).await?;

    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        OutputFormat::Table => {
            eprintln!("ドリフトID: {}", report.drift_id);
            eprintln!("サマリー:");
            eprintln!("  State内リソース: {}", report.summary.total_in_state);
            eprintln!("  クラウド内リソース: {}", report.summary.total_in_cloud);
            eprintln!("  追加: {}", report.summary.added);
            eprintln!("  削除: {}", report.summary.removed);
            eprintln!("  変更: {}", report.summary.changed);
            eprintln!("  変更なし: {}", report.summary.unchanged);
            if !report.drifts.is_empty() {
                eprintln!("\nドリフト一覧:");
                for item in &report.drifts {
                    eprintln!(
                        "  [{:?}] {} - {}",
                        item.drift_type, item.resource_type, item.resource_id
                    );
                }
            }
        }
        OutputFormat::Quiet => {
            // Exit code indicates drift: 0 = no drift, 1 = drift detected
            if !report.drifts.is_empty() {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

/// Execute generate command
pub async fn run_generate(scan_id: &str, output_dir: &Path) -> Result<()> {
    eprintln!("Terraform生成を開始... (scan_id: {})", scan_id);
    eprintln!("出力先: {}", output_dir.display());

    // For now, print a message. Full implementation requires GenerationService integration
    // which needs the scan data to already be in memory.
    eprintln!("注意: CLIからの生成は、先にscanコマンドで取得したデータに対して実行してください");
    eprintln!("TODO: スキャンデータの永続化が必要です");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn loads_toml_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(
            file,
            r#"
provider = "aws"
account_id = "123456789012"
profile = "default"
include_tags = true
"#
        )
        .unwrap();

        let config = load_scan_config(&path).unwrap();
        assert_eq!(config.provider, "aws");
        assert_eq!(config.account_id, Some("123456789012".to_string()));
        assert_eq!(config.profile, Some("default".to_string()));
    }

    #[test]
    fn loads_json_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.json");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(
            file,
            r#"{{
                "provider": "azure",
                "subscription_id": "sub-123",
                "tenant_id": "tenant-456",
                "auth_method": "az_login"
            }}"#
        )
        .unwrap();

        let config = load_scan_config(&path).unwrap();
        assert_eq!(config.provider, "azure");
        assert_eq!(config.subscription_id, Some("sub-123".to_string()));
    }

    #[test]
    fn rejects_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.yaml");
        std::fs::write(&path, "provider: aws").unwrap();

        let result = load_scan_config(&path);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("サポートされていない設定ファイル形式"));
    }

    #[test]
    fn returns_error_for_missing_file() {
        let path = Path::new("/tmp/nonexistent_tfkosmos_test.toml");
        let result = load_scan_config(path);
        assert!(result.is_err());
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(err_msg.contains("設定ファイルの読み込みに失敗"));
    }
}
