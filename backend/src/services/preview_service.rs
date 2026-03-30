use std::collections::HashMap;
use std::path::Path;

/// プレビュー生成を担当するサービス
pub struct PreviewService;

impl PreviewService {
    /// 生成されたファイルのプレビューを生成する
    ///
    /// 各ファイルの先頭1000文字を読み取り、ファイル名をキーとするHashMapを返す。
    pub fn generate_preview(
        output_path: &Path,
        files: &[String],
        import_script_path: Option<&str>,
    ) -> HashMap<String, String> {
        let mut preview = HashMap::new();

        for file_path in files {
            let full_path = output_path.join(file_path);
            if full_path.exists() && full_path.is_file() {
                match std::fs::read_to_string(&full_path) {
                    Ok(content) => {
                        let preview_content = Self::truncate_to_chars(&content, 1000);
                        preview.insert(file_path.clone(), preview_content);
                    }
                    Err(e) => {
                        tracing::warn!(file = ?full_path, error = %e, "Failed to read file for preview");
                    }
                }
            } else {
                tracing::warn!(file = ?full_path, "File does not exist for preview");
            }
        }

        // インポートスクリプトもプレビューに含める
        if let Some(script_path) = import_script_path {
            let full_path = output_path.join(script_path);
            if full_path.exists() && full_path.is_file() {
                match std::fs::read_to_string(&full_path) {
                    Ok(content) => {
                        let preview_content = Self::truncate_to_chars(&content, 1000);
                        preview.insert(script_path.to_string(), preview_content);
                    }
                    Err(e) => {
                        tracing::warn!(file = ?full_path, error = %e, "Failed to read import script for preview");
                    }
                }
            } else {
                tracing::warn!(file = ?full_path, "Import script does not exist for preview");
            }
        }

        preview
    }

    /// 文字列を指定文字数で切り詰める
    ///
    /// UTF-8マルチバイト文字の途中でスライスしないように文字境界で切り詰める。
    fn truncate_to_chars(s: &str, max_chars: usize) -> String {
        let char_count = s.chars().count();
        if char_count <= max_chars {
            return s.to_string();
        }

        let mut byte_index = 0;
        for (count, (idx, _)) in s.char_indices().enumerate() {
            if count >= max_chars {
                byte_index = idx;
                break;
            }
        }

        if byte_index == 0 {
            s.to_string()
        } else {
            format!("{}...", &s[..byte_index])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_truncate_to_chars_short_string() {
        let input = "Hello World";
        let result = PreviewService::truncate_to_chars(input, 100);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_truncate_to_chars_exact_length() {
        let input = "Hello";
        let result = PreviewService::truncate_to_chars(input, 5);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_truncate_to_chars_long_string() {
        let input = "Hello World, this is a test string";
        let result = PreviewService::truncate_to_chars(input, 10);
        assert_eq!(result, "Hello Worl...");
    }

    #[test]
    fn test_truncate_to_chars_multibyte_characters() {
        let input = "こんにちは世界";
        let result = PreviewService::truncate_to_chars(input, 5);
        assert_eq!(result, "こんにちは...");
    }

    #[test]
    fn test_truncate_to_chars_mixed_characters() {
        let input = "Hello世界";
        let result = PreviewService::truncate_to_chars(input, 6);
        assert_eq!(result, "Hello世...");
    }

    #[test]
    fn test_generate_preview_normal() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("main.tf"),
            "resource \"aws_iam_user\" \"test\" {}",
        )
        .unwrap();

        let files = vec!["main.tf".to_string()];
        let preview = PreviewService::generate_preview(temp_dir.path(), &files, None);

        assert!(preview.contains_key("main.tf"));
        assert!(preview["main.tf"].contains("aws_iam_user"));
    }

    #[test]
    fn test_generate_preview_with_import_script() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("main.tf"),
            "resource \"aws_iam_user\" \"test\" {}",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("import.sh"),
            "#!/bin/bash\nterraform import",
        )
        .unwrap();

        let files = vec!["main.tf".to_string()];
        let preview = PreviewService::generate_preview(temp_dir.path(), &files, Some("import.sh"));

        assert!(preview.contains_key("main.tf"));
        assert!(preview.contains_key("import.sh"));
    }

    #[test]
    fn test_generate_preview_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let files = vec!["nonexistent.tf".to_string()];

        let preview = PreviewService::generate_preview(temp_dir.path(), &files, None);
        assert!(preview.is_empty());
    }

    #[test]
    fn test_generate_preview_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("empty.tf"), "").unwrap();

        let files = vec!["empty.tf".to_string()];
        let preview = PreviewService::generate_preview(temp_dir.path(), &files, None);

        assert!(
            preview.contains_key("empty.tf"),
            "空ファイルもプレビューに含まれるべき"
        );
        assert_eq!(
            preview["empty.tf"], "",
            "空ファイルの内容は空文字列であるべき"
        );
    }

    #[test]
    fn test_generate_preview_truncates_long_content() {
        let temp_dir = TempDir::new().unwrap();
        // 1000文字以上のコンテンツを作成
        let long_content = "x".repeat(2000);
        fs::write(temp_dir.path().join("long.tf"), &long_content).unwrap();

        let files = vec!["long.tf".to_string()];
        let preview = PreviewService::generate_preview(temp_dir.path(), &files, None);

        assert!(preview.contains_key("long.tf"));
        let content = &preview["long.tf"];
        assert!(
            content.len() < long_content.len(),
            "プレビューは元のコンテンツより短くなるべき"
        );
        assert!(
            content.ends_with("..."),
            "切り詰められたプレビューは...で終わるべき"
        );
    }

    #[test]
    fn test_generate_preview_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("main.tf"), "resource \"a\" {}").unwrap();
        fs::write(temp_dir.path().join("variables.tf"), "variable \"x\" {}").unwrap();
        fs::write(temp_dir.path().join("outputs.tf"), "output \"y\" {}").unwrap();

        let files = vec![
            "main.tf".to_string(),
            "variables.tf".to_string(),
            "outputs.tf".to_string(),
        ];
        let preview = PreviewService::generate_preview(temp_dir.path(), &files, None);

        assert_eq!(preview.len(), 3, "3ファイル全てがプレビューに含まれるべき");
    }

    #[test]
    fn test_generate_preview_missing_import_script() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("main.tf"), "resource {}").unwrap();

        let files = vec!["main.tf".to_string()];
        let preview =
            PreviewService::generate_preview(temp_dir.path(), &files, Some("missing_import.sh"));

        // main.tfはあるが、import scriptはない
        assert_eq!(
            preview.len(),
            1,
            "存在するファイルのみプレビューに含まれるべき"
        );
        assert!(preview.contains_key("main.tf"));
        assert!(!preview.contains_key("missing_import.sh"));
    }

    #[test]
    fn test_generate_preview_empty_file_list() {
        let temp_dir = TempDir::new().unwrap();

        let files: Vec<String> = vec![];
        let preview = PreviewService::generate_preview(temp_dir.path(), &files, None);

        assert!(
            preview.is_empty(),
            "空のファイルリストでは空のプレビューを返すべき"
        );
    }

    #[test]
    fn test_truncate_to_chars_empty_string() {
        let result = PreviewService::truncate_to_chars("", 10);
        assert_eq!(result, "", "空文字列はそのまま返されるべき");
    }

    #[test]
    fn test_truncate_to_chars_single_char() {
        let result = PreviewService::truncate_to_chars("a", 1);
        assert_eq!(result, "a", "1文字で最大1文字ならそのまま返されるべき");
    }
}
