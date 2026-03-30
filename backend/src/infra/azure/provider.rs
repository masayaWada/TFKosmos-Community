//! Azure プロバイダーの CloudProviderScanner 実装

use anyhow::Result;
use async_trait::async_trait;

use crate::infra::azure::scanner::AzureIamScanner;
use crate::infra::provider_trait::{CloudProviderScanner, ResourceTemplate};
use crate::models::ScanConfig;

/// Azure プロバイダー
pub struct AzureProvider;

#[async_trait]
impl CloudProviderScanner for AzureProvider {
    fn provider_name(&self) -> &'static str {
        "azure"
    }

    fn get_templates(&self) -> Vec<ResourceTemplate> {
        vec![
            // IAM
            ResourceTemplate {
                resource_type: "role_definitions",
                template_path: "azure/role_definition.tf.j2",
                provider: "azure",
            },
            ResourceTemplate {
                resource_type: "role_assignments",
                template_path: "azure/role_assignment.tf.j2",
                provider: "azure",
            },
            // Compute
            ResourceTemplate {
                resource_type: "virtual_machines",
                template_path: "azure/virtual_machine.tf.j2",
                provider: "azure",
            },
            // Network
            ResourceTemplate {
                resource_type: "virtual_networks",
                template_path: "azure/virtual_network.tf.j2",
                provider: "azure",
            },
            ResourceTemplate {
                resource_type: "network_security_groups",
                template_path: "azure/nsg.tf.j2",
                provider: "azure",
            },
            // Storage
            ResourceTemplate {
                resource_type: "storage_accounts",
                template_path: "azure/storage_account.tf.j2",
                provider: "azure",
            },
            // SQL
            ResourceTemplate {
                resource_type: "sql_databases",
                template_path: "azure/sql_database.tf.j2",
                provider: "azure",
            },
            // App Service
            ResourceTemplate {
                resource_type: "app_services",
                template_path: "azure/app_service.tf.j2",
                provider: "azure",
            },
            ResourceTemplate {
                resource_type: "function_apps",
                template_path: "azure/function_app.tf.j2",
                provider: "azure",
            },
        ]
    }

    async fn scan(
        &self,
        config: ScanConfig,
        callback: Box<dyn Fn(u32, String) + Send + Sync>,
    ) -> Result<serde_json::Value> {
        let scanner = AzureIamScanner::new(config).await?;
        scanner.scan(callback).await
    }
}
