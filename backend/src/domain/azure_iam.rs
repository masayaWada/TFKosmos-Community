#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Azure ロール定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureRoleDefinition {
    pub role_definition_id: String,
    pub role_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_type: Option<String>, // "BuiltInRole" or "CustomRole"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<AzurePermission>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignable_scopes: Option<Vec<String>>,
}

/// Azure ロール割り当て
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureRoleAssignment {
    pub role_assignment_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_definition_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal_type: Option<String>, // "User", "Group", "ServicePrincipal"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
}

/// Azure パーミッション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzurePermission {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub not_actions: Vec<String>,
    #[serde(default)]
    pub data_actions: Vec<String>,
    #[serde(default)]
    pub not_data_actions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_role_definition_serde_roundtrip() {
        let role_def = AzureRoleDefinition {
            role_definition_id:
                "/subscriptions/123/providers/Microsoft.Authorization/roleDefinitions/abc"
                    .to_string(),
            role_name: "Custom Contributor".to_string(),
            description: Some("A custom contributor role".to_string()),
            role_type: Some("CustomRole".to_string()),
            scope: Some("/subscriptions/123".to_string()),
            permissions: Some(vec![AzurePermission {
                actions: vec!["Microsoft.Resources/subscriptions/read".to_string()],
                not_actions: vec![],
                data_actions: vec![],
                not_data_actions: vec![],
            }]),
            assignable_scopes: Some(vec!["/subscriptions/123".to_string()]),
        };

        let json = serde_json::to_string(&role_def).unwrap();
        let deserialized: AzureRoleDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role_name, "Custom Contributor");
        assert_eq!(deserialized.permissions.unwrap()[0].actions.len(), 1);
    }

    #[test]
    fn test_azure_role_assignment_serde_roundtrip() {
        let assignment = AzureRoleAssignment {
            role_assignment_id:
                "/subscriptions/123/providers/Microsoft.Authorization/roleAssignments/def"
                    .to_string(),
            role_definition_id: Some(
                "/subscriptions/123/providers/Microsoft.Authorization/roleDefinitions/abc"
                    .to_string(),
            ),
            principal_id: Some("user-principal-id".to_string()),
            principal_type: Some("User".to_string()),
            scope: Some("/subscriptions/123".to_string()),
            role_name: Some("Contributor".to_string()),
        };

        let json = serde_json::to_string(&assignment).unwrap();
        let deserialized: AzureRoleAssignment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.principal_type, Some("User".to_string()));
    }

    #[test]
    fn test_azure_role_definition_minimal() {
        let json = r#"{
            "role_definition_id": "/providers/Microsoft.Authorization/roleDefinitions/xyz",
            "role_name": "MinimalRole"
        }"#;

        let role_def: AzureRoleDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(role_def.role_name, "MinimalRole");
        assert!(role_def.description.is_none());
        assert!(role_def.permissions.is_none());
    }

    #[test]
    fn test_azure_permission_serde() {
        let perm = AzurePermission {
            actions: vec![
                "Microsoft.Compute/virtualMachines/read".to_string(),
                "Microsoft.Compute/virtualMachines/write".to_string(),
            ],
            not_actions: vec!["Microsoft.Compute/virtualMachines/delete".to_string()],
            data_actions: vec![],
            not_data_actions: vec![],
        };

        let json = serde_json::to_string(&perm).unwrap();
        let deserialized: AzurePermission = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.actions.len(), 2);
        assert_eq!(deserialized.not_actions.len(), 1);
    }
}
