//! スキャン設定のインポート/エクスポートと管理
//!
//! JSON形式での設定の保存・読み込み・一覧を提供します。

use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::models::ScanConfig;

pub struct ConfigManagementService {
    config_dir: PathBuf,
}

impl ConfigManagementService {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// 設定ディレクトリを確保
    fn ensure_dir(&self) -> Result<()> {
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)
                .with_context(|| format!("Failed to create config dir: {:?}", self.config_dir))?;
        }
        Ok(())
    }

    /// 設定をJSONとしてエクスポート
    pub fn export_json(&self, config: &ScanConfig) -> Result<String> {
        serde_json::to_string_pretty(config).context("Failed to serialize config to JSON")
    }

    /// JSON文字列から設定をインポート
    pub fn import_json(&self, content: &str) -> Result<ScanConfig> {
        serde_json::from_str(content).context("Failed to parse JSON config")
    }

    /// 設定を名前付きで保存
    pub fn save_config(&self, name: &str, config: &ScanConfig) -> Result<()> {
        self.ensure_dir()?;
        let sanitized =
            Self::sanitize_filename(name).ok_or_else(|| anyhow::anyhow!("invalid config name"))?;
        let file_path = self.config_dir.join(format!("{}.json", sanitized));
        let json = serde_json::to_string_pretty(config)?;
        fs::write(&file_path, json)
            .with_context(|| format!("Failed to save config to {:?}", file_path))?;
        tracing::info!(name = %name, path = ?file_path, "Config saved");
        Ok(())
    }

    /// 名前付き設定を読み込み
    pub fn load_config(&self, name: &str) -> Result<ScanConfig> {
        let sanitized =
            Self::sanitize_filename(name).ok_or_else(|| anyhow::anyhow!("invalid config name"))?;
        let file_path = self.config_dir.join(format!("{}.json", sanitized));
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read config from {:?}", file_path))?;
        self.import_json(&content)
    }

    /// 保存済み設定の一覧を取得
    pub fn list_saved_configs(&self) -> Result<Vec<ConfigMetadata>> {
        if !self.config_dir.exists() {
            return Ok(Vec::new());
        }

        let mut configs = Vec::new();
        for entry in fs::read_dir(&self.config_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs())
                    });

                // プロバイダー情報を取得
                let provider = fs::read_to_string(&path)
                    .ok()
                    .and_then(|content| serde_json::from_str::<Value>(&content).ok())
                    .and_then(|v| v.get("provider").and_then(|p| p.as_str()).map(String::from));

                configs.push(ConfigMetadata {
                    name,
                    provider,
                    modified_at: modified,
                });
            }
        }

        configs.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(configs)
    }

    /// 保存済み設定を削除
    pub fn delete_config(&self, name: &str) -> Result<()> {
        let sanitized =
            Self::sanitize_filename(name).ok_or_else(|| anyhow::anyhow!("invalid config name"))?;
        let file_path = self.config_dir.join(format!("{}.json", sanitized));
        if file_path.exists() {
            fs::remove_file(&file_path)
                .with_context(|| format!("Failed to delete config {:?}", file_path))?;
        }
        Ok(())
    }

    /// ファイル名をサニタイズ（パストラバーサル防止）
    ///
    /// 許可される文字は英数字・ハイフン・アンダースコアのみとし、
    /// それ以外の文字を含む場合や、結果が空文字列となる場合は `None` を返します。
    fn sanitize_filename(name: &str) -> Option<String> {
        let sanitized: String = name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if sanitized.is_empty() {
            None
        } else {
            Some(sanitized)
        }
    }
}

/// 保存済み設定のメタデータ
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigMetadata {
    pub name: String,
    pub provider: Option<String>,
    pub modified_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_test_config() -> ScanConfig {
        ScanConfig {
            provider: "aws".to_string(),
            account_id: Some("123456789012".to_string()),
            profile: Some("default".to_string()),
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets: HashMap::from([("users".to_string(), true), ("roles".to_string(), true)]),
            filters: HashMap::new(),
            include_tags: true,
        }
    }

    #[test]
    fn test_export_import_json_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let service = ConfigManagementService::new(tmp.path().to_path_buf());
        let config = make_test_config();

        let json = service.export_json(&config).unwrap();
        let imported = service.import_json(&json).unwrap();

        assert_eq!(imported.provider, "aws");
        assert_eq!(imported.account_id, Some("123456789012".to_string()));
        assert_eq!(imported.scan_targets.get("users"), Some(&true));
    }

    #[test]
    fn test_save_and_load_config() {
        let tmp = TempDir::new().unwrap();
        let service = ConfigManagementService::new(tmp.path().to_path_buf());
        let config = make_test_config();

        service.save_config("my-config", &config).unwrap();
        let loaded = service.load_config("my-config").unwrap();

        assert_eq!(loaded.provider, "aws");
        assert_eq!(loaded.profile, Some("default".to_string()));
    }

    #[test]
    fn test_list_saved_configs() {
        let tmp = TempDir::new().unwrap();
        let service = ConfigManagementService::new(tmp.path().to_path_buf());

        service
            .save_config("config-a", &make_test_config())
            .unwrap();
        service
            .save_config("config-b", &make_test_config())
            .unwrap();

        let configs = service.list_saved_configs().unwrap();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_delete_config() {
        let tmp = TempDir::new().unwrap();
        let service = ConfigManagementService::new(tmp.path().to_path_buf());

        service
            .save_config("to-delete", &make_test_config())
            .unwrap();
        assert!(service.load_config("to-delete").is_ok());

        service.delete_config("to-delete").unwrap();
        assert!(service.load_config("to-delete").is_err());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            ConfigManagementService::sanitize_filename("my-config"),
            Some("my-config".to_string())
        );
        assert_eq!(
            ConfigManagementService::sanitize_filename("../evil"),
            Some("evil".to_string())
        );
        assert_eq!(
            ConfigManagementService::sanitize_filename("path/traversal"),
            Some("pathtraversal".to_string())
        );
        assert_eq!(ConfigManagementService::sanitize_filename("../.."), None);
    }

    #[test]
    fn test_list_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let service = ConfigManagementService::new(tmp.path().join("nonexistent"));
        let configs = service.list_saved_configs().unwrap();
        assert!(configs.is_empty());
    }
}
