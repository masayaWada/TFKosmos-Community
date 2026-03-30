#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Azure 仮想ネットワーク
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureVirtualNetwork {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_space: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnets: Option<Vec<AzureSubnet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>,
}

/// Azure サブネット
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureSubnet {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_security_group_id: Option<String>,
}

/// Azure ネットワークセキュリティグループ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureNetworkSecurityGroup {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_rules: Option<Vec<AzureSecurityRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>,
}

/// Azure セキュリティルール
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureSecurityRule {
    pub name: String,
    pub priority: i32,
    pub direction: String,
    pub access: String,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_port_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_port_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_address_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_address_prefix: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_vnet_serde_roundtrip() {
        let vnet = AzureVirtualNetwork {
            id: "/subscriptions/123/resourceGroups/rg/providers/Microsoft.Network/virtualNetworks/vnet1".to_string(),
            name: "vnet1".to_string(),
            location: Some("japaneast".to_string()),
            resource_group: Some("rg".to_string()),
            address_space: Some(vec!["10.0.0.0/16".to_string()]),
            subnets: Some(vec![AzureSubnet {
                id: "subnet-1".to_string(),
                name: "default".to_string(),
                address_prefix: Some("10.0.0.0/24".to_string()),
                network_security_group_id: None,
            }]),
            tags: None,
        };

        let json = serde_json::to_string(&vnet).unwrap();
        let deserialized: AzureVirtualNetwork = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "vnet1");
        assert_eq!(deserialized.subnets.unwrap().len(), 1);
    }

    #[test]
    fn test_azure_nsg_serde_roundtrip() {
        let nsg = AzureNetworkSecurityGroup {
            id: "nsg-1".to_string(),
            name: "web-nsg".to_string(),
            location: Some("japaneast".to_string()),
            resource_group: Some("rg".to_string()),
            security_rules: Some(vec![AzureSecurityRule {
                name: "AllowHTTPS".to_string(),
                priority: 100,
                direction: "Inbound".to_string(),
                access: "Allow".to_string(),
                protocol: "Tcp".to_string(),
                source_port_range: Some("*".to_string()),
                destination_port_range: Some("443".to_string()),
                source_address_prefix: Some("*".to_string()),
                destination_address_prefix: Some("*".to_string()),
            }]),
            tags: None,
        };

        let json = serde_json::to_string(&nsg).unwrap();
        let deserialized: AzureNetworkSecurityGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "web-nsg");
        assert_eq!(deserialized.security_rules.unwrap()[0].priority, 100);
    }
}
