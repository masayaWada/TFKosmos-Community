use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use tokio::process::Command;

use crate::models::{AzureResourceGroup, AzureSubscription, ConnectionTestResponse};

pub struct AzureClientFactory;

impl AzureClientFactory {
    /// Azure CLIコマンドを実行してJSONを取得
    async fn execute_az_command(args: &[&str]) -> Result<Value> {
        let output = Command::new("az")
            .args(args)
            .output()
            .await
            .context("Azure CLIがインストールされていないか、PATHに含まれていません")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Azure CLIコマンドが失敗しました: {}", stderr);
        }

        let stdout = String::from_utf8(output.stdout)
            .context("Azure CLIの出力をUTF-8として解析できませんでした")?;

        let json: Value = serde_json::from_str(&stdout)
            .context("Azure CLIの出力をJSONとして解析できませんでした")?;

        Ok(json)
    }

    pub async fn test_connection(
        _auth_method: Option<String>,
        _tenant_id: Option<String>,
        _service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<ConnectionTestResponse> {
        // Azure CLIで現在のアカウント情報を取得
        let json = Self::execute_az_command(&["account", "show", "--output", "json"]).await?;

        let subscription_id = json
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let subscription_name = json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ConnectionTestResponse {
            success: true,
            message: Some("Connection successful".to_string()),
            account_id: subscription_id.clone(),
            user_arn: None,
            subscription_name,
        })
    }

    pub async fn list_subscriptions(
        _auth_method: Option<String>,
        _tenant_id: Option<String>,
        _service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<Vec<AzureSubscription>> {
        // Azure CLIでサブスクリプション一覧を取得
        let json = Self::execute_az_command(&["account", "list", "--output", "json"]).await?;

        let subscriptions = json
            .as_array()
            .context("サブスクリプション一覧が配列形式ではありません")?
            .iter()
            .filter_map(|sub| {
                Some(AzureSubscription {
                    subscription_id: sub.get("id")?.as_str()?.to_string(),
                    display_name: sub.get("name")?.as_str()?.to_string(),
                    state: sub
                        .get("state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                })
            })
            .collect();

        Ok(subscriptions)
    }

    pub async fn list_resource_groups(
        subscription_id: String,
        _auth_method: Option<String>,
        _tenant_id: Option<String>,
        _service_principal_config: Option<HashMap<String, String>>,
    ) -> Result<Vec<AzureResourceGroup>> {
        // Azure CLIでリソースグループ一覧を取得
        let json = Self::execute_az_command(&[
            "group",
            "list",
            "--subscription",
            &subscription_id,
            "--output",
            "json",
        ])
        .await?;

        let resource_groups = json
            .as_array()
            .context("リソースグループ一覧が配列形式ではありません")?
            .iter()
            .filter_map(|rg| {
                Some(AzureResourceGroup {
                    name: rg.get("name")?.as_str()?.to_string(),
                    location: rg
                        .get("location")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                })
            })
            .collect();

        Ok(resource_groups)
    }
}
