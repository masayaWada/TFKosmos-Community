use crate::api::error::ApiError;

/// AWSプロファイル名のバリデーション
/// 英数字・ハイフン・アンダースコアのみ許可（コマンドインジェクション防御）
pub fn validate_aws_profile_name(profile: &str) -> Result<(), ApiError> {
    if profile.is_empty() {
        let mut fields = std::collections::HashMap::new();
        fields.insert(
            "profile".to_string(),
            "Profile name cannot be empty".to_string(),
        );
        return Err(ApiError::ValidationFields {
            message: "Validation failed".to_string(),
            fields,
        });
    }

    if !profile
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        let mut fields = std::collections::HashMap::new();
        fields.insert(
            "profile".to_string(),
            "Invalid AWS profile name: only alphanumeric characters, hyphens (-), and underscores (_) are allowed".to_string(),
        );
        return Err(ApiError::ValidationFields {
            message: "Validation failed".to_string(),
            fields,
        });
    }

    Ok(())
}

/// テンプレート名のバリデーション
/// パストラバーサル攻撃を防御
pub fn validate_template_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() {
        return Err(ApiError::Validation(
            "Template name cannot be empty".to_string(),
        ));
    }

    // NULバイトチェック
    if name.contains('\0') {
        return Err(ApiError::Validation(
            "Template name contains invalid characters (null byte)".to_string(),
        ));
    }

    // パストラバーサル: .. を含む場合は拒否
    if name.contains("..") {
        return Err(ApiError::Validation(
            "Template name cannot contain '..' (path traversal attempt detected)".to_string(),
        ));
    }

    // 絶対パスを拒否
    if name.starts_with('/') {
        return Err(ApiError::Validation(
            "Template name cannot be an absolute path".to_string(),
        ));
    }

    // Windowsドライブレターを拒否 (例: C:/...)
    if name.len() >= 2 {
        let mut chars = name.chars();
        let first = chars.next().unwrap_or_default();
        let second = chars.next().unwrap_or_default();
        if first.is_ascii_alphabetic() && second == ':' {
            return Err(ApiError::Validation(
                "Template name cannot be an absolute path".to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // AWS プロファイル名バリデーション
    // ========================================

    #[test]
    fn test_valid_aws_profile_names() {
        assert!(validate_aws_profile_name("default").is_ok());
        assert!(validate_aws_profile_name("my-profile").is_ok());
        assert!(validate_aws_profile_name("test_123").is_ok());
        assert!(validate_aws_profile_name("MyProfile").is_ok());
        assert!(validate_aws_profile_name("dev-us-east-1").is_ok());
        assert!(validate_aws_profile_name("prod_account_123").is_ok());
    }

    #[test]
    fn test_invalid_aws_profile_name_empty() {
        let result = validate_aws_profile_name("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ApiError::ValidationFields { fields, .. } if fields.get("profile").map(|v| v.contains("empty")).unwrap_or(false)),
            "Empty profile name should return ValidationFields with 'empty' in profile field: {:?}",
            err
        );
    }

    #[test]
    fn test_invalid_aws_profile_name_semicolon() {
        assert!(validate_aws_profile_name("profile;rm -rf /").is_err());
    }

    #[test]
    fn test_invalid_aws_profile_name_pipe() {
        assert!(validate_aws_profile_name("profile|bash").is_err());
    }

    #[test]
    fn test_invalid_aws_profile_name_ampersand() {
        assert!(validate_aws_profile_name("profile&command").is_err());
    }

    #[test]
    fn test_invalid_aws_profile_name_dollar() {
        assert!(validate_aws_profile_name("$ENV_VAR").is_err());
    }

    #[test]
    fn test_invalid_aws_profile_name_backtick() {
        assert!(validate_aws_profile_name("`command`").is_err());
    }

    #[test]
    fn test_invalid_aws_profile_name_space() {
        assert!(validate_aws_profile_name("my profile").is_err());
    }

    #[test]
    fn test_invalid_aws_profile_name_slash() {
        assert!(validate_aws_profile_name("../../etc/passwd").is_err());
    }

    #[test]
    fn test_invalid_aws_profile_name_newline() {
        assert!(validate_aws_profile_name("profile\ncommand").is_err());
    }

    #[test]
    fn test_invalid_aws_profile_name_parentheses() {
        assert!(validate_aws_profile_name("profile(x)").is_err());
    }

    // ========================================
    // テンプレート名バリデーション
    // ========================================

    #[test]
    fn test_valid_template_names() {
        assert!(validate_template_name("aws/iam_user.tf.j2").is_ok());
        assert!(validate_template_name("azure/role.tf.j2").is_ok());
        assert!(validate_template_name("iam_policy.tf.j2").is_ok());
        assert!(validate_template_name("simple.j2").is_ok());
        assert!(validate_template_name("aws/iam-role.tf.j2").is_ok());
    }

    #[test]
    fn test_invalid_template_name_empty() {
        assert!(validate_template_name("").is_err());
    }

    #[test]
    fn test_invalid_template_name_path_traversal_dotdot() {
        assert!(validate_template_name("../../../etc/passwd").is_err());
    }

    #[test]
    fn test_invalid_template_name_path_traversal_embedded() {
        assert!(validate_template_name("aws/../../secret").is_err());
    }

    #[test]
    fn test_invalid_template_name_absolute_path_unix() {
        assert!(validate_template_name("/etc/passwd").is_err());
    }

    #[test]
    fn test_invalid_template_name_null_byte() {
        assert!(validate_template_name("template\x00name").is_err());
    }

    #[test]
    fn test_invalid_template_name_windows_absolute() {
        assert!(validate_template_name("C:/Windows/System32").is_err());
    }

    #[test]
    fn test_invalid_template_name_windows_drive_lowercase() {
        assert!(validate_template_name("c:/secret").is_err());
    }
}
