use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// ドリフト検出リクエスト
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DriftDetectionRequest {
    /// 比較対象のスキャン ID
    pub scan_id: String,
    /// Terraform state ファイルの JSON 文字列
    pub state_content: String,
}

/// ドリフト検出レスポンス
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DriftDetectionResponse {
    /// ドリフトレポートの一意 ID
    pub drift_id: String,
    /// 比較に使用したスキャン ID
    pub scan_id: String,
    /// サマリー情報
    pub summary: DriftSummary,
    /// 個別のドリフト項目
    pub drifts: Vec<DriftItem>,
}

/// ドリフト検出サマリー
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DriftSummary {
    /// state ファイル内のリソース総数
    pub total_in_state: usize,
    /// クラウド上のリソース総数
    pub total_in_cloud: usize,
    /// クラウドにのみ存在（state に未登録）
    pub added: usize,
    /// state にのみ存在（クラウドから削除済み）
    pub removed: usize,
    /// 属性が異なる
    pub changed: usize,
    /// 一致している
    pub unchanged: usize,
}

/// 個別のドリフト項目
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DriftItem {
    /// Terraform リソースタイプ
    pub resource_type: String,
    /// リソースの識別子
    pub resource_id: String,
    /// ドリフトの種類
    pub drift_type: DriftType,
    /// state 側の属性（Removed / Changed の場合）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_attributes: Option<Value>,
    /// クラウド側の属性（Added / Changed の場合）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_attributes: Option<Value>,
    /// 変更されたフィールドの詳細（Changed の場合）
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub changed_fields: Vec<ChangedField>,
}

/// ドリフトの種類
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DriftType {
    /// クラウドにのみ存在
    Added,
    /// state にのみ存在
    Removed,
    /// 属性が異なる
    Changed,
}

/// 変更されたフィールドの詳細
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChangedField {
    /// フィールド名
    pub field: String,
    /// state 側の値
    pub state_value: Value,
    /// クラウド側の値
    pub cloud_value: Value,
}
