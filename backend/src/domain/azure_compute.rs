#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Azure 仮想マシン
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureVirtualMachine {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_disk_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_reference: Option<AzureImageReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_interfaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_set_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>,
}

/// Azure イメージ参照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureImageReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_vm_serde_roundtrip() {
        let vm = AzureVirtualMachine {
            id: "/subscriptions/123/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/vm1".to_string(),
            name: "vm1".to_string(),
            location: Some("japaneast".to_string()),
            resource_group: Some("rg".to_string()),
            vm_size: Some("Standard_B2s".to_string()),
            os_type: Some("Linux".to_string()),
            os_disk_type: Some("Premium_LRS".to_string()),
            image_reference: Some(AzureImageReference {
                publisher: Some("Canonical".to_string()),
                offer: Some("0001-com-ubuntu-server-jammy".to_string()),
                sku: Some("22_04-lts".to_string()),
                version: Some("latest".to_string()),
            }),
            admin_username: Some("azureuser".to_string()),
            network_interfaces: Some(vec!["nic-1".to_string()]),
            availability_set_id: None,
            tags: Some(HashMap::from([("env".to_string(), "prod".to_string())])),
        };

        let json = serde_json::to_string(&vm).unwrap();
        let deserialized: AzureVirtualMachine = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "vm1");
        assert_eq!(deserialized.vm_size, Some("Standard_B2s".to_string()));
    }
}
