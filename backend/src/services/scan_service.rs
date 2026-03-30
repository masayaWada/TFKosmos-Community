use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::infra::aws::provider::AwsProvider;
use crate::infra::azure::provider::AzureProvider;
use crate::infra::provider_trait::CloudProviderScanner;
use crate::models::{ScanConfig, ScanResponse};

/// スキャン結果のTTL（1時間）
const SCAN_RESULT_TTL: Duration = Duration::from_secs(3600);

/// ストリーミングスキャンの進捗イベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgressEvent {
    pub scan_id: String,
    pub event_type: String, // "progress", "resource", "completed", "error"
    pub progress: u32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// スキャナー作成のトレイト（DIとモック化のため）
#[async_trait]
pub trait ScannerFactory: Send + Sync {
    async fn run_scan(
        &self,
        config: ScanConfig,
        callback: Box<dyn Fn(u32, String) + Send + Sync>,
    ) -> Result<serde_json::Value>;
}

/// テスト用手動モック（mockall は Fn 引数に非対応のため手書き）
#[cfg(test)]
type MockScanFn = Box<dyn Fn(ScanConfig) -> Result<serde_json::Value> + Send + Sync>;

#[cfg(test)]
pub struct MockScannerFactory {
    run_scan_fn: std::sync::Mutex<Option<MockScanFn>>,
}

#[cfg(test)]
impl MockScannerFactory {
    pub fn new() -> Self {
        Self {
            run_scan_fn: std::sync::Mutex::new(None),
        }
    }

    pub fn expect_run_scan(&mut self) -> MockRunScanExpectation<'_> {
        MockRunScanExpectation(self)
    }
}

#[cfg(test)]
pub struct MockRunScanExpectation<'a>(&'a mut MockScannerFactory);

#[cfg(test)]
impl MockRunScanExpectation<'_> {
    pub fn returning<F>(self, f: F)
    where
        F: Fn(ScanConfig, Box<dyn Fn(u32, String) + Send + Sync>) -> Result<serde_json::Value>
            + Send
            + Sync
            + 'static,
    {
        *self.0.run_scan_fn.lock().unwrap() =
            Some(Box::new(move |config| f(config, Box::new(|_, _| {}))));
    }
}

#[cfg(test)]
#[async_trait]
impl ScannerFactory for MockScannerFactory {
    async fn run_scan(
        &self,
        config: ScanConfig,
        _callback: Box<dyn Fn(u32, String) + Send + Sync>,
    ) -> Result<serde_json::Value> {
        let guard = self.run_scan_fn.lock().unwrap();
        match &*guard {
            Some(f) => f(config),
            None => anyhow::bail!("MockScannerFactory: no expectation set for run_scan"),
        }
    }
}

/// 本番用スキャナーファクトリー（プロバイダーレジストリベース）
///
/// 登録されたプロバイダー実装からスキャナーを選択して実行する。
/// 新規プロバイダー追加時は `CloudProviderScanner` を実装して `register` するだけでよい。
pub struct RealScannerFactory {
    providers: Vec<Box<dyn CloudProviderScanner>>,
}

impl RealScannerFactory {
    /// デフォルトのプロバイダー（AWS, Azure）を登録したファクトリーを生成する
    pub fn new() -> Self {
        let mut factory = Self {
            providers: Vec::new(),
        };
        factory.register(Box::new(AwsProvider));
        factory.register(Box::new(AzureProvider));
        factory
    }

    /// プロバイダーを登録する
    pub fn register(&mut self, provider: Box<dyn CloudProviderScanner>) {
        self.providers.push(provider);
    }
}

#[async_trait]
impl ScannerFactory for RealScannerFactory {
    async fn run_scan(
        &self,
        config: ScanConfig,
        callback: Box<dyn Fn(u32, String) + Send + Sync>,
    ) -> Result<serde_json::Value> {
        let provider_name = config.provider.clone();
        let provider = self
            .providers
            .iter()
            .find(|p| p.provider_name() == provider_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", provider_name))?;

        provider.scan(config, callback).await.map_err(|e| {
            tracing::error!(provider = %provider_name, error = %e, "Scan failed");
            e
        })
    }
}

// In-memory storage for scan results (in production, use Redis or database)
type ScanResults = Arc<RwLock<HashMap<String, ScanResult>>>;

struct ScanResult {
    scan_id: String,
    status: String,
    progress: Option<u32>,
    message: Option<String>,
    _config: ScanConfig,
    data: Option<serde_json::Value>,
    /// エントリ作成時刻（TTL管理用）
    created_at: Instant,
}

pub struct ScanService {
    factory: Arc<dyn ScannerFactory>,
    results: ScanResults,
}

impl ScanService {
    pub fn new(factory: Arc<dyn ScannerFactory>) -> Self {
        Self {
            factory,
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// スキャン進捗状況を更新する
    async fn update_progress_in(
        results: &ScanResults,
        scan_id: &str,
        progress: u32,
        message: String,
    ) {
        let mut r = results.write().await;
        if let Some(entry) = r.get_mut(scan_id) {
            entry.progress = Some(progress);
            entry.message = Some(message);
        }
    }

    /// スキャン結果の初期エントリを挿入する
    async fn init_scan_result_in(results: &ScanResults, scan_id: &str, config: &ScanConfig) {
        let scan_result = ScanResult {
            scan_id: scan_id.to_string(),
            status: "in_progress".to_string(),
            progress: Some(0),
            message: Some("スキャンを開始しています...".to_string()),
            _config: config.clone(),
            data: None,
            created_at: Instant::now(),
        };
        results
            .write()
            .await
            .insert(scan_id.to_string(), scan_result);
    }

    /// 非ストリームスキャン用プログレスコールバックを生成する
    fn make_simple_progress_callback(
        scan_id: String,
        results: ScanResults,
    ) -> Box<dyn Fn(u32, String) + Send + Sync> {
        Box::new(move |progress: u32, message: String| {
            let scan_id = scan_id.clone();
            let results = results.clone();
            tokio::spawn(async move {
                Self::update_progress_in(&results, &scan_id, progress, message).await;
            });
        })
    }

    /// ストリームスキャン用プログレスコールバックを生成する
    fn make_stream_progress_callback(
        scan_id: String,
        results: ScanResults,
        tx: mpsc::Sender<ScanProgressEvent>,
    ) -> Box<dyn Fn(u32, String) + Send + Sync> {
        Box::new(move |progress: u32, message: String| {
            let scan_id = scan_id.clone();
            let results = results.clone();
            let tx = tx.clone();
            let (resource_type, resource_count) = Self::parse_progress_message(&message);
            tokio::spawn(async move {
                Self::update_progress_in(&results, &scan_id, progress, message.clone()).await;
                let _ = tx
                    .send(ScanProgressEvent {
                        scan_id,
                        event_type: if resource_count.is_some() {
                            "resource".to_string()
                        } else {
                            "progress".to_string()
                        },
                        progress,
                        message,
                        resource_type,
                        resource_count,
                        data: None,
                    })
                    .await;
            });
        })
    }

    /// 非ストリームスキャンの完了/失敗を反映する
    async fn finalize_scan(
        results: &ScanResults,
        scan_id: &str,
        result: Result<serde_json::Value>,
    ) {
        match result {
            Ok(json_data) => {
                let mut r = results.write().await;
                if let Some(entry) = r.get_mut(scan_id) {
                    entry.status = "completed".to_string();
                    entry.progress = Some(100);
                    entry.message = Some("スキャンが完了しました".to_string());
                    entry.data = Some(json_data);
                    tracing::info!(scan_id = %scan_id, "Scan completed successfully");
                } else {
                    tracing::error!(scan_id = %scan_id, "Scan result not found");
                }
            }
            Err(e) => {
                tracing::error!(scan_id = %scan_id, error = %e, "Scan failed");
                let mut r = results.write().await;
                if let Some(entry) = r.get_mut(scan_id) {
                    entry.status = "failed".to_string();
                    entry.message = Some(format!("スキャンに失敗しました: {}", e));
                }
            }
        }
    }

    /// ストリームスキャンの完了/失敗を反映し、SSEイベントを送信する
    async fn finalize_scan_stream(
        results: &ScanResults,
        scan_id: &str,
        tx: mpsc::Sender<ScanProgressEvent>,
        result: Result<serde_json::Value>,
    ) {
        match result {
            Ok(json_data) => {
                let mut r = results.write().await;
                if let Some(entry) = r.get_mut(scan_id) {
                    entry.status = "completed".to_string();
                    entry.progress = Some(100);
                    entry.message = Some("スキャンが完了しました".to_string());
                    entry.data = Some(json_data.clone());
                }
                drop(r);
                let _ = tx
                    .send(ScanProgressEvent {
                        scan_id: scan_id.to_string(),
                        event_type: "completed".to_string(),
                        progress: 100,
                        message: "スキャンが完了しました".to_string(),
                        resource_type: None,
                        resource_count: None,
                        data: Some(json_data),
                    })
                    .await;
            }
            Err(e) => {
                let error_msg = format!("スキャンに失敗しました: {}", e);
                let mut r = results.write().await;
                if let Some(entry) = r.get_mut(scan_id) {
                    entry.status = "failed".to_string();
                    entry.message = Some(error_msg.clone());
                }
                drop(r);
                let _ = tx
                    .send(ScanProgressEvent {
                        scan_id: scan_id.to_string(),
                        event_type: "error".to_string(),
                        progress: 0,
                        message: error_msg,
                        resource_type: None,
                        resource_count: None,
                        data: None,
                    })
                    .await;
            }
        }
    }

    pub async fn start_scan(&self, config: ScanConfig) -> Result<String> {
        let scan_id = Uuid::new_v4().to_string();
        let results = self.results.clone();
        let factory = self.factory.clone();

        Self::init_scan_result_in(&results, &scan_id, &config).await;

        let scan_id_clone = scan_id.clone();
        tokio::spawn(async move {
            let callback =
                Self::make_simple_progress_callback(scan_id_clone.clone(), results.clone());
            let result = factory.run_scan(config, callback).await;
            Self::finalize_scan(&results, &scan_id_clone, result).await;
        });

        Ok(scan_id)
    }

    /// ストリーミングスキャンを開始し、進捗イベントをチャネル経由で送信する
    pub async fn start_scan_stream(
        &self,
        config: ScanConfig,
    ) -> Result<mpsc::Receiver<ScanProgressEvent>> {
        let scan_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel::<ScanProgressEvent>(100);
        let results = self.results.clone();
        let factory = self.factory.clone();

        Self::init_scan_result_in(&results, &scan_id, &config).await;

        // 初期イベントを送信
        let _ = tx
            .send(ScanProgressEvent {
                scan_id: scan_id.clone(),
                event_type: "progress".to_string(),
                progress: 0,
                message: "スキャンを開始しています...".to_string(),
                resource_type: None,
                resource_count: None,
                data: None,
            })
            .await;

        let scan_id_clone = scan_id.clone();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let callback = Self::make_stream_progress_callback(
                scan_id_clone.clone(),
                results.clone(),
                tx_clone.clone(),
            );
            let result = factory.run_scan(config, callback).await;
            Self::finalize_scan_stream(&results, &scan_id_clone, tx_clone, result).await;
        });

        Ok(rx)
    }

    /// 進捗メッセージからリソースタイプと件数を抽出
    fn parse_progress_message(message: &str) -> (Option<String>, Option<usize>) {
        // パターン: "XXXのスキャン完了: N件"
        if message.contains("完了:") && message.contains("件") {
            let parts: Vec<&str> = message.split("完了:").collect();
            if parts.len() == 2 {
                let resource_type = parts[0]
                    .replace("のスキャン", "")
                    .replace("IAM ", "")
                    .trim()
                    .to_lowercase();
                let count_str = parts[1].replace("件", "").trim().to_string();
                if let Ok(count) = count_str.parse::<usize>() {
                    return (Some(resource_type), Some(count));
                }
            }
        }
        (None, None)
    }

    pub async fn get_scan_result(&self, scan_id: &str) -> Option<ScanResponse> {
        let results = self.results.read().await;
        results
            .get(scan_id)
            .and_then(|result| {
                if result.created_at.elapsed() > SCAN_RESULT_TTL {
                    return None;
                }
                Some(result)
            })
            .map(|result| {
                let summary = result.data.as_ref().map(|data| {
                    let mut summary = std::collections::HashMap::new();
                    if let Some(provider) = data.get("provider").and_then(|v| v.as_str()) {
                        if provider == "aws" {
                            // IAM
                            if let Some(users) = data.get("users").and_then(|v| v.as_array()) {
                                summary.insert("users".to_string(), users.len());
                            }
                            if let Some(groups) = data.get("groups").and_then(|v| v.as_array()) {
                                summary.insert("groups".to_string(), groups.len());
                            }
                            if let Some(roles) = data.get("roles").and_then(|v| v.as_array()) {
                                summary.insert("roles".to_string(), roles.len());
                            }
                            if let Some(policies) = data.get("policies").and_then(|v| v.as_array())
                            {
                                summary.insert("policies".to_string(), policies.len());
                            }
                            if let Some(attachments) =
                                data.get("attachments").and_then(|v| v.as_array())
                            {
                                summary.insert("attachments".to_string(), attachments.len());
                            }
                            if let Some(cleanup) = data.get("cleanup").and_then(|v| v.as_array()) {
                                summary.insert("cleanup".to_string(), cleanup.len());
                            }
                            // S3
                            if let Some(v) = data.get("buckets").and_then(|v| v.as_array()) {
                                summary.insert("buckets".to_string(), v.len());
                            }
                            if let Some(v) = data.get("bucket_policies").and_then(|v| v.as_array())
                            {
                                summary.insert("bucket_policies".to_string(), v.len());
                            }
                            if let Some(v) = data.get("lifecycle_rules").and_then(|v| v.as_array())
                            {
                                summary.insert("lifecycle_rules".to_string(), v.len());
                            }
                            // EC2
                            if let Some(v) = data.get("instances").and_then(|v| v.as_array()) {
                                summary.insert("instances".to_string(), v.len());
                            }
                            // VPC
                            if let Some(v) = data.get("vpcs").and_then(|v| v.as_array()) {
                                summary.insert("vpcs".to_string(), v.len());
                            }
                            if let Some(v) = data.get("subnets").and_then(|v| v.as_array()) {
                                summary.insert("subnets".to_string(), v.len());
                            }
                            if let Some(v) = data.get("route_tables").and_then(|v| v.as_array()) {
                                summary.insert("route_tables".to_string(), v.len());
                            }
                            if let Some(v) = data.get("security_groups").and_then(|v| v.as_array())
                            {
                                summary.insert("security_groups".to_string(), v.len());
                            }
                            if let Some(v) = data.get("network_acls").and_then(|v| v.as_array()) {
                                summary.insert("network_acls".to_string(), v.len());
                            }
                            // RDS
                            if let Some(v) = data.get("db_instances").and_then(|v| v.as_array()) {
                                summary.insert("db_instances".to_string(), v.len());
                            }
                            if let Some(v) = data.get("db_subnet_groups").and_then(|v| v.as_array())
                            {
                                summary.insert("db_subnet_groups".to_string(), v.len());
                            }
                            if let Some(v) =
                                data.get("db_parameter_groups").and_then(|v| v.as_array())
                            {
                                summary.insert("db_parameter_groups".to_string(), v.len());
                            }
                            // VPC (IGW/NAT/EIP)
                            if let Some(v) =
                                data.get("internet_gateways").and_then(|v| v.as_array())
                            {
                                summary.insert("internet_gateways".to_string(), v.len());
                            }
                            if let Some(v) = data.get("nat_gateways").and_then(|v| v.as_array()) {
                                summary.insert("nat_gateways".to_string(), v.len());
                            }
                            if let Some(v) = data.get("elastic_ips").and_then(|v| v.as_array()) {
                                summary.insert("elastic_ips".to_string(), v.len());
                            }
                            // Lambda
                            if let Some(v) = data.get("functions").and_then(|v| v.as_array()) {
                                summary.insert("functions".to_string(), v.len());
                            }
                            if let Some(v) = data.get("lambda_layers").and_then(|v| v.as_array()) {
                                summary.insert("lambda_layers".to_string(), v.len());
                            }
                            // DynamoDB
                            if let Some(v) = data.get("dynamodb_tables").and_then(|v| v.as_array())
                            {
                                summary.insert("dynamodb_tables".to_string(), v.len());
                            }
                            // ELB/ALB
                            if let Some(v) = data.get("load_balancers").and_then(|v| v.as_array()) {
                                summary.insert("load_balancers".to_string(), v.len());
                            }
                            if let Some(v) = data.get("lb_listeners").and_then(|v| v.as_array()) {
                                summary.insert("lb_listeners".to_string(), v.len());
                            }
                            if let Some(v) = data.get("lb_target_groups").and_then(|v| v.as_array())
                            {
                                summary.insert("lb_target_groups".to_string(), v.len());
                            }
                            // CloudWatch/SNS
                            if let Some(v) =
                                data.get("cloudwatch_alarms").and_then(|v| v.as_array())
                            {
                                summary.insert("cloudwatch_alarms".to_string(), v.len());
                            }
                            if let Some(v) = data.get("sns_topics").and_then(|v| v.as_array()) {
                                summary.insert("sns_topics".to_string(), v.len());
                            }
                            if let Some(v) =
                                data.get("sns_subscriptions").and_then(|v| v.as_array())
                            {
                                summary.insert("sns_subscriptions".to_string(), v.len());
                            }
                        } else if provider == "azure" {
                            // IAM
                            if let Some(role_definitions) =
                                data.get("role_definitions").and_then(|v| v.as_array())
                            {
                                summary
                                    .insert("role_definitions".to_string(), role_definitions.len());
                            }
                            if let Some(role_assignments) =
                                data.get("role_assignments").and_then(|v| v.as_array())
                            {
                                summary
                                    .insert("role_assignments".to_string(), role_assignments.len());
                            }
                            // Compute
                            if let Some(v) = data.get("virtual_machines").and_then(|v| v.as_array())
                            {
                                summary.insert("virtual_machines".to_string(), v.len());
                            }
                            // Network
                            if let Some(v) = data.get("virtual_networks").and_then(|v| v.as_array())
                            {
                                summary.insert("virtual_networks".to_string(), v.len());
                            }
                            if let Some(v) = data
                                .get("network_security_groups")
                                .and_then(|v| v.as_array())
                            {
                                summary.insert("network_security_groups".to_string(), v.len());
                            }
                            // Storage
                            if let Some(v) = data.get("storage_accounts").and_then(|v| v.as_array())
                            {
                                summary.insert("storage_accounts".to_string(), v.len());
                            }
                            // SQL
                            if let Some(v) = data.get("sql_databases").and_then(|v| v.as_array()) {
                                summary.insert("sql_databases".to_string(), v.len());
                            }
                            // App Service
                            if let Some(v) = data.get("app_services").and_then(|v| v.as_array()) {
                                summary.insert("app_services".to_string(), v.len());
                            }
                            if let Some(v) = data.get("function_apps").and_then(|v| v.as_array()) {
                                summary.insert("function_apps".to_string(), v.len());
                            }
                        }
                    }
                    summary
                });

                ScanResponse {
                    scan_id: result.scan_id.clone(),
                    status: result.status.clone(),
                    progress: result.progress,
                    message: result.message.clone(),
                    summary,
                }
            })
    }

    pub async fn get_scan_data(&self, scan_id: &str) -> Option<serde_json::Value> {
        let results = self.results.read().await;
        results.get(scan_id).and_then(|result| {
            if result.created_at.elapsed() > SCAN_RESULT_TTL {
                return None;
            }
            result.data.clone()
        })
    }

    /// 期限切れのスキャン結果を削除する
    pub async fn cleanup_expired_scans(&self) {
        let mut results = self.results.write().await;
        let before = results.len();
        results.retain(|_, v| v.created_at.elapsed() <= SCAN_RESULT_TTL);
        let removed = before - results.len();
        if removed > 0 {
            tracing::info!(removed_count = removed, "Cleaned up expired scan results");
        }
    }

    /// テスト用: 全スキャン結果をクリアする
    #[cfg(test)]
    pub async fn clear_all(&self) {
        self.results.write().await.clear();
    }

    /// テスト用: スキャン結果を直接挿入する
    #[cfg(test)]
    pub async fn insert_test_scan_data(
        &self,
        scan_id: String,
        config: ScanConfig,
        data: serde_json::Value,
    ) {
        let scan_result = ScanResult {
            scan_id: scan_id.clone(),
            status: "completed".to_string(),
            progress: Some(100),
            message: Some("Test scan completed".to_string()),
            _config: config,
            data: Some(data),
            created_at: Instant::now(),
        };
        self.results.write().await.insert(scan_id, scan_result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_service() -> ScanService {
        let mut mock = MockScannerFactory::new();
        mock.expect_run_scan().returning(|_, _| {
            Ok(serde_json::json!({
                "provider": "aws",
                "users": [],
            }))
        });
        ScanService::new(Arc::new(mock))
    }

    #[test]
    fn test_scan_service_can_be_created() {
        let _service = create_mock_service();
    }

    #[tokio::test]
    async fn test_scan_result_not_found() {
        let service = create_mock_service();
        let result = service.get_scan_result("non-existent-scan-id").await;
        assert!(result.is_none(), "Expected None for non-existent scan ID");
    }

    #[tokio::test]
    async fn test_scan_data_not_found() {
        let service = create_mock_service();
        let result = service.get_scan_data("non-existent-scan-id").await;
        assert!(result.is_none(), "Expected None for non-existent scan ID");
    }

    #[tokio::test]
    async fn test_update_progress_for_nonexistent_scan() {
        let service = create_mock_service();
        // should not panic
        ScanService::update_progress_in(
            &service.results,
            "non-existent-scan-id",
            50,
            "Test message".to_string(),
        )
        .await;
        let result = service.get_scan_result("non-existent-scan-id").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_clear_all_removes_all_scan_results() {
        let service = create_mock_service();
        let config = ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets: std::collections::HashMap::new(),
            filters: std::collections::HashMap::new(),
            include_tags: true,
        };
        service
            .insert_test_scan_data(
                "test-1".to_string(),
                config.clone(),
                serde_json::json!({"provider": "aws"}),
            )
            .await;
        service
            .insert_test_scan_data(
                "test-2".to_string(),
                config,
                serde_json::json!({"provider": "aws"}),
            )
            .await;
        assert!(service.get_scan_data("test-1").await.is_some());
        assert!(service.get_scan_data("test-2").await.is_some());

        service.clear_all().await;

        assert!(service.get_scan_data("test-1").await.is_none());
        assert!(service.get_scan_data("test-2").await.is_none());
    }

    #[tokio::test]
    async fn test_start_scan_returns_scan_id() {
        let service = create_mock_service();
        let config = ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets: std::collections::HashMap::new(),
            filters: std::collections::HashMap::new(),
            include_tags: true,
        };
        let result = service.start_scan(config).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    fn make_aws_config() -> ScanConfig {
        ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets: std::collections::HashMap::new(),
            filters: std::collections::HashMap::new(),
            include_tags: true,
        }
    }

    fn make_azure_config() -> ScanConfig {
        ScanConfig {
            provider: "azure".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: Some("sub-123".to_string()),
            tenant_id: Some("tenant-123".to_string()),
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets: std::collections::HashMap::new(),
            filters: std::collections::HashMap::new(),
            include_tags: true,
        }
    }

    #[tokio::test]
    async fn test_get_scan_result_with_aws_data() {
        let service = create_mock_service();
        let config = make_aws_config();

        let data = serde_json::json!({
            "provider": "aws",
            "users": [{"name": "user1"}, {"name": "user2"}],
            "groups": [{"name": "group1"}],
            "roles": [{"name": "role1"}, {"name": "role2"}, {"name": "role3"}],
            "policies": [{"name": "policy1"}],
            "buckets": [{"name": "bucket1"}, {"name": "bucket2"}],
            "instances": [{"id": "i-123"}],
            "vpcs": [{"id": "vpc-123"}],
            "functions": [{"name": "func1"}, {"name": "func2"}],
            "dynamodb_tables": [{"name": "table1"}],
            "load_balancers": [{"name": "lb1"}],
            "cloudwatch_alarms": [{"name": "alarm1"}],
            "sns_topics": [{"name": "topic1"}, {"name": "topic2"}],
        });

        service
            .insert_test_scan_data("aws-scan-1".to_string(), config, data)
            .await;

        let result = service.get_scan_result("aws-scan-1").await;
        assert!(result.is_some());
        let resp = result.unwrap();
        assert_eq!(resp.scan_id, "aws-scan-1");
        assert_eq!(resp.status, "completed");

        let summary = resp.summary.unwrap();
        assert_eq!(summary.get("users"), Some(&2));
        assert_eq!(summary.get("groups"), Some(&1));
        assert_eq!(summary.get("roles"), Some(&3));
        assert_eq!(summary.get("policies"), Some(&1));
        assert_eq!(summary.get("buckets"), Some(&2));
        assert_eq!(summary.get("instances"), Some(&1));
        assert_eq!(summary.get("vpcs"), Some(&1));
        assert_eq!(summary.get("functions"), Some(&2));
        assert_eq!(summary.get("dynamodb_tables"), Some(&1));
        assert_eq!(summary.get("load_balancers"), Some(&1));
        assert_eq!(summary.get("cloudwatch_alarms"), Some(&1));
        assert_eq!(summary.get("sns_topics"), Some(&2));
    }

    #[tokio::test]
    async fn test_get_scan_result_with_azure_data() {
        let service = create_mock_service();
        let config = make_azure_config();

        let data = serde_json::json!({
            "provider": "azure",
            "role_definitions": [{"name": "Reader"}, {"name": "Contributor"}, {"name": "Owner"}],
            "role_assignments": [{"name": "assignment-1"}],
            "virtual_machines": [{"name": "vm1"}, {"name": "vm2"}],
            "virtual_networks": [{"name": "vnet1"}],
            "network_security_groups": [{"name": "nsg1"}, {"name": "nsg2"}],
            "storage_accounts": [{"name": "storage1"}],
            "sql_databases": [{"name": "db1"}],
            "app_services": [{"name": "app1"}, {"name": "app2"}],
            "function_apps": [{"name": "func1"}],
        });

        service
            .insert_test_scan_data("azure-scan-1".to_string(), config, data)
            .await;

        let result = service.get_scan_result("azure-scan-1").await;
        assert!(result.is_some());
        let resp = result.unwrap();
        assert_eq!(resp.scan_id, "azure-scan-1");
        assert_eq!(resp.status, "completed");

        let summary = resp.summary.unwrap();
        assert_eq!(summary.get("role_definitions"), Some(&3));
        assert_eq!(summary.get("role_assignments"), Some(&1));
        assert_eq!(summary.get("virtual_machines"), Some(&2));
        assert_eq!(summary.get("virtual_networks"), Some(&1));
        assert_eq!(summary.get("network_security_groups"), Some(&2));
        assert_eq!(summary.get("storage_accounts"), Some(&1));
        assert_eq!(summary.get("sql_databases"), Some(&1));
        assert_eq!(summary.get("app_services"), Some(&2));
        assert_eq!(summary.get("function_apps"), Some(&1));
    }

    #[test]
    fn test_parse_progress_message_with_resource() {
        // "XXXのスキャン完了: N件" パターン
        let (resource_type, count) =
            ScanService::parse_progress_message("IAM ユーザーのスキャン完了: 5件");
        assert!(resource_type.is_some());
        assert_eq!(count, Some(5));
        // "IAM " と "のスキャン" が除去され lowercase になる
        let rtype = resource_type.unwrap();
        assert_eq!(rtype, "ユーザー");
    }

    #[test]
    fn test_parse_progress_message_no_match() {
        let (resource_type, count) =
            ScanService::parse_progress_message("スキャンを開始しています...");
        assert!(resource_type.is_none());
        assert!(count.is_none());
    }

    #[test]
    fn test_parse_progress_message_no_count() {
        // "件" を含まないメッセージ
        let (resource_type, count) = ScanService::parse_progress_message("完了: 成功");
        assert!(resource_type.is_none());
        assert!(count.is_none());
    }

    #[test]
    fn test_parse_progress_message_zero_count() {
        let (resource_type, count) =
            ScanService::parse_progress_message("ロールのスキャン完了: 0件");
        assert!(resource_type.is_some());
        assert_eq!(count, Some(0));
    }

    #[tokio::test]
    async fn test_cleanup_expired_scans_removes_nothing_for_fresh_scans() {
        let service = create_mock_service();
        let config = make_aws_config();

        service
            .insert_test_scan_data(
                "fresh-scan".to_string(),
                config,
                serde_json::json!({"provider": "aws"}),
            )
            .await;

        // TTLは1時間なので新規挿入直後は削除されない
        service.cleanup_expired_scans().await;

        // データがまだ存在することを確認
        assert!(service.get_scan_data("fresh-scan").await.is_some());
    }

    #[tokio::test]
    async fn test_get_scan_result_returns_none_for_missing_data() {
        let service = create_mock_service();
        let config = make_aws_config();

        // data なし（provider キーがない）のデータを挿入
        service
            .insert_test_scan_data(
                "no-provider-scan".to_string(),
                config,
                serde_json::json!({"other": "value"}),
            )
            .await;

        let result = service.get_scan_result("no-provider-scan").await;
        assert!(result.is_some());
        // provider がなければ summary は空のマップ
        let summary = result.unwrap().summary.unwrap();
        assert!(summary.is_empty());
    }
}
