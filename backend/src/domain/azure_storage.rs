#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Azure ストレージアカウント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureStorageAccount {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_tier: Option<String>,
    #[serde(default)]
    pub https_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_tls_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_storage_account_serde_roundtrip() {
        let sa = AzureStorageAccount {
            id: "/subscriptions/123/resourceGroups/rg/providers/Microsoft.Storage/storageAccounts/sa1".to_string(),
            name: "sa1".to_string(),
            location: Some("japaneast".to_string()),
            resource_group: Some("rg".to_string()),
            kind: Some("StorageV2".to_string()),
            sku_name: Some("Standard_LRS".to_string()),
            access_tier: Some("Hot".to_string()),
            https_only: true,
            minimum_tls_version: Some("TLS1_2".to_string()),
            tags: Some(HashMap::from([("env".to_string(), "prod".to_string())])),
        };

        let json = serde_json::to_string(&sa).unwrap();
        let deserialized: AzureStorageAccount = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "sa1");
        assert!(deserialized.https_only);
    }
}
