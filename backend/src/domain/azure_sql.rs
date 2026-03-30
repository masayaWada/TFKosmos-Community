#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Azure SQL Database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureSqlDatabase {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size_gb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>,
}

/// Azure SQL Server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureSqlServer {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub administrator_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fully_qualified_domain_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_sql_database_serde_roundtrip() {
        let db = AzureSqlDatabase {
            id: "/subscriptions/123/resourceGroups/rg/providers/Microsoft.Sql/servers/srv/databases/db1".to_string(),
            name: "db1".to_string(),
            location: Some("japaneast".to_string()),
            resource_group: Some("rg".to_string()),
            server_name: Some("srv".to_string()),
            sku_name: Some("S0".to_string()),
            max_size_gb: Some(250.0),
            collation: Some("SQL_Latin1_General_CP1_CI_AS".to_string()),
            status: Some("Online".to_string()),
            tags: None,
        };

        let json = serde_json::to_string(&db).unwrap();
        let deserialized: AzureSqlDatabase = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "db1");
        assert_eq!(deserialized.server_name, Some("srv".to_string()));
    }

    #[test]
    fn test_azure_sql_server_serde_roundtrip() {
        let server = AzureSqlServer {
            id: "server-id".to_string(),
            name: "my-sql-server".to_string(),
            location: Some("japaneast".to_string()),
            resource_group: Some("rg".to_string()),
            administrator_login: Some("sqladmin".to_string()),
            version: Some("12.0".to_string()),
            fully_qualified_domain_name: Some("my-sql-server.database.windows.net".to_string()),
            tags: None,
        };

        let json = serde_json::to_string(&server).unwrap();
        let deserialized: AzureSqlServer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "my-sql-server");
    }
}
