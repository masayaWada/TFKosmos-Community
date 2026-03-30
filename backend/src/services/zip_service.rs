use anyhow::Result;
use std::path::PathBuf;

/// ZIP圧縮を担当するサービス
pub struct ZipService;

impl ZipService {
    /// 指定ディレクトリをZIPファイルとして圧縮し、バイト列を返す
    pub async fn create_zip(output_path: &str, generation_id: &str) -> Result<Vec<u8>> {
        use zip::write::{FileOptions, ZipWriter};
        use zip::CompressionMethod;

        let path = PathBuf::from(output_path);

        if !path.exists() {
            return Err(anyhow::anyhow!(
                "Output directory does not exist: {}. Generation may have failed.",
                output_path
            ));
        }

        if !path.is_dir() {
            return Err(anyhow::anyhow!(
                "Output path is not a directory: {}",
                output_path
            ));
        }

        // ディレクトリにファイルが存在するか確認
        let mut has_files = false;
        for entry in std::fs::read_dir(&path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_file() {
                has_files = true;
                break;
            } else if entry_path.is_dir() {
                for sub_entry in std::fs::read_dir(&entry_path)? {
                    let sub_entry = sub_entry?;
                    if sub_entry.path().is_file() {
                        has_files = true;
                        break;
                    }
                }
                if has_files {
                    break;
                }
            }
        }

        if !has_files {
            return Err(anyhow::anyhow!(
                "No files were generated. The output directory is empty: {}. Please check if generation completed successfully.",
                output_path
            ));
        }

        let mut zip_data = Vec::new();
        {
            let mut zip = ZipWriter::new(std::io::Cursor::new(&mut zip_data));
            let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

            Self::add_directory_to_zip(&mut zip, &path, &path, options)?;

            zip.finish()?;
        }

        if zip_data.is_empty() {
            return Err(anyhow::anyhow!(
                "Failed to create ZIP file: no data was written. Generation ID: {}",
                generation_id
            ));
        }

        Ok(zip_data)
    }

    fn add_directory_to_zip(
        zip: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
        dir: &PathBuf,
        base: &PathBuf,
        options: zip::write::FileOptions,
    ) -> Result<()> {
        use std::io::{Read, Write};

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .strip_prefix(base)
                .map_err(|_| {
                    anyhow::anyhow!(
                        "Path {:?} is not a child of base path {:?}. This may occur due to symbolic links or filesystem issues.",
                        path, base
                    )
                })?
                .to_string_lossy()
                .replace('\\', "/");

            if path.is_file() {
                zip.start_file(&name, options)?;
                let mut file = std::fs::File::open(&path)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;
                zip.write_all(&buffer)?;
            } else if path.is_dir() {
                zip.add_directory(&name, options)?;
                Self::add_directory_to_zip(zip, &path, base, options)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_zip_success() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        std::fs::write(
            temp_dir.path().join("test.tf"),
            "resource \"aws_iam_user\" \"test\" {}",
        )
        .unwrap();

        let result = ZipService::create_zip(output_path, "test-id").await;
        assert!(result.is_ok());

        let zip_data = result.unwrap();
        assert!(!zip_data.is_empty());
    }

    #[tokio::test]
    async fn test_create_zip_directory_not_exists() {
        let result = ZipService::create_zip("/nonexistent/path", "test-id").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Output directory does not exist"));
    }

    #[tokio::test]
    async fn test_create_zip_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        let result = ZipService::create_zip(output_path, "test-id").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No files were generated"));
    }

    #[tokio::test]
    async fn test_create_zip_not_a_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "test").unwrap();

        let result = ZipService::create_zip(file_path.to_str().unwrap(), "test-id").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a directory"));
    }

    #[tokio::test]
    async fn test_create_zip_nested_directory() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        // サブディレクトリを作成してファイルを配置
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        std::fs::write(
            sub_dir.join("nested.tf"),
            "resource \"aws_iam_user\" \"nested\" {}",
        )
        .unwrap();

        let result = ZipService::create_zip(output_path, "nested-id").await;
        assert!(
            result.is_ok(),
            "Nested directory ZIP creation should succeed"
        );

        let zip_data = result.unwrap();
        assert!(!zip_data.is_empty(), "ZIP data should not be empty");

        // ZIPの内容を検証
        let cursor = std::io::Cursor::new(&zip_data);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut found_nested = false;
        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();
            if file.name().contains("nested.tf") {
                found_nested = true;
                break;
            }
        }
        assert!(found_nested, "nested.tf should be in the ZIP archive");
    }

    #[tokio::test]
    async fn test_create_zip_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_str().unwrap();

        // 複数ファイルを作成
        std::fs::write(
            temp_dir.path().join("main.tf"),
            "terraform { required_version = \">= 1.0\" }",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("variables.tf"),
            "variable \"region\" { default = \"ap-northeast-1\" }",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("outputs.tf"),
            "output \"region\" { value = var.region }",
        )
        .unwrap();

        let result = ZipService::create_zip(output_path, "multi-id").await;
        assert!(result.is_ok(), "Multiple files ZIP creation should succeed");

        let zip_data = result.unwrap();
        assert!(!zip_data.is_empty());

        // ZIPに3つのファイルが含まれることを確認
        let cursor = std::io::Cursor::new(&zip_data);
        let archive = zip::ZipArchive::new(cursor).unwrap();
        assert_eq!(archive.len(), 3, "ZIP should contain 3 files");
    }
}
