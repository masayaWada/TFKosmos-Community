//! スキャン結果のエクスポートサービス
//!
//! スキャンデータをCSVまたはJSON形式でエクスポートします。

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

pub struct ExportService;

impl ExportService {
    /// スキャンデータをJSON形式でエクスポート
    pub fn export_json(scan_data: &Value, resource_types: &[String]) -> Result<String> {
        if resource_types.is_empty() {
            return serde_json::to_string_pretty(scan_data)
                .context("Failed to serialize scan data to JSON");
        }

        let mut filtered = serde_json::Map::new();
        if let Some(provider) = scan_data.get("provider") {
            filtered.insert("provider".to_string(), provider.clone());
        }
        for rt in resource_types {
            if let Some(data) = scan_data.get(rt) {
                filtered.insert(rt.clone(), data.clone());
            }
        }

        serde_json::to_string_pretty(&Value::Object(filtered))
            .context("Failed to serialize filtered data to JSON")
    }

    /// スキャンデータをCSV形式でエクスポート
    pub fn export_csv(scan_data: &Value, resource_types: &[String]) -> Result<String> {
        let mut csv_output = String::new();

        let types_to_export: Vec<&str> = if resource_types.is_empty() {
            // すべてのリソースタイプを抽出
            scan_data
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .filter(|(_, v)| v.is_array())
                        .map(|(k, _)| k.as_str())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            resource_types.iter().map(|s| s.as_str()).collect()
        };

        for resource_type in types_to_export {
            if let Some(resources) = scan_data.get(resource_type).and_then(|v| v.as_array()) {
                if resources.is_empty() {
                    continue;
                }

                // ヘッダー行を追加
                csv_output.push_str(&format!("# {}\n", resource_type));

                // すべてのリソースからキーを収集
                let headers = Self::collect_headers(resources);
                csv_output.push_str(&headers.join(","));
                csv_output.push('\n');

                // データ行を追加
                for resource in resources {
                    let row: Vec<String> = headers
                        .iter()
                        .map(|h| Self::value_to_csv_field(resource.get(h)))
                        .collect();
                    csv_output.push_str(&row.join(","));
                    csv_output.push('\n');
                }

                csv_output.push('\n');
            }
        }

        Ok(csv_output)
    }

    /// リソース配列からすべてのキーを収集（順序を維持）
    fn collect_headers(resources: &[Value]) -> Vec<String> {
        let mut headers_set = HashMap::new();
        let mut headers_order = Vec::new();

        for resource in resources {
            if let Some(obj) = resource.as_object() {
                for key in obj.keys() {
                    if !headers_set.contains_key(key) {
                        headers_set.insert(key.clone(), true);
                        headers_order.push(key.clone());
                    }
                }
            }
        }

        headers_order
    }

    /// JSON値をCSVフィールドに変換
    fn value_to_csv_field(value: Option<&Value>) -> String {
        match value {
            None => String::new(),
            Some(Value::Null) => String::new(),
            Some(Value::String(s)) => {
                if s.contains(',') || s.contains('"') || s.contains('\n') {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s.clone()
                }
            }
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            Some(v) => {
                let s = v.to_string();
                format!("\"{}\"", s.replace('"', "\"\""))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_export_json_all_resources() {
        let data = json!({
            "provider": "aws",
            "users": [{"user_name": "alice"}, {"user_name": "bob"}],
            "roles": [{"role_name": "admin"}],
        });

        let result = ExportService::export_json(&data, &[]).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["provider"], "aws");
        assert_eq!(parsed["users"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_export_json_filtered() {
        let data = json!({
            "provider": "aws",
            "users": [{"user_name": "alice"}],
            "roles": [{"role_name": "admin"}],
        });

        let result = ExportService::export_json(&data, &["users".to_string()]).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("users").is_some());
        assert!(parsed.get("roles").is_none());
    }

    #[test]
    fn test_export_csv_basic() {
        let data = json!({
            "provider": "aws",
            "users": [
                {"user_name": "alice", "arn": "arn:aws:iam::123:user/alice"},
                {"user_name": "bob", "arn": "arn:aws:iam::123:user/bob"},
            ],
        });

        let result = ExportService::export_csv(&data, &["users".to_string()]).unwrap();
        assert!(result.contains("# users"));
        assert!(result.contains("user_name"));
        assert!(result.contains("alice"));
        assert!(result.contains("bob"));
    }

    #[test]
    fn test_csv_field_escaping() {
        assert_eq!(ExportService::value_to_csv_field(None), "");
        assert_eq!(
            ExportService::value_to_csv_field(Some(&json!("hello"))),
            "hello"
        );
        assert_eq!(
            ExportService::value_to_csv_field(Some(&json!("hello,world"))),
            "\"hello,world\""
        );
        assert_eq!(ExportService::value_to_csv_field(Some(&json!(42))), "42");
        assert_eq!(
            ExportService::value_to_csv_field(Some(&json!(true))),
            "true"
        );
    }

    #[test]
    fn test_export_csv_empty_resources() {
        let data = json!({
            "provider": "aws",
            "users": [],
        });

        let result = ExportService::export_csv(&data, &["users".to_string()]).unwrap();
        assert!(!result.contains("# users"));
    }
}
