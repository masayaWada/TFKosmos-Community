use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// クラウドプロバイダーの種類
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    Aws,
    Azure,
}

impl std::fmt::Display for CloudProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CloudProvider::Aws => write!(f, "aws"),
            CloudProvider::Azure => write!(f, "azure"),
        }
    }
}

/// プロバイダー固有の設定を型安全に表現するenum
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProviderConfig {
    Aws(AwsProviderConfig),
    Azure(AzureProviderConfig),
}

/// AWS固有の設定
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AwsProviderConfig {
    pub account_id: Option<String>,
    pub profile: Option<String>,
    pub assume_role_arn: Option<String>,
    pub assume_role_session_name: Option<String>,
}

/// Azure固有の設定
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AzureProviderConfig {
    pub subscription_id: Option<String>,
    pub tenant_id: Option<String>,
    pub auth_method: Option<String>,
    pub service_principal_config: Option<HashMap<String, String>>,
    pub scope_type: Option<String>,
    pub scope_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScanConfig {
    pub provider: String, // "aws" or "azure"

    // AWS specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assume_role_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assume_role_session_name: Option<String>,

    // Azure specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<String>, // "az_login", "service_principal"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_principal_config: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_type: Option<String>, // "management_group", "subscription", "resource_group"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_value: Option<String>,

    // Common
    #[serde(default)]
    pub scan_targets: HashMap<String, bool>,
    #[serde(default)]
    pub filters: HashMap<String, String>,

    // Performance options
    /// タグ情報を取得するかどうか（デフォルト: true）
    /// 大規模環境ではfalseにすることでスキャン速度が向上
    #[serde(default = "default_true")]
    pub include_tags: bool,
}

impl ScanConfig {
    /// プロバイダー種別をenum型で取得
    #[allow(dead_code)]
    pub fn cloud_provider(&self) -> Option<CloudProvider> {
        match self.provider.as_str() {
            "aws" => Some(CloudProvider::Aws),
            "azure" => Some(CloudProvider::Azure),
            _ => None,
        }
    }

    /// プロバイダー固有の設定を型安全に取得
    #[allow(dead_code)]
    pub fn provider_config(&self) -> Option<ProviderConfig> {
        match self.provider.as_str() {
            "aws" => Some(ProviderConfig::Aws(AwsProviderConfig {
                account_id: self.account_id.clone(),
                profile: self.profile.clone(),
                assume_role_arn: self.assume_role_arn.clone(),
                assume_role_session_name: self.assume_role_session_name.clone(),
            })),
            "azure" => Some(ProviderConfig::Azure(AzureProviderConfig {
                subscription_id: self.subscription_id.clone(),
                tenant_id: self.tenant_id.clone(),
                auth_method: self.auth_method.clone(),
                service_principal_config: self.service_principal_config.clone(),
                scope_type: self.scope_type.clone(),
                scope_value: self.scope_value.clone(),
            })),
            _ => None,
        }
    }

    /// AWSプロバイダーかどうか
    #[allow(dead_code)]
    pub fn is_aws(&self) -> bool {
        self.provider == "aws"
    }

    /// Azureプロバイダーかどうか
    #[allow(dead_code)]
    pub fn is_azure(&self) -> bool {
        self.provider == "azure"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GenerationConfig {
    pub output_path: String,
    #[serde(default = "default_file_split_rule")]
    pub file_split_rule: String, // "single", "by_resource_type", "by_resource_name", "by_resource_group", "by_subscription"
    #[serde(default = "default_naming_convention")]
    pub naming_convention: String, // "snake_case", "kebab-case", "original"
    #[serde(default = "default_import_script_format")]
    pub import_script_format: String, // "sh", "ps1"
    #[serde(default = "default_true")]
    pub generate_readme: bool,
    #[serde(default)]
    pub selected_resources: HashMap<String, Vec<serde_json::Value>>,
}

fn default_file_split_rule() -> String {
    "single".to_string()
}

fn default_naming_convention() -> String {
    "snake_case".to_string()
}

fn default_import_script_format() -> String {
    "sh".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_aws_config() -> ScanConfig {
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
            scan_targets: HashMap::new(),
            filters: HashMap::new(),
            include_tags: true,
        }
    }

    fn make_azure_config() -> ScanConfig {
        ScanConfig {
            provider: "azure".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: Some("sub-123".to_string()),
            tenant_id: Some("tenant-456".to_string()),
            auth_method: Some("az_login".to_string()),
            service_principal_config: None,
            scope_type: Some("subscription".to_string()),
            scope_value: Some("sub-123".to_string()),
            scan_targets: HashMap::new(),
            filters: HashMap::new(),
            include_tags: true,
        }
    }

    #[test]
    fn test_cloud_provider_aws() {
        let config = make_aws_config();
        assert_eq!(config.cloud_provider(), Some(CloudProvider::Aws));
        assert!(config.is_aws());
        assert!(!config.is_azure());
    }

    #[test]
    fn test_cloud_provider_azure() {
        let config = make_azure_config();
        assert_eq!(config.cloud_provider(), Some(CloudProvider::Azure));
        assert!(!config.is_aws());
        assert!(config.is_azure());
    }

    #[test]
    fn test_cloud_provider_unknown() {
        let mut config = make_aws_config();
        config.provider = "gcp".to_string();
        assert_eq!(config.cloud_provider(), None);
        assert!(!config.is_aws());
        assert!(!config.is_azure());
    }

    #[test]
    fn test_provider_config_aws() {
        let config = make_aws_config();
        match config.provider_config() {
            Some(ProviderConfig::Aws(aws)) => {
                assert_eq!(aws.account_id, Some("123456789012".to_string()));
                assert_eq!(aws.profile, Some("default".to_string()));
                assert!(aws.assume_role_arn.is_none());
            }
            _ => panic!("Expected ProviderConfig::Aws"),
        }
    }

    #[test]
    fn test_provider_config_azure() {
        let config = make_azure_config();
        match config.provider_config() {
            Some(ProviderConfig::Azure(azure)) => {
                assert_eq!(azure.subscription_id, Some("sub-123".to_string()));
                assert_eq!(azure.tenant_id, Some("tenant-456".to_string()));
                assert_eq!(azure.auth_method, Some("az_login".to_string()));
                assert_eq!(azure.scope_type, Some("subscription".to_string()));
            }
            _ => panic!("Expected ProviderConfig::Azure"),
        }
    }

    #[test]
    fn test_provider_config_unknown_returns_none() {
        let mut config = make_aws_config();
        config.provider = "unknown".to_string();
        assert!(config.provider_config().is_none());
    }

    #[test]
    fn test_cloud_provider_display() {
        assert_eq!(CloudProvider::Aws.to_string(), "aws");
        assert_eq!(CloudProvider::Azure.to_string(), "azure");
    }

    #[test]
    fn test_cloud_provider_serde_roundtrip() {
        let aws = CloudProvider::Aws;
        let json = serde_json::to_string(&aws).unwrap();
        assert_eq!(json, "\"aws\"");
        let deserialized: CloudProvider = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, CloudProvider::Aws);

        let azure = CloudProvider::Azure;
        let json = serde_json::to_string(&azure).unwrap();
        assert_eq!(json, "\"azure\"");
        let deserialized: CloudProvider = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, CloudProvider::Azure);
    }
}
