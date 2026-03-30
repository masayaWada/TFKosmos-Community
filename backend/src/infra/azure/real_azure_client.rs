//! Azure CLI/API クライアントの本番実装
//!
//! このモジュールは、`AzureClientOps`トレイトの本番実装を提供します。

use anyhow::{Context, Result};
use async_trait::async_trait;
use azure_core::credentials::TokenCredential;
use azure_identity::AzureCliCredential;
use reqwest::Client as HttpClient;
use serde_json::Value;
use tokio::process::Command;

use super::azure_client_trait::AzureClientOps;

/// Azure CLI/API クライアントをラップした本番実装
pub struct RealAzureClient {
    http_client: Option<HttpClient>,
}

impl RealAzureClient {
    pub fn new() -> Self {
        let http_client = HttpClient::builder().build().ok();
        Self { http_client }
    }
}

impl Default for RealAzureClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AzureClientOps for RealAzureClient {
    async fn execute_az_command(&self, args: Vec<String>) -> Result<Value> {
        let output = Command::new("az")
            .args(&args)
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

    async fn get_auth_token(&self, scope: &str) -> Option<String> {
        let credential = match AzureCliCredential::new(None) {
            Ok(cred) => cred,
            Err(_) => return None,
        };

        let scopes = &[scope];
        let token_response = match credential.get_token(scopes, None).await {
            Ok(token) => token,
            Err(_) => return None,
        };
        Some(token_response.token.secret().to_string())
    }

    fn get_http_client(&self) -> Option<HttpClient> {
        self.http_client.clone()
    }

    async fn get_principal_display_name(
        &self,
        principal_id: &str,
        principal_type: Option<String>,
        token: &str,
    ) -> Option<String> {
        let http_client = match &self.http_client {
            Some(client) => client,
            None => return None,
        };

        // Microsoft Graph APIのエンドポイントを決定
        let endpoint = match principal_type.as_deref() {
            Some("User") => format!("https://graph.microsoft.com/v1.0/users/{}", principal_id),
            Some("ServicePrincipal") => format!(
                "https://graph.microsoft.com/v1.0/servicePrincipals/{}",
                principal_id
            ),
            _ => return None,
        };

        // APIリクエストを送信
        let response = match http_client
            .get(&endpoint)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(_) => return None,
        };

        // レスポンスをJSONとして解析
        let json: Value = match response.json().await {
            Ok(json) => json,
            Err(_) => return None,
        };

        // 表示名を取得
        if let Some(display_name) = json.get("displayName") {
            display_name.as_str().map(|s| s.to_string())
        } else if let Some(app_display_name) = json.get("appDisplayName") {
            app_display_name.as_str().map(|s| s.to_string())
        } else {
            None
        }
    }

    async fn get_role_display_name(
        &self,
        role_definition_id: &str,
        subscription_id: Option<String>,
        token: &str,
    ) -> Option<String> {
        let http_client = match &self.http_client {
            Some(client) => client,
            None => return None,
        };

        // roleDefinitionIdの形式: /subscriptions/{subId}/providers/Microsoft.Authorization/roleDefinitions/{roleId}
        // または単にroleIdのみの場合もある

        let (sub_id, role_id) = if role_definition_id.starts_with("/subscriptions/") {
            // フルパスの場合
            if let Some(role_id_start) = role_definition_id.rfind('/') {
                let role_id = &role_definition_id[role_id_start + 1..];
                let sub_id_start =
                    role_definition_id.find("/subscriptions/").unwrap() + "/subscriptions/".len();
                let sub_id_end = role_definition_id[sub_id_start..]
                    .find('/')
                    .unwrap_or(role_definition_id.len() - sub_id_start);
                let sub_id = &role_definition_id[sub_id_start..sub_id_start + sub_id_end];
                (Some(sub_id.to_string()), role_id.to_string())
            } else {
                return None;
            }
        } else {
            // roleIdのみの場合
            (subscription_id, role_definition_id.to_string())
        };

        let sub_id = match sub_id {
            Some(id) => id,
            None => return None,
        };

        // Azure Management APIのエンドポイント
        let endpoint = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Authorization/roleDefinitions/{}?api-version=2022-04-01",
            sub_id, role_id
        );

        // APIリクエストを送信（日本語ロケールを指定）
        let response = match http_client
            .get(&endpoint)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept-Language", "ja-JP")
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(_) => return None,
        };

        // レスポンスをJSONとして解析
        let json: Value = match response.json().await {
            Ok(json) => json,
            Err(_) => return None,
        };

        // 表示名を取得（properties.displayNameが存在する場合はそれを使用、存在しない場合はproperties.roleNameを使用）
        let properties = json.get("properties");
        if let Some(props) = properties {
            // displayNameが存在する場合はそれを使用（ローカライズされた名前、日本語）
            if let Some(display_name_localized) = props.get("displayName") {
                if let Some(name) = display_name_localized.as_str() {
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
            // displayNameが存在しない、または空の場合はroleNameを使用（英語名）
            if let Some(role_name) = props.get("roleName") {
                if let Some(name) = role_name.as_str() {
                    return Some(name.to_string());
                }
            }
        }
        None
    }
}
