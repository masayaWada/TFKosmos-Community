#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Azure App Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureAppService {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_host_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub https_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_stack: Option<String>,
}

/// Azure Function App
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureFunctionApp {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_plan_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_service_serde_roundtrip() {
        let app = AzureAppService {
            name: "my-web-app".to_string(),
            id: Some(
                "/subscriptions/123/resourceGroups/rg/providers/Microsoft.Web/sites/my-web-app"
                    .to_string(),
            ),
            resource_group: Some("my-rg".to_string()),
            location: Some("japaneast".to_string()),
            kind: Some("app".to_string()),
            state: Some("Running".to_string()),
            default_host_name: Some("my-web-app.azurewebsites.net".to_string()),
            service_plan_id: Some(
                "/subscriptions/123/resourceGroups/rg/providers/Microsoft.Web/serverfarms/my-plan"
                    .to_string(),
            ),
            https_only: Some(true),
            runtime_stack: Some("DOTNET|8.0".to_string()),
        };

        let json = serde_json::to_string(&app).unwrap();
        let deserialized: AzureAppService = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "my-web-app");
    }

    #[test]
    fn test_function_app_serde_roundtrip() {
        let func = AzureFunctionApp {
            name: "my-func-app".to_string(),
            id: Some(
                "/subscriptions/123/resourceGroups/rg/providers/Microsoft.Web/sites/my-func-app"
                    .to_string(),
            ),
            resource_group: Some("my-rg".to_string()),
            location: Some("japaneast".to_string()),
            kind: Some("functionapp".to_string()),
            state: Some("Running".to_string()),
            runtime: Some("python".to_string()),
            service_plan_id: None,
        };

        let json = serde_json::to_string(&func).unwrap();
        let deserialized: AzureFunctionApp = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "my-func-app");
    }
}
