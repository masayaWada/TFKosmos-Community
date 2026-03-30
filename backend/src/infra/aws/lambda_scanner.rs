//! AWS Lambdaスキャナー

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

use crate::infra::aws::lambda_client_trait::LambdaClientOps;
use crate::models::ScanConfig;

/// AWS Lambdaスキャナー
pub struct AwsLambdaScanner<C: LambdaClientOps> {
    config: ScanConfig,
    lambda_client: Arc<C>,
}

impl<C: LambdaClientOps> AwsLambdaScanner<C> {
    /// 本番用・テスト用共通：クライアントを指定してスキャナーを作成
    pub fn new(config: ScanConfig, client: Arc<C>) -> Self {
        Self {
            config,
            lambda_client: client,
        }
    }

    /// テスト用：モッククライアントを使用してスキャナーを作成
    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: C) -> Self {
        Self {
            config,
            lambda_client: Arc::new(client),
        }
    }

    /// Lambdaリソースをスキャンし結果をresultsに追加
    pub async fn scan_into(
        &self,
        results: &mut serde_json::Map<String, Value>,
        progress_callback: &(dyn Fn(u32, String) + Send + Sync),
        completed_targets: &AtomicUsize,
        total_targets: usize,
    ) -> Result<()> {
        let scan_targets = &self.config.scan_targets;

        // Functions
        if scan_targets.get("functions").copied().unwrap_or(false) {
            debug!("Lambda Functionsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "Lambda Functionsのスキャン中...".to_string(),
            );
            let functions = self.scan_functions().await?;
            let count = functions.len();
            results.insert("functions".to_string(), Value::Array(functions));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            debug!(count, "Lambda Functionsのスキャン完了");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("Lambda Functionsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("functions".to_string(), Value::Array(Vec::new()));
        }

        // Layers
        if scan_targets.get("lambda_layers").copied().unwrap_or(false) {
            debug!("Lambda Layersのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "Lambda Layersのスキャン中...".to_string(),
            );
            let layers = self.scan_layers().await?;
            let count = layers.len();
            results.insert("lambda_layers".to_string(), Value::Array(layers));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            debug!(count, "Lambda Layersのスキャン完了");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("Lambda Layersのスキャン完了: {}件", count),
            );
        } else {
            results.insert("lambda_layers".to_string(), Value::Array(Vec::new()));
        }

        Ok(())
    }

    /// Lambda関数名のフィルタを適用
    fn apply_name_prefix_filter(&self, name: &str) -> bool {
        if let Some(prefix) = self.config.filters.get("name_prefix") {
            name.starts_with(prefix)
        } else {
            true
        }
    }

    /// Lambda関数をスキャン
    async fn scan_functions(&self) -> Result<Vec<Value>> {
        info!("Lambda Functions一覧を取得中");
        let functions_info = self.lambda_client.list_functions().await?;
        let mut functions = Vec::new();

        for func in functions_info {
            if !self.apply_name_prefix_filter(&func.function_name) {
                continue;
            }

            let mut func_json = json!({
                "function_name": func.function_name,
            });

            if let Some(arn) = &func.function_arn {
                func_json["arn"] = json!(arn);
            }
            if let Some(runtime) = &func.runtime {
                func_json["runtime"] = json!(runtime);
            }
            if let Some(handler) = &func.handler {
                func_json["handler"] = json!(handler);
            }
            if let Some(role) = &func.role {
                func_json["role"] = json!(role);
            }
            if let Some(description) = &func.description {
                if !description.is_empty() {
                    func_json["description"] = json!(description);
                }
            }
            if let Some(memory_size) = func.memory_size {
                func_json["memory_size"] = json!(memory_size);
            }
            if let Some(timeout) = func.timeout {
                func_json["timeout"] = json!(timeout);
            }
            if let Some(code_size) = func.code_size {
                func_json["code_size"] = json!(code_size);
            }
            if let Some(last_modified) = &func.last_modified {
                func_json["last_modified"] = json!(last_modified);
            }
            if !func.environment.is_empty() {
                func_json["environment"] = json!(func.environment);
            }
            if !func.vpc_subnet_ids.is_empty() || !func.vpc_security_group_ids.is_empty() {
                func_json["vpc_config"] = json!({
                    "subnet_ids": func.vpc_subnet_ids,
                    "security_group_ids": func.vpc_security_group_ids,
                });
            }
            if !func.layers.is_empty() {
                func_json["layers"] = json!(func.layers);
            }
            if !func.tags.is_empty() {
                let tags: Vec<Value> = func
                    .tags
                    .iter()
                    .map(|(k, v)| json!({"key": k, "value": v}))
                    .collect();
                func_json["tags"] = json!(tags);
            }

            functions.push(func_json);
        }

        info!(count = functions.len(), "Lambda Functions一覧取得完了");
        Ok(functions)
    }

    /// Lambdaレイヤーをスキャン
    async fn scan_layers(&self) -> Result<Vec<Value>> {
        info!("Lambda Layers一覧を取得中");
        let layers_info = self.lambda_client.list_layers().await?;
        let mut layers = Vec::new();

        for layer in layers_info {
            if !self.apply_name_prefix_filter(&layer.layer_name) {
                continue;
            }

            let mut layer_json = json!({
                "layer_name": layer.layer_name,
            });

            if let Some(arn) = &layer.layer_arn {
                layer_json["layer_arn"] = json!(arn);
            }
            if let Some(version) = layer.latest_version {
                layer_json["latest_version"] = json!(version);
            }
            if let Some(description) = &layer.description {
                if !description.is_empty() {
                    layer_json["description"] = json!(description);
                }
            }
            if !layer.compatible_runtimes.is_empty() {
                layer_json["compatible_runtimes"] = json!(layer.compatible_runtimes);
            }

            layers.push(layer_json);
        }

        info!(count = layers.len(), "Lambda Layers一覧取得完了");
        Ok(layers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::lambda_client_trait::mock::MockLambdaClient;
    use crate::infra::aws::lambda_client_trait::{LambdaFunctionInfo, LambdaLayerInfo};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn make_test_config(targets: HashMap<String, bool>) -> ScanConfig {
        ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: Some("test".to_string()),
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets: targets,
            filters: HashMap::new(),
            include_tags: true,
            assume_role_arn: None,
            assume_role_session_name: None,
        }
    }

    #[tokio::test]
    async fn test_scan_functions() {
        let mut mock = MockLambdaClient::new();
        mock.expect_list_functions().returning(|| {
            Ok(vec![LambdaFunctionInfo {
                function_name: "my-function".to_string(),
                function_arn: Some(
                    "arn:aws:lambda:ap-northeast-1:123456789012:function:my-function".to_string(),
                ),
                runtime: Some("python3.12".to_string()),
                handler: Some("index.handler".to_string()),
                role: Some("arn:aws:iam::123456789012:role/lambda-role".to_string()),
                description: Some("Test function".to_string()),
                memory_size: Some(256),
                timeout: Some(30),
                code_size: Some(1024),
                last_modified: Some("2024-01-01T00:00:00Z".to_string()),
                environment: HashMap::from([("ENV".to_string(), "prod".to_string())]),
                vpc_subnet_ids: vec!["subnet-123".to_string()],
                vpc_security_group_ids: vec!["sg-456".to_string()],
                layers: vec!["arn:aws:lambda:ap-northeast-1:123456789012:layer:utils:1".to_string()],
                tags: HashMap::from([("team".to_string(), "backend".to_string())]),
            }])
        });

        let config = make_test_config(HashMap::from([("functions".to_string(), true)]));
        let scanner = AwsLambdaScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let progress = AtomicU32::new(0);
        let cb = |p: u32, _m: String| {
            progress.store(p, Ordering::Relaxed);
        };

        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &cb, &completed, 1)
            .await
            .unwrap();

        let functions = results.get("functions").unwrap().as_array().unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0]["function_name"], "my-function");
        assert_eq!(functions[0]["runtime"], "python3.12");
        assert_eq!(functions[0]["memory_size"], 256);
        assert!(functions[0]["vpc_config"].is_object());
        assert!(functions[0]["tags"].is_array());
    }

    #[tokio::test]
    async fn test_scan_layers() {
        let mut mock = MockLambdaClient::new();
        mock.expect_list_functions().returning(|| Ok(vec![]));
        mock.expect_list_layers().returning(|| {
            Ok(vec![LambdaLayerInfo {
                layer_name: "shared-utils".to_string(),
                layer_arn: Some(
                    "arn:aws:lambda:ap-northeast-1:123456789012:layer:shared-utils".to_string(),
                ),
                latest_version: Some(3),
                description: Some("Shared utilities".to_string()),
                compatible_runtimes: vec!["python3.11".to_string(), "python3.12".to_string()],
            }])
        });

        let config = make_test_config(HashMap::from([
            ("functions".to_string(), false),
            ("lambda_layers".to_string(), true),
        ]));
        let scanner = AwsLambdaScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let layers = results.get("lambda_layers").unwrap().as_array().unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0]["layer_name"], "shared-utils");
        assert_eq!(layers[0]["latest_version"], 3);
    }

    #[tokio::test]
    async fn test_scan_empty_targets() {
        let mut mock = MockLambdaClient::new();
        // list_functions and list_layers should not be called
        mock.expect_list_functions().never();
        mock.expect_list_layers().never();

        let config = make_test_config(HashMap::from([
            ("functions".to_string(), false),
            ("lambda_layers".to_string(), false),
        ]));
        let scanner = AwsLambdaScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        assert!(results
            .get("functions")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        assert!(results
            .get("lambda_layers")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn test_scan_functions_with_name_prefix_filter() {
        let mut mock = MockLambdaClient::new();
        mock.expect_list_functions().returning(|| {
            Ok(vec![
                LambdaFunctionInfo {
                    function_name: "app-handler".to_string(),
                    function_arn: None,
                    runtime: Some("nodejs20.x".to_string()),
                    handler: None,
                    role: None,
                    description: None,
                    memory_size: None,
                    timeout: None,
                    code_size: None,
                    last_modified: None,
                    environment: HashMap::new(),
                    vpc_subnet_ids: vec![],
                    vpc_security_group_ids: vec![],
                    layers: vec![],
                    tags: HashMap::new(),
                },
                LambdaFunctionInfo {
                    function_name: "other-function".to_string(),
                    function_arn: None,
                    runtime: Some("python3.12".to_string()),
                    handler: None,
                    role: None,
                    description: None,
                    memory_size: None,
                    timeout: None,
                    code_size: None,
                    last_modified: None,
                    environment: HashMap::new(),
                    vpc_subnet_ids: vec![],
                    vpc_security_group_ids: vec![],
                    layers: vec![],
                    tags: HashMap::new(),
                },
            ])
        });

        let mut config = make_test_config(HashMap::from([("functions".to_string(), true)]));
        config
            .filters
            .insert("name_prefix".to_string(), "app-".to_string());
        let scanner = AwsLambdaScanner::new_with_client(config, mock);

        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);
        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let functions = results.get("functions").unwrap().as_array().unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0]["function_name"], "app-handler");
    }
}
