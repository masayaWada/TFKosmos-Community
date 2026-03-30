//! リソース管理サービス
//!
//! スキャン済みクラウドリソースの取得・フィルタリング・選択状態の管理を担当する。
//! クエリ言語（`==` / `!=` 演算子）によるフィルタリング、ページネーション、
//! リソース選択のインメモリ保持をサポートする。

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::infra::query::{Lexer, QueryEvaluator, QueryParser};
use crate::models::ResourceListResponse;
use crate::services::scan_service::ScanService;

/// リソースサービスのエラー型
#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("Scan not found: {0}")]
    ScanNotFound(String),
    #[error("Query syntax error: {0}")]
    QuerySyntaxError(String),
    #[error("Query parse error: {0}")]
    QueryParseError(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

// In-memory storage for resource selections (in production, use Redis or database)
type ResourceSelections = Arc<RwLock<HashMap<String, HashMap<String, Vec<Value>>>>>;

/// スキャン済みリソースの取得と選択状態を管理するサービス
///
/// リソースの取得時にクエリフィルターとページネーションを適用し、
/// ユーザーが選択したリソースをメモリ上で保持する。
pub struct ResourceService {
    scan_service: Arc<ScanService>,
    selections: ResourceSelections,
}

impl ResourceService {
    /// 新しい `ResourceService` を生成する
    ///
    /// # Arguments
    /// * `scan_service` - スキャンデータ取得に使用するサービス
    pub fn new(scan_service: Arc<ScanService>) -> Self {
        Self {
            scan_service,
            selections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 内部の `ScanService` への参照を返す
    pub fn scan_service(&self) -> &ScanService {
        &self.scan_service
    }

    /// スキャン結果からリソース一覧を取得する
    ///
    /// # Arguments
    /// * `scan_id` - スキャンID
    /// * `resource_type` - フィルタするリソースタイプ（`None` の場合は全タイプ）
    /// * `page` - ページ番号（1始まり）
    /// * `page_size` - 1ページあたりの件数
    /// * `filter_conditions` - クエリフィルター条件
    ///
    /// # Errors
    /// - スキャンが見つからない場合は `ResourceError::ScanNotFound`
    /// - クエリ構文が不正な場合は `ResourceError::QuerySyntaxError`
    pub async fn get_resources(
        &self,
        scan_id: &str,
        resource_type: Option<&str>,
        page: u32,
        page_size: u32,
        filter_conditions: Option<Value>,
    ) -> Result<ResourceListResponse, ResourceError> {
        // Get scan data
        let scan_data = self
            .scan_service
            .get_scan_data(scan_id)
            .await
            .ok_or_else(|| ResourceError::ScanNotFound(scan_id.to_string()))?;

        // Extract resources based on type
        let mut all_resources: Vec<Value> = Vec::new();

        if let Some(rt) = resource_type {
            if let Some(resources) = scan_data.get(rt) {
                if let Some(arr) = resources.as_array() {
                    all_resources = arr.clone();
                }
            }
        } else {
            // Get all resource types
            if let Some(obj) = scan_data.as_object() {
                for (_, resources) in obj {
                    if let Some(arr) = resources.as_array() {
                        all_resources.extend(arr.clone());
                    }
                }
            }
        }

        // Apply filters if provided
        if let Some(filters) = filter_conditions {
            all_resources = Self::apply_filters(all_resources, filters)?;
        }

        let total = all_resources.len();
        let total_pages = (total as f64 / page_size as f64).ceil() as u32;

        // Paginate
        let start = ((page - 1) * page_size) as usize;
        let end = (start + page_size as usize).min(total);
        let resources = if start < total {
            all_resources[start..end].to_vec()
        } else {
            Vec::new()
        };

        // Get provider from scan result
        let provider = scan_data
            .get("provider")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());

        Ok(ResourceListResponse {
            resources,
            total,
            page,
            page_size,
            total_pages,
            provider,
        })
    }

    pub async fn query_resources(
        &self,
        scan_id: &str,
        query: &str,
        resource_type: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<ResourceListResponse, ResourceError> {
        // Parse query
        let mut lexer = Lexer::new(query);
        let tokens = lexer
            .tokenize()
            .map_err(|e| ResourceError::QuerySyntaxError(format!("クエリ構文エラー: {}", e)))?;

        let mut parser = QueryParser::new(tokens);
        let expr = parser
            .parse()
            .map_err(|e| ResourceError::QueryParseError(format!("クエリパースエラー: {}", e)))?;

        // Get scan data
        let scan_data = self
            .scan_service
            .get_scan_data(scan_id)
            .await
            .ok_or_else(|| ResourceError::ScanNotFound(scan_id.to_string()))?;

        // Extract resources based on type
        let mut all_resources: Vec<Value> = Vec::new();

        if let Some(rt) = resource_type {
            if let Some(resources) = scan_data.get(rt) {
                if let Some(arr) = resources.as_array() {
                    all_resources = arr.clone();
                }
            }
        } else {
            // Get all resource types (excluding metadata fields)
            if let Some(obj) = scan_data.as_object() {
                for (key, resources) in obj {
                    // Skip metadata fields
                    if key == "provider" || key == "scan_id" || key == "timestamp" {
                        continue;
                    }
                    if let Some(arr) = resources.as_array() {
                        all_resources.extend(arr.clone());
                    }
                }
            }
        }

        // Filter using query expression
        let filtered: Vec<Value> = all_resources
            .into_iter()
            .filter(|resource| QueryEvaluator::evaluate(&expr, resource))
            .collect();

        let total = filtered.len();
        let total_pages = (total as f64 / page_size as f64).ceil() as u32;

        // Paginate
        let start = ((page - 1) * page_size) as usize;
        let end = (start + page_size as usize).min(total);
        let resources = if start < total {
            filtered[start..end].to_vec()
        } else {
            Vec::new()
        };

        // Get provider from scan result
        let provider = scan_data
            .get("provider")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());

        Ok(ResourceListResponse {
            resources,
            total,
            page,
            page_size,
            total_pages,
            provider,
        })
    }

    pub async fn update_selection(
        &self,
        scan_id: &str,
        selections: HashMap<String, Vec<Value>>,
    ) -> anyhow::Result<Value> {
        let mut storage = self.selections.write().await;
        let scan_selections = storage
            .entry(scan_id.to_string())
            .or_insert_with(HashMap::new);

        // Merge new selections with existing ones
        for (resource_type, ids) in selections {
            scan_selections.insert(resource_type, ids);
        }

        let total_count: usize = scan_selections.values().map(|v| v.len()).sum();

        Ok(json!({
            "success": true,
            "selected_count": total_count
        }))
    }

    pub async fn get_selection(
        &self,
        scan_id: &str,
    ) -> anyhow::Result<HashMap<String, Vec<Value>>> {
        let storage = self.selections.read().await;
        Ok(storage.get(scan_id).cloned().unwrap_or_default())
    }

    /// テスト用: 全選択状態をクリアする
    #[cfg(test)]
    pub async fn clear_all(&self) {
        self.selections.write().await.clear();
    }

    fn apply_filters(resources: Vec<Value>, filters: Value) -> anyhow::Result<Vec<Value>> {
        // Extract search term from filters
        let search_term = filters
            .get("search")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());

        if let Some(term) = search_term {
            if term.is_empty() {
                return Ok(resources);
            }

            // Filter resources that match the search term in any field
            let filtered: Vec<Value> = resources
                .into_iter()
                .filter(|resource| Self::resource_matches_search(resource, &term))
                .collect();

            Ok(filtered)
        } else {
            Ok(resources)
        }
    }

    fn resource_matches_search(resource: &Value, search_term: &str) -> bool {
        // Check if any field in the resource contains the search term
        match resource {
            Value::Object(map) => {
                for (_, value) in map {
                    if Self::value_contains_search(value, search_term) {
                        return true;
                    }
                }
                false
            }
            Value::String(s) => s.to_lowercase().contains(search_term),
            Value::Array(arr) => {
                for item in arr {
                    if Self::resource_matches_search(item, search_term) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn value_contains_search(value: &Value, search_term: &str) -> bool {
        match value {
            Value::String(s) => s.to_lowercase().contains(search_term),
            Value::Number(n) => {
                if let Some(n_str) = n.as_f64().map(|f| f.to_string()) {
                    n_str.contains(search_term)
                } else if let Some(n_str) = n.as_i64().map(|i| i.to_string()) {
                    n_str.contains(search_term)
                } else if let Some(n_str) = n.as_u64().map(|u| u.to_string()) {
                    n_str.contains(search_term)
                } else {
                    false
                }
            }
            Value::Bool(b) => b.to_string().contains(search_term),
            Value::Array(arr) => {
                for item in arr {
                    if Self::value_contains_search(item, search_term) {
                        return true;
                    }
                }
                false
            }
            Value::Object(map) => {
                for (_, v) in map {
                    if Self::value_contains_search(v, search_term) {
                        return true;
                    }
                }
                false
            }
            Value::Null => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// テストデータ生成用ヘルパー関数
    mod test_helpers {
        use serde_json::{json, Value};

        pub fn create_user_resource(name: &str, arn: &str) -> Value {
            json!({
                "name": name,
                "arn": arn
            })
        }

        pub fn create_role_resource(name: &str, permissions: Value) -> Value {
            json!({
                "name": name,
                "permissions": permissions
            })
        }

        pub fn create_group_resource(name: &str, members: Vec<&str>) -> Value {
            json!({
                "name": name,
                "members": members
            })
        }
    }

    #[test]
    fn test_resource_matches_search_string_field() {
        // Arrange
        let resource = test_helpers::create_user_resource(
            "TestUser",
            "arn:aws:iam::123456789012:user/TestUser",
        );

        // Act & Assert
        assert!(
            ResourceService::resource_matches_search(&resource, "testuser"),
            "名前フィールドで検索がマッチするべき"
        );
        assert!(
            ResourceService::resource_matches_search(&resource, "123456789012"),
            "ARNフィールドで検索がマッチするべき"
        );
        assert!(
            !ResourceService::resource_matches_search(&resource, "nonexistent"),
            "存在しない文字列では検索がマッチしないべき"
        );
    }

    #[test]
    fn test_resource_matches_search_case_insensitive() {
        // Arrange
        // 注意: resource_matches_search は検索語を小文字化しない
        // apply_filters で小文字化されるため、直接呼び出す場合は小文字で渡す
        let resource = json!({"name": "AdminUser"});

        // Act & Assert
        assert!(
            ResourceService::resource_matches_search(&resource, "adminuser"),
            "大文字小文字を区別しない検索がマッチするべき"
        );
        assert!(
            ResourceService::resource_matches_search(&resource, "admin"),
            "部分一致検索がマッチするべき"
        );
    }

    #[test]
    fn test_resource_matches_search_nested_object() {
        // Arrange
        let resource = test_helpers::create_role_resource(
            "TestRole",
            json!({
                "action": "s3:GetObject",
                "resource": "*"
            }),
        );

        // Act & Assert
        assert!(
            ResourceService::resource_matches_search(&resource, "s3:getobject"),
            "ネストしたオブジェクトの検索がマッチするべき"
        );
        assert!(
            ResourceService::resource_matches_search(&resource, "testrole"),
            "トップレベルフィールドの検索がマッチするべき"
        );
    }

    #[test]
    fn test_resource_matches_search_array_field() {
        // Arrange
        let resource =
            test_helpers::create_group_resource("TestGroup", vec!["user1", "user2", "admin"]);

        // Act & Assert
        assert!(
            ResourceService::resource_matches_search(&resource, "user1"),
            "配列フィールドの検索がマッチするべき"
        );
        assert!(
            ResourceService::resource_matches_search(&resource, "admin"),
            "配列フィールドの別の要素でも検索がマッチするべき"
        );
        assert!(
            !ResourceService::resource_matches_search(&resource, "user3"),
            "配列に存在しない要素では検索がマッチしないべき"
        );
    }

    #[test]
    fn test_value_contains_search_number() {
        // Arrange
        let value = json!(12345);

        // Act & Assert
        assert!(
            ResourceService::value_contains_search(&value, "123"),
            "数値の部分一致検索がマッチするべき"
        );
        assert!(
            ResourceService::value_contains_search(&value, "12345"),
            "数値の完全一致検索がマッチするべき"
        );
        assert!(
            !ResourceService::value_contains_search(&value, "999"),
            "数値に含まれない文字列では検索がマッチしないべき"
        );
    }

    #[test]
    fn test_value_contains_search_boolean() {
        // Arrange
        let value_true = json!(true);
        let value_false = json!(false);

        // Act & Assert
        assert!(
            ResourceService::value_contains_search(&value_true, "true"),
            "ブール値trueの検索がマッチするべき"
        );
        assert!(
            ResourceService::value_contains_search(&value_false, "false"),
            "ブール値falseの検索がマッチするべき"
        );
    }

    #[test]
    fn test_value_contains_search_null() {
        // Arrange
        let value = json!(null);

        // Act & Assert
        assert!(
            !ResourceService::value_contains_search(&value, "null"),
            "null値は検索にマッチしないべき"
        );
    }

    #[test]
    fn test_apply_filters_with_search_term() {
        // Arrange
        let resources = vec![
            json!({"name": "AdminUser", "type": "user"}),
            json!({"name": "TestRole", "type": "role"}),
            json!({"name": "AdminGroup", "type": "group"}),
        ];
        let filters = json!({"search": "Admin"});

        // Act
        let result = ResourceService::apply_filters(resources, filters).unwrap();

        // Assert
        assert_eq!(result.len(), 2, "フィルタ後のリソース数は2であるべき");
        assert!(
            result.iter().any(|r| r["name"] == "AdminUser"),
            "AdminUserがフィルタ結果に含まれるべき"
        );
        assert!(
            result.iter().any(|r| r["name"] == "AdminGroup"),
            "AdminGroupがフィルタ結果に含まれるべき"
        );
    }

    #[test]
    fn test_apply_filters_empty_search() {
        // Arrange
        let resources = vec![json!({"name": "User1"}), json!({"name": "User2"})];
        let filters = json!({"search": ""});

        // Act
        let result = ResourceService::apply_filters(resources.clone(), filters).unwrap();

        // Assert
        assert_eq!(
            result.len(),
            2,
            "空の検索語では全てのリソースが返されるべき"
        );
    }

    #[test]
    fn test_apply_filters_no_match() {
        // Arrange
        let resources = vec![json!({"name": "User1"}), json!({"name": "User2"})];
        let filters = json!({"search": "nonexistent"});

        // Act
        let result = ResourceService::apply_filters(resources, filters).unwrap();

        // Assert
        assert_eq!(
            result.len(),
            0,
            "マッチしない検索語では空のリストが返されるべき"
        );
    }

    #[test]
    fn test_apply_filters_without_search_key() {
        // Arrange
        let resources = vec![json!({"name": "User1"}), json!({"name": "User2"})];
        let filters = json!({"other_filter": "value"});

        // Act
        let result = ResourceService::apply_filters(resources.clone(), filters).unwrap();

        // Assert
        assert_eq!(
            result.len(),
            2,
            "searchキーがない場合は全てのリソースが返されるべき"
        );
    }

    #[test]
    fn test_resource_matches_search_string_value() {
        let resource = Value::String("hello world".to_string());

        assert!(ResourceService::resource_matches_search(&resource, "hello"));
        assert!(!ResourceService::resource_matches_search(
            &resource, "missing"
        ));
    }

    #[test]
    fn test_resource_matches_search_array_value() {
        let resource = json!(["alice", "bob", "charlie"]);

        assert!(ResourceService::resource_matches_search(&resource, "bob"));
        assert!(!ResourceService::resource_matches_search(&resource, "dave"));
    }

    #[test]
    fn test_resource_matches_search_non_matching_types() {
        // Number, bool, null at top level don't match as resources
        assert!(!ResourceService::resource_matches_search(&json!(42), "42"));
        assert!(!ResourceService::resource_matches_search(
            &json!(true),
            "true"
        ));
        assert!(!ResourceService::resource_matches_search(
            &json!(null),
            "null"
        ));
    }

    #[test]
    fn test_value_contains_search_nested_array() {
        let value = json!(["one", ["two", "three"]]);

        assert!(ResourceService::value_contains_search(&value, "three"));
        assert!(!ResourceService::value_contains_search(&value, "four"));
    }

    #[test]
    fn test_value_contains_search_nested_object() {
        let value = json!({
            "level1": {
                "level2": "deep_value"
            }
        });

        assert!(ResourceService::value_contains_search(&value, "deep_value"));
    }

    #[test]
    fn test_value_contains_search_null_value() {
        assert!(!ResourceService::value_contains_search(
            &json!(null),
            "anything"
        ));
    }

    #[test]
    fn test_apply_filters_case_insensitive_search() {
        let resources = vec![
            json!({"name": "UPPERCASE_USER"}),
            json!({"name": "lowercase_user"}),
        ];
        let filters = json!({"search": "uppercase"});

        let result = ResourceService::apply_filters(resources, filters).unwrap();

        assert_eq!(
            result.len(),
            1,
            "大文字小文字を区別しないフィルタで1件マッチするべき"
        );
    }

    #[test]
    fn test_apply_filters_with_empty_resources() {
        let resources: Vec<Value> = vec![];
        let filters = json!({"search": "anything"});

        let result = ResourceService::apply_filters(resources, filters).unwrap();

        assert!(
            result.is_empty(),
            "空のリソースリストでは空の結果を返すべき"
        );
    }

    #[test]
    fn test_value_contains_search_float_number() {
        let value = json!(9.81);

        assert!(ResourceService::value_contains_search(&value, "9.81"));
        assert!(!ResourceService::value_contains_search(&value, "2.71"));
    }

    #[tokio::test]
    async fn test_clear_all_removes_all_selections() {
        use crate::services::scan_service::{MockScannerFactory, ScanService};

        let mock = MockScannerFactory::new();
        let scan_service = Arc::new(ScanService::new(Arc::new(mock)));
        let resource_service = ResourceService::new(scan_service);

        let mut selections = HashMap::new();
        selections.insert("users".to_string(), vec![json!("user-1")]);
        resource_service
            .update_selection("scan-1", selections)
            .await
            .unwrap();

        let before = resource_service.get_selection("scan-1").await.unwrap();
        assert!(!before.is_empty());

        resource_service.clear_all().await;

        let after = resource_service.get_selection("scan-1").await.unwrap();
        assert!(after.is_empty());
    }

    #[tokio::test]
    async fn test_get_resources_by_type_with_data() {
        use crate::models::ScanConfig;
        use crate::services::scan_service::{MockScannerFactory, ScanService};

        let mock = MockScannerFactory::new();
        let scan_service = Arc::new(ScanService::new(Arc::new(mock)));

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
            scan_targets: HashMap::new(),
            filters: HashMap::new(),
            include_tags: true,
        };
        scan_service
            .insert_test_scan_data(
                "scan-test-1".to_string(),
                config,
                json!({
                    "provider": "aws",
                    "users": [
                        {"user_name": "alice", "arn": "arn:aws:iam::123:user/alice"},
                        {"user_name": "bob", "arn": "arn:aws:iam::123:user/bob"}
                    ],
                    "groups": [
                        {"group_name": "devs", "arn": "arn:aws:iam::123:group/devs"}
                    ]
                }),
            )
            .await;

        let resource_service = ResourceService::new(scan_service);

        // users タイプでフィルタ
        let result = resource_service
            .get_resources("scan-test-1", Some("users"), 1, 10, None)
            .await
            .unwrap();

        assert_eq!(result.total, 2, "Should return 2 users");
        assert_eq!(result.resources.len(), 2);
        assert_eq!(result.provider, Some("aws".to_string()));

        // groups タイプでフィルタ
        let group_result = resource_service
            .get_resources("scan-test-1", Some("groups"), 1, 10, None)
            .await
            .unwrap();

        assert_eq!(group_result.total, 1, "Should return 1 group");
    }

    #[tokio::test]
    async fn test_get_all_resources() {
        use crate::models::ScanConfig;
        use crate::services::scan_service::{MockScannerFactory, ScanService};

        let mock = MockScannerFactory::new();
        let scan_service = Arc::new(ScanService::new(Arc::new(mock)));

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
            scan_targets: HashMap::new(),
            filters: HashMap::new(),
            include_tags: true,
        };
        scan_service
            .insert_test_scan_data(
                "scan-all-1".to_string(),
                config,
                json!({
                    "provider": "aws",
                    "users": [{"user_name": "user1"}],
                    "roles": [{"role_name": "role1"}, {"role_name": "role2"}]
                }),
            )
            .await;

        let resource_service = ResourceService::new(scan_service);

        // resource_type を None にして全リソース取得
        let result = resource_service
            .get_resources("scan-all-1", None, 1, 100, None)
            .await
            .unwrap();

        // provider フィールドは配列ではないため除外され、users(1) + roles(2) = 3
        assert_eq!(
            result.total, 3,
            "Should return all resources: users + roles"
        );
    }

    #[tokio::test]
    async fn test_get_resources_scan_not_found() {
        use crate::services::scan_service::{MockScannerFactory, ScanService};

        let mock = MockScannerFactory::new();
        let scan_service = Arc::new(ScanService::new(Arc::new(mock)));
        let resource_service = ResourceService::new(scan_service);

        let result = resource_service
            .get_resources("non-existent-scan", Some("users"), 1, 10, None)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ResourceError::ScanNotFound(id) => assert_eq!(id, "non-existent-scan"),
            other => panic!("Expected ScanNotFound, got {:?}", other),
        }
    }
}
