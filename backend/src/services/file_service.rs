use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// ファイルI/O操作を担当するサービス
pub struct FileService;

impl FileService {
    /// 出力パスを解決する
    ///
    /// 相対パスの場合はbackendディレクトリを基準に解決し、
    /// generation_idをサブディレクトリとして付加する。
    pub fn resolve_output_path(config_output_path: &str, generation_id: &str) -> PathBuf {
        if config_output_path.starts_with('/')
            || (cfg!(windows) && config_output_path.contains(':'))
        {
            // 絶対パス
            PathBuf::from(config_output_path).join(generation_id)
        } else {
            // 相対パス - カレントディレクトリまたはbackendディレクトリを基準に解決
            let base_path = if let Ok(current_dir) = std::env::current_dir() {
                if current_dir.ends_with("backend") {
                    current_dir
                } else if current_dir.join("backend").exists() {
                    current_dir.join("backend")
                } else {
                    current_dir
                }
            } else {
                PathBuf::from(".")
            };
            base_path.join(config_output_path).join(generation_id)
        }
    }

    /// ディレクトリが存在しない場合は作成する
    pub fn ensure_directory(path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create output directory: {:?}", path))?;

        if !path.exists() {
            return Err(anyhow::anyhow!(
                "Output directory was not created: {:?}",
                path
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_output_path_absolute() {
        let result = FileService::resolve_output_path("/tmp/output", "gen-123");
        assert_eq!(result, PathBuf::from("/tmp/output/gen-123"));
    }

    #[test]
    fn test_resolve_output_path_relative() {
        let result = FileService::resolve_output_path("terraform-output", "gen-456");
        // 相対パスの場合、カレントディレクトリまたはbackendを基準に解決される
        assert!(result.ends_with("terraform-output/gen-456"));
    }

    #[test]
    fn test_ensure_directory_creates_new_directory() {
        let temp_dir = TempDir::new().unwrap();
        let new_dir = temp_dir.path().join("new_subdir").join("nested");

        let result = FileService::ensure_directory(&new_dir);
        assert!(result.is_ok());
        assert!(new_dir.exists());
    }

    #[test]
    fn test_ensure_directory_existing_directory() {
        let temp_dir = TempDir::new().unwrap();

        // 既存ディレクトリへの呼び出しは成功する
        let result = FileService::ensure_directory(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_output_path_absolute_with_trailing_component() {
        let result = FileService::resolve_output_path("/var/data/terraform", "gen-abc");
        assert_eq!(
            result,
            PathBuf::from("/var/data/terraform/gen-abc"),
            "絶対パスにgeneration_idがサブディレクトリとして付加されるべき"
        );
    }

    #[test]
    fn test_resolve_output_path_different_generation_ids() {
        let r1 = FileService::resolve_output_path("/tmp/out", "gen-001");
        let r2 = FileService::resolve_output_path("/tmp/out", "gen-002");

        assert_ne!(r1, r2, "異なるgeneration_idで異なるパスが返されるべき");
        assert!(r1.ends_with("gen-001"));
        assert!(r2.ends_with("gen-002"));
    }

    #[test]
    fn test_ensure_directory_creates_deeply_nested() {
        let temp_dir = TempDir::new().unwrap();
        let deep_path = temp_dir.path().join("a").join("b").join("c").join("d");

        let result = FileService::ensure_directory(&deep_path);
        assert!(result.is_ok(), "深いネストのディレクトリ作成に成功するべき");
        assert!(deep_path.exists(), "作成されたディレクトリが存在するべき");
    }

    #[test]
    fn test_ensure_directory_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let new_dir = temp_dir.path().join("idempotent_test");

        // 2回呼び出しても成功する
        let result1 = FileService::ensure_directory(&new_dir);
        let result2 = FileService::ensure_directory(&new_dir);
        assert!(result1.is_ok());
        assert!(result2.is_ok(), "ディレクトリが既に存在しても成功するべき");
    }

    #[test]
    fn test_resolve_output_path_empty_generation_id() {
        let result = FileService::resolve_output_path("/tmp/out", "");
        assert_eq!(
            result,
            PathBuf::from("/tmp/out/"),
            "空のgeneration_idでもパスが正しく構築されるべき"
        );
    }
}
