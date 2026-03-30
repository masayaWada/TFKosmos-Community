use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScanResponse {
    pub scan_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<std::collections::HashMap<String, usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GenerationResponse {
    pub generation_id: String,
    pub output_path: String,
    pub files: Vec<String>,
    pub import_script_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceListResponse {
    pub resources: Vec<serde_json::Value>,
    pub total: usize,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConnectionTestResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_name: Option<String>, // Azure用
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AzureSubscription {
    pub subscription_id: String,
    pub display_name: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AzureResourceGroup {
    pub name: String,
    pub location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ValidationError {
    pub error_type: String, // "jinja2" | "terraform"
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TemplateValidationResponse {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DependencyNode {
    pub id: String,
    pub node_type: String, // "user", "group", "role", "policy"
    pub name: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DependencyEdge {
    pub source: String,
    pub target: String,
    pub edge_type: String, // "policy_attachment", "group_membership"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DependencyGraph {
    pub nodes: Vec<DependencyNode>,
    pub edges: Vec<DependencyEdge>,
}
