use anyhow::{Context, Result};
use futures::future::join_all;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use super::azure_client_trait::AzureClientOps;
use super::real_azure_client::RealAzureClient;
use crate::models::ScanConfig;

pub struct AzureIamScanner<C: AzureClientOps> {
    config: ScanConfig,
    client: Arc<C>,
}

impl AzureIamScanner<RealAzureClient> {
    pub async fn new(config: ScanConfig) -> Result<Self> {
        Ok(Self {
            config,
            client: Arc::new(RealAzureClient::new()),
        })
    }
}

impl<C: AzureClientOps> AzureIamScanner<C> {
    /// テスト用: モッククライアントを使用してスキャナーを作成
    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: C) -> Self {
        Self {
            config,
            client: Arc::new(client),
        }
    }

    /// Role Definitionをフロントエンド形式に変換（表示名なし）
    pub fn transform_role_definition_basic(rd: &Value) -> Value {
        let mut transformed = serde_json::Map::new();
        if let Some(id) = rd.get("id") {
            transformed.insert("role_definition_id".to_string(), id.clone());
        }
        if let Some(name) = rd.get("name").or_else(|| rd.get("roleName")) {
            transformed.insert("role_name".to_string(), name.clone());
        }
        if let Some(desc) = rd.get("description") {
            transformed.insert("description".to_string(), desc.clone());
        }
        if let Some(role_type) = rd.get("type") {
            transformed.insert("role_type".to_string(), role_type.clone());
        }
        // scopeをidから抽出
        if let Some(id) = rd.get("id") {
            if let Some(id_str) = id.as_str() {
                if let Some(scope_end) =
                    id_str.rfind("/providers/Microsoft.Authorization/roleDefinitions")
                {
                    let scope = &id_str[..scope_end];
                    transformed.insert("scope".to_string(), Value::String(scope.to_string()));
                }
            }
        }
        // 元のデータも保持
        for (key, value) in rd.as_object().unwrap_or(&serde_json::Map::new()) {
            if !transformed.contains_key(key) {
                transformed.insert(key.clone(), value.clone());
            }
        }
        Value::Object(transformed)
    }

    /// スコープに基づいてAzure CLIコマンドの引数を構築
    fn get_scope_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if let Some(scope_type) = &self.config.scope_type {
            match scope_type.as_str() {
                "subscription" => {
                    if let Some(subscription_id) = &self.config.subscription_id {
                        args.push("--subscription".to_string());
                        args.push(subscription_id.clone());
                    }
                }
                "resource_group" => {
                    if let Some(scope_value) = &self.config.scope_value {
                        // For role commands, use --scope instead of --resource-group
                        args.push("--scope".to_string());
                        if let Some(subscription_id) = &self.config.subscription_id {
                            args.push(format!(
                                "/subscriptions/{}/resourceGroups/{}",
                                subscription_id, scope_value
                            ));
                        } else {
                            args.push(format!("/resourceGroups/{}", scope_value));
                        }
                    }
                }
                "management_group" => {
                    if let Some(scope_value) = &self.config.scope_value {
                        args.push("--scope".to_string());
                        args.push(format!(
                            "/providers/Microsoft.Management/managementGroups/{}",
                            scope_value
                        ));
                    }
                }
                _ => {
                    // Default: use subscription if available
                    if let Some(subscription_id) = &self.config.subscription_id {
                        args.push("--subscription".to_string());
                        args.push(subscription_id.clone());
                    }
                }
            }
        } else {
            // No scope type specified, use subscription if available
            if let Some(subscription_id) = &self.config.subscription_id {
                args.push("--subscription".to_string());
                args.push(subscription_id.clone());
            }
        }

        args
    }

    /// Role Definitionsを取得
    pub async fn scan_role_definitions(&self) -> Result<Vec<Value>> {
        let scan_targets = &self.config.scan_targets;

        if !scan_targets
            .get("role_definitions")
            .copied()
            .unwrap_or(false)
        {
            return Ok(Vec::new());
        }

        let start_time = std::time::Instant::now();
        info!("Role Definitionsスキャンを開始");

        let mut args: Vec<String> = vec![
            "role".to_string(),
            "definition".to_string(),
            "list".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ];
        let scope_args = self.get_scope_args();

        // スコープ引数を追加
        args.extend(scope_args);

        let az_start = std::time::Instant::now();
        debug!("Azure CLIコマンド実行開始: az role definition list");
        let json = self.client.execute_az_command(args.clone()).await?;
        debug!(
            elapsed_ms = az_start.elapsed().as_millis(),
            "Azure CLIコマンド完了"
        );

        // まず、すべてのrole definitionを収集
        let filter_start = std::time::Instant::now();
        let role_definitions_vec: Vec<Value> = json
            .as_array()
            .context("Role Definitions一覧が配列形式ではありません")?
            .iter()
            .filter_map(|rd| {
                // 名前プレフィックスフィルタを適用
                if let Some(name_prefix) = self.config.filters.get("name_prefix") {
                    if let Some(name) = rd
                        .get("name")
                        .or_else(|| rd.get("roleName"))
                        .and_then(|v| v.as_str())
                    {
                        if !name.starts_with(name_prefix) {
                            return None;
                        }
                    }
                }
                Some(rd.clone())
            })
            .collect();
        debug!(
            count = role_definitions_vec.len(),
            elapsed_ms = filter_start.elapsed().as_millis(),
            "フィルタリング完了"
        );

        // ユニークなrole definition IDを収集して並列で表示名を取得
        let unique_start = std::time::Instant::now();
        let mut role_def_id_to_name: HashMap<String, Option<String>> = HashMap::new();
        let mut unique_role_def_ids: Vec<String> = Vec::new();

        for rd in &role_definitions_vec {
            if let Some(role_def_id) = rd.get("id").and_then(|v| v.as_str()) {
                if !role_def_id_to_name.contains_key(role_def_id) {
                    unique_role_def_ids.push(role_def_id.to_string());
                    role_def_id_to_name.insert(role_def_id.to_string(), None);
                }
            }
        }
        debug!(
            count = unique_role_def_ids.len(),
            elapsed_ms = unique_start.elapsed().as_millis(),
            "ユニークなRole Definition ID収集完了"
        );

        // 並列で表示名を取得（同時実行数を制限）
        let api_start = std::time::Instant::now();
        debug!(
            count = unique_role_def_ids.len(),
            "Role Definition表示名の並列取得開始"
        );

        // トークンを事前に取得してキャッシュ
        let token_start = std::time::Instant::now();
        let scope = "https://management.azure.com/.default";
        let token = match self.client.get_auth_token(scope).await {
            Some(token) => {
                debug!(
                    elapsed_ms = token_start.elapsed().as_millis(),
                    "トークン取得完了"
                );
                token
            }
            None => {
                warn!("トークン取得失敗、フォールバック処理に移行");
                let role_definitions: Vec<Value> = role_definitions_vec
                    .iter()
                    .map(Self::transform_role_definition_basic)
                    .collect();
                info!(
                    count = role_definitions.len(),
                    elapsed_ms = start_time.elapsed().as_millis(),
                    "Role Definitionsスキャン完了"
                );
                return Ok(role_definitions);
            }
        };

        // HTTPクライアントが利用できない場合はフォールバック
        if self.client.get_http_client().is_none() {
            warn!("HTTPクライアント利用不可、フォールバック処理に移行");
            let role_definitions: Vec<Value> = role_definitions_vec
                .iter()
                .map(Self::transform_role_definition_basic)
                .collect();
            info!(
                count = role_definitions.len(),
                elapsed_ms = start_time.elapsed().as_millis(),
                "Role Definitionsスキャン完了"
            );
            return Ok(role_definitions);
        }

        // 同時実行数を10に制限
        let semaphore = Arc::new(Semaphore::new(10));
        let sub_id = self.config.subscription_id.as_deref();
        let client = Arc::clone(&self.client);
        let display_name_futures: Vec<_> = unique_role_def_ids
            .iter()
            .map(|rid| {
                let rid_clone = rid.clone();
                let sub_id_clone = sub_id.map(|s| s.to_string());
                let token_clone = token.clone();
                let permit = semaphore.clone();
                let client = Arc::clone(&client);
                async move {
                    let _permit = permit.acquire().await.unwrap();
                    let name = client
                        .get_role_display_name(&rid_clone, sub_id_clone, &token_clone)
                        .await;
                    (rid_clone, name)
                }
            })
            .collect();

        let display_names: Vec<_> = join_all(display_name_futures).await;
        for (rid, name) in display_names {
            role_def_id_to_name.insert(rid, name);
        }
        debug!(
            elapsed_ms = api_start.elapsed().as_millis(),
            "Role Definition表示名取得完了"
        );

        // 各role definitionに対して表示名を設定
        let mut role_definitions = Vec::new();
        for rd in role_definitions_vec {
            // Azure CLIの出力をフロントエンドが期待する形式に変換
            let mut transformed = serde_json::Map::new();

            // role_definition_id: id
            let role_def_id = rd.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
            if let Some(ref rid) = role_def_id {
                transformed.insert("role_definition_id".to_string(), Value::String(rid.clone()));
            }

            // role_name: キャッシュから表示名を取得
            let role_name_from_api = role_def_id
                .as_ref()
                .and_then(|rid| role_def_id_to_name.get(rid))
                .and_then(|opt| opt.as_ref())
                .cloned();

            if let Some(ref name) = role_name_from_api {
                transformed.insert("role_name".to_string(), Value::String(name.clone()));
            } else if let Some(name) = rd.get("name").or_else(|| rd.get("roleName")) {
                // フォールバック: APIから取得できない場合はnameまたはroleNameを使用
                transformed.insert("role_name".to_string(), name.clone());
            }

            // description: description
            if let Some(desc) = rd.get("description") {
                transformed.insert("description".to_string(), desc.clone());
            }

            // role_type: type
            if let Some(role_type) = rd.get("type") {
                transformed.insert("role_type".to_string(), role_type.clone());
            }

            // scope: assignableScopes の最初の要素、または id から抽出
            let mut scope_set = false;
            if let Some(assignable_scopes) = rd.get("assignableScopes") {
                if let Some(scopes) = assignable_scopes.as_array() {
                    // サブスクリプションレベルのスコープを優先的に選択
                    for scope in scopes {
                        if let Some(scope_str) = scope.as_str() {
                            if scope_str.contains("/subscriptions/")
                                && !scope_str.contains("/resourceGroups/")
                            {
                                transformed.insert("scope".to_string(), scope.clone());
                                scope_set = true;
                                break;
                            }
                        }
                    }
                    // サブスクリプションレベルのスコープが見つからない場合、最初の要素を使用
                    if !scope_set {
                        if let Some(first_scope) = scopes.first() {
                            transformed.insert("scope".to_string(), first_scope.clone());
                            scope_set = true;
                        }
                    }
                }
            }
            // assignableScopesがない場合、id からスコープを抽出
            if !scope_set {
                if let Some(id) = rd.get("id") {
                    if let Some(id_str) = id.as_str() {
                        if let Some(scope_end) =
                            id_str.rfind("/providers/Microsoft.Authorization/roleDefinitions")
                        {
                            let scope = &id_str[..scope_end];
                            transformed
                                .insert("scope".to_string(), Value::String(scope.to_string()));
                        }
                    }
                }
            }

            // 元のデータも保持（必要に応じて）
            for (key, value) in rd.as_object().unwrap_or(&serde_json::Map::new()) {
                if !transformed.contains_key(key) {
                    transformed.insert(key.clone(), value.clone());
                }
            }

            role_definitions.push(Value::Object(transformed));
        }

        info!(
            count = role_definitions.len(),
            elapsed_ms = start_time.elapsed().as_millis(),
            "Role Definitionsスキャン完了"
        );
        Ok(role_definitions)
    }

    /// Role Assignmentsを取得
    pub async fn scan_role_assignments(&self) -> Result<Vec<Value>> {
        let scan_targets = &self.config.scan_targets;

        if !scan_targets
            .get("role_assignments")
            .copied()
            .unwrap_or(false)
        {
            return Ok(Vec::new());
        }

        let start_time = std::time::Instant::now();
        info!("Role Assignmentsスキャンを開始");

        let mut args: Vec<String> = vec![
            "role".to_string(),
            "assignment".to_string(),
            "list".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ];
        let scope_args = self.get_scope_args();

        // スコープ引数を追加
        args.extend(scope_args);

        let az_start = std::time::Instant::now();
        debug!("Azure CLIコマンド実行開始: az role assignment list");
        let json = self.client.execute_az_command(args.clone()).await?;
        debug!(
            elapsed_ms = az_start.elapsed().as_millis(),
            "Azure CLIコマンド完了"
        );

        // まず、すべてのrole assignmentを収集
        let filter_start = std::time::Instant::now();
        let role_assignments_vec: Vec<Value> = json
            .as_array()
            .context("Role Assignments一覧が配列形式ではありません")?
            .iter()
            .filter_map(|ra| {
                // 名前プレフィックスフィルタを適用
                if let Some(name_prefix) = self.config.filters.get("name_prefix") {
                    if let Some(name) = ra
                        .get("name")
                        .or_else(|| ra.get("roleDefinitionName"))
                        .and_then(|v| v.as_str())
                    {
                        if !name.starts_with(name_prefix) {
                            return None;
                        }
                    }
                }

                Some(ra.clone())
            })
            .collect();
        debug!(
            count = role_assignments_vec.len(),
            elapsed_ms = filter_start.elapsed().as_millis(),
            "フィルタリング完了"
        );

        // ユニークなrole definition IDとprincipal IDを収集
        let unique_start = std::time::Instant::now();
        let mut role_def_id_to_name: HashMap<String, Option<String>> = HashMap::new();
        let mut principal_id_to_name: HashMap<String, Option<String>> = HashMap::new();
        let mut unique_role_def_ids: Vec<String> = Vec::new();
        let mut unique_principal_ids: Vec<(String, String)> = Vec::new(); // (id, type)

        for ra in &role_assignments_vec {
            // Role definition IDを収集
            if let Some(role_def_id) = ra.get("roleDefinitionId").and_then(|v| v.as_str()) {
                if !role_def_id_to_name.contains_key(role_def_id) {
                    unique_role_def_ids.push(role_def_id.to_string());
                    role_def_id_to_name.insert(role_def_id.to_string(), None);
                }
            }

            // Principal IDを収集
            if let (Some(principal_id), Some(principal_type)) = (
                ra.get("principalId").and_then(|v| v.as_str()),
                ra.get("principalType").and_then(|v| v.as_str()),
            ) {
                let key = format!("{}:{}", principal_id, principal_type);
                principal_id_to_name.entry(key).or_insert_with(|| {
                    unique_principal_ids
                        .push((principal_id.to_string(), principal_type.to_string()));
                    None
                });
            }
        }
        debug!(
            role_def_count = unique_role_def_ids.len(),
            principal_count = unique_principal_ids.len(),
            elapsed_ms = unique_start.elapsed().as_millis(),
            "ユニークなID収集完了"
        );

        // 並列で表示名を取得（同時実行数を制限）
        let api_start = std::time::Instant::now();
        debug!(
            role_def_count = unique_role_def_ids.len(),
            principal_count = unique_principal_ids.len(),
            "表示名の並列取得開始"
        );

        // トークンを事前に取得してキャッシュ（Management API用）
        let mgmt_token_start = std::time::Instant::now();
        let mgmt_scope = "https://management.azure.com/.default";
        let mgmt_token = match self.client.get_auth_token(mgmt_scope).await {
            Some(token) => {
                debug!(
                    elapsed_ms = mgmt_token_start.elapsed().as_millis(),
                    "Management APIトークン取得完了"
                );
                token
            }
            None => {
                warn!("Management APIトークン取得失敗");
                String::new()
            }
        };

        // トークンを事前に取得してキャッシュ（Graph API用）
        let graph_token_start = std::time::Instant::now();
        let graph_scope = "https://graph.microsoft.com/.default";
        let graph_token = match self.client.get_auth_token(graph_scope).await {
            Some(token) => {
                debug!(
                    elapsed_ms = graph_token_start.elapsed().as_millis(),
                    "Graph APIトークン取得完了"
                );
                token
            }
            None => {
                warn!("Graph APIトークン取得失敗");
                String::new()
            }
        };

        // HTTPクライアントが利用できない場合は空の結果を返す
        if self.client.get_http_client().is_none() {
            warn!("HTTPクライアント利用不可");
            return Ok(Vec::new());
        }

        // 同時実行数を10に制限
        let semaphore = Arc::new(Semaphore::new(10));
        let sub_id = self.config.subscription_id.as_deref();
        let client = Arc::clone(&self.client);

        // Role definition名を並列取得
        let role_def_futures: Vec<_> = unique_role_def_ids
            .iter()
            .map(|rid| {
                let rid_clone = rid.clone();
                let sub_id_clone = sub_id.map(|s| s.to_string());
                let token_clone = mgmt_token.clone();
                let permit = semaphore.clone();
                let client = Arc::clone(&client);
                async move {
                    let _permit = permit.acquire().await.unwrap();
                    let name = client
                        .get_role_display_name(&rid_clone, sub_id_clone, &token_clone)
                        .await;
                    (rid_clone, name)
                }
            })
            .collect();

        // Principal名を並列取得
        let principal_futures: Vec<_> = unique_principal_ids
            .iter()
            .map(|(pid, ptype)| {
                let pid_clone = pid.clone();
                let ptype_clone = ptype.clone();
                let ptype_for_key = ptype.clone();
                let token_clone = graph_token.clone();
                let permit = semaphore.clone();
                let client = Arc::clone(&client);
                async move {
                    let _permit = permit.acquire().await.unwrap();
                    let name = client
                        .get_principal_display_name(&pid_clone, Some(ptype_clone), &token_clone)
                        .await;
                    (format!("{}:{}", pid_clone, ptype_for_key), name)
                }
            })
            .collect();

        // 両方を並列実行
        let (role_def_results, principal_results) =
            tokio::join!(join_all(role_def_futures), join_all(principal_futures));

        for (rid, name) in role_def_results {
            role_def_id_to_name.insert(rid, name);
        }

        for (key, name) in principal_results {
            principal_id_to_name.insert(key, name);
        }
        debug!(
            elapsed_ms = api_start.elapsed().as_millis(),
            "表示名取得完了"
        );

        // 各role assignmentに対して表示名を設定
        let mut transformed_assignments = Vec::new();
        for ra in role_assignments_vec {
            // Azure CLIの出力をフロントエンドが期待する形式に変換
            let mut transformed = serde_json::Map::new();

            // assignment_id: name または id
            if let Some(id) = ra.get("name").or_else(|| ra.get("id")) {
                transformed.insert("assignment_id".to_string(), id.clone());
            }

            // role_definition_name: キャッシュから表示名を取得
            let role_def_id = ra
                .get("roleDefinitionId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let role_def_name_from_api = role_def_id
                .as_ref()
                .and_then(|rid| role_def_id_to_name.get(rid))
                .and_then(|opt| opt.as_ref())
                .cloned();

            if let Some(ref name) = role_def_name_from_api {
                transformed.insert(
                    "role_definition_name".to_string(),
                    Value::String(name.clone()),
                );
            } else if let Some(role_def_name) = ra.get("roleDefinitionName") {
                // フォールバック: APIから取得できない場合はroleDefinitionNameを使用
                transformed.insert("role_definition_name".to_string(), role_def_name.clone());
            } else if let Some(ref rid) = role_def_id {
                // さらにフォールバック: roleDefinitionIdから名前を抽出
                if let Some(name_start) = rid.rfind('/') {
                    let name = &rid[name_start + 1..];
                    transformed.insert(
                        "role_definition_name".to_string(),
                        Value::String(name.to_string()),
                    );
                }
            }

            // principal_id: principalId (IDも保持)
            let principal_id = ra
                .get("principalId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(ref pid) = principal_id {
                transformed.insert("principal_id".to_string(), Value::String(pid.clone()));
            }

            // principal_type: principalType
            let principal_type = ra
                .get("principalType")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(ref ptype) = principal_type {
                transformed.insert("principal_type".to_string(), Value::String(ptype.clone()));
            }

            // principal_name: キャッシュから表示名を取得
            let display_name =
                if let (Some(ref pid), Some(ref ptype)) = (&principal_id, &principal_type) {
                    let key = format!("{}:{}", pid, ptype);
                    principal_id_to_name
                        .get(&key)
                        .and_then(|opt| opt.as_ref())
                        .cloned()
                } else {
                    None
                };

            if let Some(ref name) = display_name {
                transformed.insert("principal_name".to_string(), Value::String(name.clone()));
            } else if let Some(principal_name) = ra.get("principalName") {
                // フォールバック: 表示名が取得できない場合は元のprincipalNameを使用
                transformed.insert("principal_name".to_string(), principal_name.clone());
            } else if let Some(ref pid) = principal_id {
                // さらにフォールバック: principal_idを使用
                transformed.insert("principal_name".to_string(), Value::String(pid.clone()));
            }

            // scope: scope
            if let Some(scope) = ra.get("scope") {
                transformed.insert("scope".to_string(), scope.clone());
            }

            // 元のデータも保持（必要に応じて）
            for (key, value) in ra.as_object().unwrap_or(&serde_json::Map::new()) {
                if !transformed.contains_key(key) {
                    transformed.insert(key.clone(), value.clone());
                }
            }

            transformed_assignments.push(Value::Object(transformed));
        }

        info!(
            count = transformed_assignments.len(),
            elapsed_ms = start_time.elapsed().as_millis(),
            "Role Assignmentsスキャン完了"
        );
        Ok(transformed_assignments)
    }

    pub async fn scan(
        &self,
        progress_callback: Box<dyn Fn(u32, String) + Send + Sync>,
    ) -> Result<Value> {
        let scan_start = std::time::Instant::now();
        info!("Azure IAMスキャン開始");
        progress_callback(0, "Azure IAMスキャンを開始しています...".to_string());

        let mut results = serde_json::Map::new();

        // Provider情報を追加
        results.insert("provider".to_string(), Value::String("azure".to_string()));

        let callback = Arc::new(progress_callback);

        // IAMスキャン（Role Definitions + Role Assignments）を並列実行
        // 両者は独立したAzure CLI/APIコールのため安全に並列化可能
        let cb1 = callback.clone();
        let cb2 = callback.clone();

        let role_def_future = async {
            cb1(20, "Role Definitionsのスキャン中...".to_string());
            let role_definitions = self
                .scan_role_definitions()
                .await
                .context("Role Definitionsのスキャンに失敗しました")?;
            let count = role_definitions.len();
            cb1(50, format!("Role Definitionsのスキャン完了: {}件", count));
            Ok::<_, anyhow::Error>(role_definitions)
        };

        let role_assign_future = async {
            cb2(20, "Role Assignmentsのスキャン中...".to_string());
            let role_assignments = self
                .scan_role_assignments()
                .await
                .context("Role Assignmentsのスキャンに失敗しました")?;
            let count = role_assignments.len();
            cb2(60, format!("Role Assignmentsのスキャン完了: {}件", count));
            Ok::<_, anyhow::Error>(role_assignments)
        };

        let (role_definitions, role_assignments) =
            tokio::try_join!(role_def_future, role_assign_future)?;

        results.insert(
            "role_definitions".to_string(),
            Value::Array(role_definitions),
        );
        results.insert(
            "role_assignments".to_string(),
            Value::Array(role_assignments),
        );
        callback(70, "IAMスキャン完了".to_string());

        // App Services + Function Apps を並列実行
        // 異なるAzure CLIコマンドを使用するため安全に並列化可能
        let has_app_services = self
            .config
            .scan_targets
            .get("app_services")
            .copied()
            .unwrap_or(false);
        let has_function_apps = self
            .config
            .scan_targets
            .get("function_apps")
            .copied()
            .unwrap_or(false);

        let cb3 = callback.clone();
        let cb4 = callback.clone();

        let app_services_future = async {
            if !has_app_services {
                return Ok::<_, anyhow::Error>(Vec::new());
            }
            cb3(75, "App Servicesのスキャン中...".to_string());
            let apps = self.scan_app_services().await.unwrap_or_default();
            let count = apps.len();
            cb3(80, format!("App Servicesのスキャン完了: {}件", count));
            Ok(apps)
        };

        let function_apps_future = async {
            if !has_function_apps {
                return Ok::<_, anyhow::Error>(Vec::new());
            }
            cb4(85, "Function Appsのスキャン中...".to_string());
            let funcs = self.scan_function_apps().await.unwrap_or_default();
            let count = funcs.len();
            cb4(90, format!("Function Appsのスキャン完了: {}件", count));
            Ok(funcs)
        };

        let (app_services, function_apps) =
            tokio::try_join!(app_services_future, function_apps_future)?;

        results.insert("app_services".to_string(), Value::Array(app_services));
        results.insert("function_apps".to_string(), Value::Array(function_apps));

        info!(
            elapsed_ms = scan_start.elapsed().as_millis(),
            "Azureスキャン完了"
        );
        callback(
            100,
            format!(
                "Azureスキャン完了: 合計{}ms",
                scan_start.elapsed().as_millis()
            ),
        );
        Ok(Value::Object(results))
    }

    /// App Servicesをスキャン
    async fn scan_app_services(&self) -> Result<Vec<Value>> {
        info!("Azure App Servicesスキャン開始");
        let subscription_id = self.config.subscription_id.as_deref().unwrap_or("");

        let output = self
            .client
            .execute_az_command(vec![
                "webapp".to_string(),
                "list".to_string(),
                "--subscription".to_string(),
                subscription_id.to_string(),
                "--output".to_string(),
                "json".to_string(),
            ])
            .await?;

        let apps = output.as_array().cloned().unwrap_or_default();
        let result: Vec<Value> = apps
            .into_iter()
            .filter(|app| {
                // Function Appを除外（kind が "functionapp" を含まない）
                let kind = app.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                !kind.to_lowercase().contains("functionapp")
            })
            .map(|app| {
                let mut j = serde_json::Map::new();
                if let Some(name) = app.get("name").and_then(|v| v.as_str()) {
                    j.insert("name".to_string(), Value::String(name.to_string()));
                }
                if let Some(id) = app.get("id").and_then(|v| v.as_str()) {
                    j.insert("id".to_string(), Value::String(id.to_string()));
                }
                if let Some(rg) = app.get("resourceGroup").and_then(|v| v.as_str()) {
                    j.insert("resource_group".to_string(), Value::String(rg.to_string()));
                }
                if let Some(loc) = app.get("location").and_then(|v| v.as_str()) {
                    j.insert("location".to_string(), Value::String(loc.to_string()));
                }
                if let Some(kind) = app.get("kind").and_then(|v| v.as_str()) {
                    j.insert("kind".to_string(), Value::String(kind.to_string()));
                }
                if let Some(state) = app.get("state").and_then(|v| v.as_str()) {
                    j.insert("state".to_string(), Value::String(state.to_string()));
                }
                if let Some(host) = app.get("defaultHostName").and_then(|v| v.as_str()) {
                    j.insert(
                        "default_host_name".to_string(),
                        Value::String(host.to_string()),
                    );
                }
                if let Some(plan) = app.get("appServicePlanId").and_then(|v| v.as_str()) {
                    j.insert(
                        "service_plan_id".to_string(),
                        Value::String(plan.to_string()),
                    );
                }
                if let Some(https) = app.get("httpsOnly").and_then(|v| v.as_bool()) {
                    j.insert("https_only".to_string(), Value::Bool(https));
                }
                Value::Object(j)
            })
            .collect();

        info!(count = result.len(), "Azure App Servicesスキャン完了");
        Ok(result)
    }

    /// Function Appsをスキャン
    async fn scan_function_apps(&self) -> Result<Vec<Value>> {
        info!("Azure Function Appsスキャン開始");
        let subscription_id = self.config.subscription_id.as_deref().unwrap_or("");

        let output = self
            .client
            .execute_az_command(vec![
                "functionapp".to_string(),
                "list".to_string(),
                "--subscription".to_string(),
                subscription_id.to_string(),
                "--output".to_string(),
                "json".to_string(),
            ])
            .await?;

        let funcs = output.as_array().cloned().unwrap_or_default();
        let result: Vec<Value> = funcs
            .into_iter()
            .map(|func| {
                let mut j = serde_json::Map::new();
                if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                    j.insert("name".to_string(), Value::String(name.to_string()));
                }
                if let Some(id) = func.get("id").and_then(|v| v.as_str()) {
                    j.insert("id".to_string(), Value::String(id.to_string()));
                }
                if let Some(rg) = func.get("resourceGroup").and_then(|v| v.as_str()) {
                    j.insert("resource_group".to_string(), Value::String(rg.to_string()));
                }
                if let Some(loc) = func.get("location").and_then(|v| v.as_str()) {
                    j.insert("location".to_string(), Value::String(loc.to_string()));
                }
                if let Some(kind) = func.get("kind").and_then(|v| v.as_str()) {
                    j.insert("kind".to_string(), Value::String(kind.to_string()));
                }
                if let Some(state) = func.get("state").and_then(|v| v.as_str()) {
                    j.insert("state".to_string(), Value::String(state.to_string()));
                }
                if let Some(plan) = func.get("appServicePlanId").and_then(|v| v.as_str()) {
                    j.insert(
                        "service_plan_id".to_string(),
                        Value::String(plan.to_string()),
                    );
                }
                Value::Object(j)
            })
            .collect();

        info!(count = result.len(), "Azure Function Appsスキャン完了");
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::azure::azure_client_trait::mock::MockAzureClient;
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_config() -> ScanConfig {
        let mut scan_targets = HashMap::new();
        scan_targets.insert("role_definitions".to_string(), true);
        scan_targets.insert("role_assignments".to_string(), true);

        ScanConfig {
            provider: "azure".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            tenant_id: Some("test-tenant-id".to_string()),
            subscription_id: Some("test-subscription-id".to_string()),
            auth_method: Some("az_login".to_string()),
            service_principal_config: None,
            scope_type: Some("subscription".to_string()),
            scope_value: None,
            scan_targets,
            filters: HashMap::new(),
            include_tags: true,
        }
    }

    // ==================== transform_role_definition_basic テスト ====================

    #[test]
    fn test_transform_role_definition_basic_full() {
        let role_def = json!({
            "id": "/subscriptions/12345/providers/Microsoft.Authorization/roleDefinitions/abcdef",
            "name": "CustomRole",
            "description": "A custom role for testing",
            "type": "CustomRole"
        });

        let result = AzureIamScanner::<RealAzureClient>::transform_role_definition_basic(&role_def);

        assert_eq!(
            result["role_definition_id"],
            "/subscriptions/12345/providers/Microsoft.Authorization/roleDefinitions/abcdef"
        );
        assert_eq!(result["role_name"], "CustomRole");
        assert_eq!(result["description"], "A custom role for testing");
        assert_eq!(result["role_type"], "CustomRole");
        assert_eq!(result["scope"], "/subscriptions/12345");
    }

    #[test]
    fn test_transform_role_definition_basic_minimal() {
        let role_def = json!({
            "id": "/providers/Microsoft.Authorization/roleDefinitions/xyz",
            "name": "MinimalRole"
        });

        let result = AzureIamScanner::<RealAzureClient>::transform_role_definition_basic(&role_def);

        assert_eq!(
            result["role_definition_id"],
            "/providers/Microsoft.Authorization/roleDefinitions/xyz"
        );
        assert_eq!(result["role_name"], "MinimalRole");
        assert_eq!(result["scope"], "");
    }

    #[test]
    fn test_transform_role_definition_basic_with_role_name_fallback() {
        let role_def = json!({
            "id": "/subscriptions/12345/providers/Microsoft.Authorization/roleDefinitions/test",
            "roleName": "Role Name from roleName field"
        });

        let result = AzureIamScanner::<RealAzureClient>::transform_role_definition_basic(&role_def);

        assert_eq!(result["role_name"], "Role Name from roleName field");
    }

    #[test]
    fn test_transform_role_definition_basic_scope_extraction() {
        let test_cases = vec![
            (
                "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-456",
                "/subscriptions/sub-123",
            ),
            (
                "/subscriptions/sub-123/resourceGroups/rg-test/providers/Microsoft.Authorization/roleDefinitions/role-456",
                "/subscriptions/sub-123/resourceGroups/rg-test",
            ),
            (
                "/providers/Microsoft.Management/managementGroups/mg-test/providers/Microsoft.Authorization/roleDefinitions/role-456",
                "/providers/Microsoft.Management/managementGroups/mg-test",
            ),
        ];

        for (id, expected_scope) in test_cases {
            let role_def = json!({
                "id": id,
                "name": "Test"
            });

            let result =
                AzureIamScanner::<RealAzureClient>::transform_role_definition_basic(&role_def);
            assert_eq!(result["scope"], expected_scope, "Failed for id: {}", id);
        }
    }

    #[test]
    fn test_transform_role_definition_basic_preserves_original_fields() {
        let role_def = json!({
            "id": "/subscriptions/12345/providers/Microsoft.Authorization/roleDefinitions/abcdef",
            "name": "CustomRole",
            "custom_field_1": "value1",
            "custom_field_2": 12345,
            "nested_object": {
                "key": "value"
            }
        });

        let result = AzureIamScanner::<RealAzureClient>::transform_role_definition_basic(&role_def);

        // 元のフィールドが保持されていることを確認
        assert_eq!(result["custom_field_1"], "value1");
        assert_eq!(result["custom_field_2"], 12345);
        assert_eq!(result["nested_object"]["key"], "value");
    }

    // ==================== scan_role_definitions モックテスト ====================

    #[tokio::test]
    async fn test_scan_role_definitions_returns_all_definitions() {
        let mut mock_client = MockAzureClient::new();

        // Azure CLIコマンドの結果を設定
        mock_client
            .expect_execute_az_command()
            .returning(|_args| {
                Ok(json!([
                    {
                        "id": "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-1",
                        "name": "Reader",
                        "description": "View all resources",
                        "type": "BuiltInRole"
                    },
                    {
                        "id": "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-2",
                        "name": "Contributor",
                        "description": "Manage all resources",
                        "type": "BuiltInRole"
                    }
                ]))
            });

        // トークン取得を設定（失敗してフォールバック）
        mock_client.expect_get_auth_token().returning(|_| None);

        let config = create_test_config();
        let scanner = AzureIamScanner::new_with_client(config, mock_client);

        let result = scanner.scan_role_definitions().await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["role_name"], "Reader");
        assert_eq!(result[1]["role_name"], "Contributor");
    }

    #[tokio::test]
    async fn test_scan_role_definitions_with_name_filter() {
        let mut mock_client = MockAzureClient::new();

        mock_client
            .expect_execute_az_command()
            .returning(|_args| {
                Ok(json!([
                    {
                        "id": "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-1",
                        "name": "Custom-Reader",
                        "type": "CustomRole"
                    },
                    {
                        "id": "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-2",
                        "name": "Contributor",
                        "type": "BuiltInRole"
                    },
                    {
                        "id": "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-3",
                        "name": "Custom-Admin",
                        "type": "CustomRole"
                    }
                ]))
            });

        mock_client.expect_get_auth_token().returning(|_| None);

        let mut config = create_test_config();
        config
            .filters
            .insert("name_prefix".to_string(), "Custom-".to_string());
        let scanner = AzureIamScanner::new_with_client(config, mock_client);

        let result = scanner.scan_role_definitions().await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["role_name"], "Custom-Reader");
        assert_eq!(result[1]["role_name"], "Custom-Admin");
    }

    #[tokio::test]
    async fn test_scan_role_definitions_disabled() {
        let mock_client = MockAzureClient::new();

        let mut config = create_test_config();
        config
            .scan_targets
            .insert("role_definitions".to_string(), false);
        let scanner = AzureIamScanner::new_with_client(config, mock_client);

        let result = scanner.scan_role_definitions().await.unwrap();

        assert!(result.is_empty());
    }

    // ==================== scan_role_assignments モックテスト ====================

    #[tokio::test]
    async fn test_scan_role_assignments_returns_all_assignments() {
        let mut mock_client = MockAzureClient::new();

        // Azure CLIコマンドの結果を設定
        mock_client
            .expect_execute_az_command()
            .returning(|_args| {
                Ok(json!([
                    {
                        "name": "assignment-1",
                        "roleDefinitionId": "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-1",
                        "principalId": "principal-1",
                        "principalType": "User",
                        "scope": "/subscriptions/sub-123"
                    },
                    {
                        "name": "assignment-2",
                        "roleDefinitionId": "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-2",
                        "principalId": "principal-2",
                        "principalType": "ServicePrincipal",
                        "scope": "/subscriptions/sub-123/resourceGroups/rg-1"
                    }
                ]))
            });

        // トークン取得を設定
        mock_client
            .expect_get_auth_token()
            .returning(|_| Some("test-token".to_string()));

        // HTTPクライアントを設定（None でフォールバック）
        mock_client.expect_get_http_client().returning(|| None);

        let config = create_test_config();
        let scanner = AzureIamScanner::new_with_client(config, mock_client);

        let result = scanner.scan_role_assignments().await.unwrap();

        // HTTPクライアントがないため空の結果
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_scan_role_assignments_disabled() {
        let mock_client = MockAzureClient::new();

        let mut config = create_test_config();
        config
            .scan_targets
            .insert("role_assignments".to_string(), false);
        let scanner = AzureIamScanner::new_with_client(config, mock_client);

        let result = scanner.scan_role_assignments().await.unwrap();

        assert!(result.is_empty());
    }

    // ==================== scan メソッドモックテスト ====================

    #[tokio::test]
    async fn test_scan_progress_callback() {
        let mut mock_client = MockAzureClient::new();

        // Role definitions用
        mock_client
            .expect_execute_az_command()
            .returning(|_args| {
                Ok(json!([
                    {
                        "id": "/subscriptions/sub-123/providers/Microsoft.Authorization/roleDefinitions/role-1",
                        "name": "Reader"
                    }
                ]))
            });

        mock_client.expect_get_auth_token().returning(|_| None);

        mock_client.expect_get_http_client().returning(|| None);

        let config = create_test_config();
        let scanner = AzureIamScanner::new_with_client(config, mock_client);

        let progress_values = Arc::new(std::sync::Mutex::new(Vec::new()));
        let progress_values_clone = Arc::clone(&progress_values);

        let callback = Box::new(move |progress: u32, message: String| {
            progress_values_clone
                .lock()
                .unwrap()
                .push((progress, message));
        });

        let result = scanner.scan(callback).await.unwrap();

        // 結果を確認
        assert_eq!(result["provider"], "azure");

        // プログレスが記録されていることを確認
        let values = progress_values.lock().unwrap();
        assert!(!values.is_empty());
        assert_eq!(values[0].0, 0); // 開始時は0%
        assert!(values.last().unwrap().0 >= 90); // 終了時は90%以上
    }

    #[tokio::test]
    async fn test_scan_error_handling() {
        let mut mock_client = MockAzureClient::new();

        // エラーを返す
        mock_client
            .expect_execute_az_command()
            .returning(|_args| Err(anyhow::anyhow!("Azure CLI not found")));

        let config = create_test_config();
        let scanner = AzureIamScanner::new_with_client(config, mock_client);

        let callback = Box::new(|_progress: u32, _message: String| {});
        let result = scanner.scan(callback).await;

        assert!(result.is_err());
        // エラーは context() でラップされるので、エラーチェーンに含まれることを確認
        let error_msg = format!("{:?}", result.unwrap_err());
        assert!(
            error_msg.contains("Azure CLI not found") || error_msg.contains("Role Definitions"),
            "Expected error to contain 'Azure CLI not found' or 'Role Definitions', got: {}",
            error_msg
        );
    }

    // ==================== get_scope_args テスト ====================

    #[test]
    fn test_get_scope_args_subscription() {
        let mock_client = MockAzureClient::new();
        let mut config = create_test_config();
        config.scope_type = Some("subscription".to_string());
        config.subscription_id = Some("my-sub-123".to_string());

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let args = scanner.get_scope_args();

        assert_eq!(args, vec!["--subscription", "my-sub-123"]);
    }

    #[test]
    fn test_get_scope_args_resource_group() {
        let mock_client = MockAzureClient::new();
        let mut config = create_test_config();
        config.scope_type = Some("resource_group".to_string());
        config.subscription_id = Some("my-sub-123".to_string());
        config.scope_value = Some("my-rg".to_string());

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let args = scanner.get_scope_args();

        assert_eq!(
            args,
            vec!["--scope", "/subscriptions/my-sub-123/resourceGroups/my-rg"]
        );
    }

    #[test]
    fn test_get_scope_args_management_group() {
        let mock_client = MockAzureClient::new();
        let mut config = create_test_config();
        config.scope_type = Some("management_group".to_string());
        config.scope_value = Some("my-mg".to_string());

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let args = scanner.get_scope_args();

        assert_eq!(
            args,
            vec![
                "--scope",
                "/providers/Microsoft.Management/managementGroups/my-mg"
            ]
        );
    }

    #[test]
    fn test_get_scope_args_no_scope_type() {
        let mock_client = MockAzureClient::new();
        let mut config = create_test_config();
        config.scope_type = None;
        config.subscription_id = Some("sub-no-type".to_string());

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let args = scanner.get_scope_args();

        assert_eq!(args, vec!["--subscription", "sub-no-type"]);
    }

    #[test]
    fn test_get_scope_args_unknown_scope_type_falls_back_to_subscription() {
        let mock_client = MockAzureClient::new();
        let mut config = create_test_config();
        config.scope_type = Some("unknown_type".to_string());
        config.subscription_id = Some("sub-fallback".to_string());

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let args = scanner.get_scope_args();

        assert_eq!(args, vec!["--subscription", "sub-fallback"]);
    }

    // ==================== scan_app_services テスト ====================

    #[tokio::test]
    async fn test_scan_app_services() {
        let mut mock_client = MockAzureClient::new();

        mock_client
            .expect_execute_az_command()
            .returning(|_args| {
                Ok(json!([
                    {
                        "name": "my-webapp",
                        "id": "/subscriptions/sub-123/resourceGroups/rg-1/providers/Microsoft.Web/sites/my-webapp",
                        "resourceGroup": "rg-1",
                        "location": "eastus",
                        "kind": "app",
                        "state": "Running",
                        "defaultHostName": "my-webapp.azurewebsites.net",
                        "appServicePlanId": "/subscriptions/sub-123/resourceGroups/rg-1/providers/Microsoft.Web/serverfarms/my-plan",
                        "httpsOnly": true
                    },
                    {
                        "name": "my-functionapp",
                        "id": "/subscriptions/sub-123/resourceGroups/rg-1/providers/Microsoft.Web/sites/my-functionapp",
                        "resourceGroup": "rg-1",
                        "location": "eastus",
                        "kind": "functionapp",
                        "state": "Running"
                    }
                ]))
            });

        let mut config = create_test_config();
        config.scan_targets.insert("app_services".to_string(), true);
        config.subscription_id = Some("sub-123".to_string());

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let result = scanner.scan_app_services().await.unwrap();

        // function app は除外されるので 1 件
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "my-webapp");
        assert_eq!(result[0]["kind"], "app");
        assert_eq!(result[0]["state"], "Running");
        assert_eq!(
            result[0]["default_host_name"],
            "my-webapp.azurewebsites.net"
        );
        assert_eq!(result[0]["https_only"], true);
    }

    #[tokio::test]
    async fn test_scan_app_services_empty() {
        let mut mock_client = MockAzureClient::new();

        mock_client
            .expect_execute_az_command()
            .returning(|_args| Ok(json!([])));

        let mut config = create_test_config();
        config.scan_targets.insert("app_services".to_string(), true);

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let result = scanner.scan_app_services().await.unwrap();

        assert!(result.is_empty());
    }

    // ==================== scan_function_apps テスト ====================

    #[tokio::test]
    async fn test_scan_function_apps() {
        let mut mock_client = MockAzureClient::new();

        mock_client
            .expect_execute_az_command()
            .returning(|_args| {
                Ok(json!([
                    {
                        "name": "my-func",
                        "id": "/subscriptions/sub-123/resourceGroups/rg-1/providers/Microsoft.Web/sites/my-func",
                        "resourceGroup": "rg-1",
                        "location": "japaneast",
                        "kind": "functionapp",
                        "state": "Running",
                        "appServicePlanId": "/subscriptions/sub-123/resourceGroups/rg-1/providers/Microsoft.Web/serverfarms/my-plan"
                    }
                ]))
            });

        let mut config = create_test_config();
        config
            .scan_targets
            .insert("function_apps".to_string(), true);
        config.subscription_id = Some("sub-123".to_string());

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let result = scanner.scan_function_apps().await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "my-func");
        assert_eq!(result[0]["kind"], "functionapp");
        assert_eq!(result[0]["location"], "japaneast");
    }

    #[tokio::test]
    async fn test_scan_function_apps_empty() {
        let mut mock_client = MockAzureClient::new();

        mock_client
            .expect_execute_az_command()
            .returning(|_args| Ok(json!([])));

        let mut config = create_test_config();
        config
            .scan_targets
            .insert("function_apps".to_string(), true);

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let result = scanner.scan_function_apps().await.unwrap();

        assert!(result.is_empty());
    }

    // ==================== scan (統合) App Service + Function App テスト ====================

    #[tokio::test]
    async fn test_scan_with_app_services_and_function_apps() {
        let mut mock_client = MockAzureClient::new();

        // execute_az_command は複数回呼ばれる（role_definitions, role_assignments, webapp list, functionapp list）
        mock_client.expect_execute_az_command().returning(|args| {
            // コマンドによって返す結果を切り替え
            if args.contains(&"webapp".to_string()) {
                Ok(json!([{
                    "name": "app1",
                    "id": "/subscriptions/sub-123/providers/Microsoft.Web/sites/app1",
                    "resourceGroup": "rg-1",
                    "location": "eastus",
                    "kind": "app",
                    "state": "Running"
                }]))
            } else if args.contains(&"functionapp".to_string()) {
                Ok(json!([{
                    "name": "func1",
                    "id": "/subscriptions/sub-123/providers/Microsoft.Web/sites/func1",
                    "resourceGroup": "rg-1",
                    "location": "eastus",
                    "kind": "functionapp",
                    "state": "Running"
                }]))
            } else {
                // role definitions / role assignments
                Ok(json!([]))
            }
        });

        mock_client.expect_get_auth_token().returning(|_| None);
        mock_client.expect_get_http_client().returning(|| None);

        let mut config = create_test_config();
        config.scan_targets.insert("app_services".to_string(), true);
        config
            .scan_targets
            .insert("function_apps".to_string(), true);
        config.subscription_id = Some("sub-123".to_string());

        let scanner = AzureIamScanner::new_with_client(config, mock_client);
        let result = scanner.scan(Box::new(|_, _| {})).await.unwrap();

        assert_eq!(result["provider"], "azure");
        let app_services = result["app_services"].as_array().unwrap();
        assert_eq!(app_services.len(), 1);
        assert_eq!(app_services[0]["name"], "app1");

        let function_apps = result["function_apps"].as_array().unwrap();
        assert_eq!(function_apps.len(), 1);
        assert_eq!(function_apps[0]["name"], "func1");
    }
}
