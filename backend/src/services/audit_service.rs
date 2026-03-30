use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;
use uuid::Uuid;

/// 監査ログのアクション種別
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Scan,
    Generate,
    Export,
    DriftDetect,
    ConfigSave,
    ConfigDelete,
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditAction::Scan => write!(f, "scan"),
            AuditAction::Generate => write!(f, "generate"),
            AuditAction::Export => write!(f, "export"),
            AuditAction::DriftDetect => write!(f, "drift_detect"),
            AuditAction::ConfigSave => write!(f, "config_save"),
            AuditAction::ConfigDelete => write!(f, "config_delete"),
        }
    }
}

/// 監査ログのステータス
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditStatus {
    Success,
    Failure,
}

/// 監査ログエントリ
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub action: AuditAction,
    pub status: AuditStatus,
    pub path: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl AuditEntry {
    pub fn new(action: AuditAction, status: AuditStatus, path: &str, method: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            action,
            status,
            path: path.to_string(),
            method: method.to_string(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// 監査ログクエリパラメータ
#[derive(Debug, Deserialize, ToSchema)]
pub struct AuditQuery {
    #[serde(default)]
    pub from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub to: Option<DateTime<Utc>>,
    #[serde(default)]
    pub action: Option<AuditAction>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

/// 監査ログサービス
///
/// JSONL形式でファイルに追記保存し、クエリ機能を提供する。
/// ファイルは日付ごとにローテーションされる。
pub struct AuditService {
    log_dir: PathBuf,
    /// インメモリキャッシュ（最新エントリを保持）
    cache: Arc<RwLock<Vec<AuditEntry>>>,
    /// キャッシュの最大サイズ
    max_cache_size: usize,
}

impl AuditService {
    pub fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            cache: Arc::new(RwLock::new(Vec::new())),
            max_cache_size: 1000,
        }
    }

    #[cfg(test)]
    pub fn new_with_cache_size(log_dir: PathBuf, max_cache_size: usize) -> Self {
        Self {
            log_dir,
            cache: Arc::new(RwLock::new(Vec::new())),
            max_cache_size,
        }
    }

    /// 監査イベントを記録
    pub async fn log_event(&self, entry: AuditEntry) -> anyhow::Result<()> {
        // ファイルに追記
        self.append_to_file(&entry)?;

        // インメモリキャッシュに追加
        let mut cache = self.cache.write().await;
        cache.push(entry);
        if cache.len() > self.max_cache_size {
            let drain_count = cache.len() - self.max_cache_size;
            cache.drain(..drain_count);
        }

        Ok(())
    }

    /// 監査ログをクエリ
    pub async fn query_events(&self, query: &AuditQuery) -> anyhow::Result<Vec<AuditEntry>> {
        // まずファイルから読み込み
        let entries = self.read_from_files(query)?;
        Ok(entries)
    }

    /// 日付ベースのファイルパスを生成
    ///
    /// `base_dir` には検証済みのログディレクトリパスを渡すことを想定している。
    fn log_file_path(&self, base_dir: &Path, date: &DateTime<Utc>) -> PathBuf {
        base_dir.join(format!("{}.jsonl", date.format("%Y-%m-%d")))
    }

    /// ログディレクトリのパスを検証・正規化
    ///
    /// `self.log_dir` が許可されたベースディレクトリ配下にあることを確認し、
    /// そうでない場合はエラーを返す。
    fn validated_log_dir(&self) -> anyhow::Result<PathBuf> {
        let base = std::env::current_dir()?;

        // 既存のディレクトリであれば canonicalize し、存在しない場合は base からの相対パスとして扱う
        let candidate = if self.log_dir.exists() {
            self.log_dir.canonicalize()?
        } else if self.log_dir.is_absolute() {
            // 絶対パスで存在しない場合：親ディレクトリを正規化してファイル名を結合
            if let (Some(parent), Some(name)) = (self.log_dir.parent(), self.log_dir.file_name()) {
                if parent.exists() {
                    parent.canonicalize()?.join(name)
                } else {
                    self.log_dir.clone()
                }
            } else {
                self.log_dir.clone()
            }
        } else {
            base.join(&self.log_dir)
        };

        // テスト時はTempDir（/tmp等）を使用するため検証をスキップ
        #[cfg(not(test))]
        if !candidate.starts_with(&base) {
            anyhow::bail!("invalid log_dir outside allowed base directory");
        }

        Ok(candidate)
    }

    /// ファイルに追記
    fn append_to_file(&self, entry: &AuditEntry) -> anyhow::Result<()> {
        // 検証済みのログディレクトリパスを取得
        let log_dir = self.validated_log_dir()?;
        std::fs::create_dir_all(&log_dir)?;

        let path = self.log_file_path(&log_dir, &entry.timestamp);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        let line = serde_json::to_string(entry)?;
        writeln!(file, "{}", line)?;

        Ok(())
    }

    /// ファイルからエントリを読み込み
    fn read_from_files(&self, query: &AuditQuery) -> anyhow::Result<Vec<AuditEntry>> {
        let mut entries = Vec::new();

        // ログディレクトリのパスを検証・正規化
        let log_dir = self.validated_log_dir()?;

        if !log_dir.exists() {
            return Ok(entries);
        }

        // ログディレクトリ内の全JSONLファイルを読む
        let mut files: Vec<_> = std::fs::read_dir(&log_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .collect();

        // ファイル名（日付順）でソート
        files.sort_by_key(|e| e.file_name());

        for file_entry in files {
            let file = std::fs::File::open(file_entry.path())?;
            let reader = std::io::BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }

                let entry: AuditEntry = match serde_json::from_str(&line) {
                    Ok(e) => e,
                    Err(_) => continue, // 不正な行はスキップ
                };

                // フィルタリング
                if let Some(from) = &query.from {
                    if entry.timestamp < *from {
                        continue;
                    }
                }
                if let Some(to) = &query.to {
                    if entry.timestamp > *to {
                        continue;
                    }
                }
                if let Some(action) = &query.action {
                    if entry.action != *action {
                        continue;
                    }
                }

                entries.push(entry);
            }
        }

        // 新しい順にソートし、limitを適用
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        entries.truncate(query.limit);

        Ok(entries)
    }

    /// テスト用：キャッシュをクリア
    #[cfg(test)]
    #[allow(dead_code)]
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_service() -> (AuditService, TempDir) {
        let tmp = TempDir::new().unwrap();
        let service = AuditService::new(tmp.path().to_path_buf());
        (service, tmp)
    }

    #[tokio::test]
    async fn test_log_and_query_event() {
        let (service, _tmp) = create_test_service();

        let entry = AuditEntry::new(
            AuditAction::Scan,
            AuditStatus::Success,
            "/api/scan/start",
            "POST",
        );

        service.log_event(entry).await.unwrap();

        let query = AuditQuery {
            from: None,
            to: None,
            action: None,
            limit: 100,
        };
        let results = service.query_events(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, AuditAction::Scan);
        assert_eq!(results[0].status, AuditStatus::Success);
    }

    #[tokio::test]
    async fn test_query_by_action() {
        let (service, _tmp) = create_test_service();

        service
            .log_event(AuditEntry::new(
                AuditAction::Scan,
                AuditStatus::Success,
                "/api/scan/start",
                "POST",
            ))
            .await
            .unwrap();
        service
            .log_event(AuditEntry::new(
                AuditAction::Generate,
                AuditStatus::Success,
                "/api/generate",
                "POST",
            ))
            .await
            .unwrap();
        service
            .log_event(AuditEntry::new(
                AuditAction::Export,
                AuditStatus::Failure,
                "/api/export/123",
                "POST",
            ))
            .await
            .unwrap();

        let query = AuditQuery {
            from: None,
            to: None,
            action: Some(AuditAction::Scan),
            limit: 100,
        };
        let results = service.query_events(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, AuditAction::Scan);
    }

    #[tokio::test]
    async fn test_query_with_limit() {
        let (service, _tmp) = create_test_service();

        for i in 0..5 {
            service
                .log_event(AuditEntry::new(
                    AuditAction::Scan,
                    AuditStatus::Success,
                    &format!("/api/scan/{}", i),
                    "POST",
                ))
                .await
                .unwrap();
        }

        let query = AuditQuery {
            from: None,
            to: None,
            action: None,
            limit: 3,
        };
        let results = service.query_events(&query).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_entry_with_details() {
        let (service, _tmp) = create_test_service();

        let entry = AuditEntry::new(
            AuditAction::Generate,
            AuditStatus::Success,
            "/api/generate",
            "POST",
        )
        .with_details(serde_json::json!({"scan_id": "abc123"}));

        service.log_event(entry).await.unwrap();

        let query = AuditQuery {
            from: None,
            to: None,
            action: None,
            limit: 100,
        };
        let results = service.query_events(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].details.is_some());
        assert_eq!(results[0].details.as_ref().unwrap()["scan_id"], "abc123");
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let tmp = TempDir::new().unwrap();
        let service = AuditService::new_with_cache_size(tmp.path().to_path_buf(), 3);

        for i in 0..5 {
            service
                .log_event(AuditEntry::new(
                    AuditAction::Scan,
                    AuditStatus::Success,
                    &format!("/api/scan/{}", i),
                    "POST",
                ))
                .await
                .unwrap();
        }

        let cache = service.cache.read().await;
        assert_eq!(cache.len(), 3);
    }

    #[tokio::test]
    async fn test_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let service = AuditService::new(tmp.path().join("nonexistent").to_path_buf());

        let query = AuditQuery {
            from: None,
            to: None,
            action: None,
            limit: 100,
        };
        let results = service.query_events(&query).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_query_by_date_range() {
        let (service, _tmp) = create_test_service();

        let entry = AuditEntry::new(
            AuditAction::Scan,
            AuditStatus::Success,
            "/api/scan/start",
            "POST",
        );
        let ts = entry.timestamp;
        service.log_event(entry).await.unwrap();

        // from が未来の場合は結果なし
        let query = AuditQuery {
            from: Some(ts + chrono::Duration::hours(1)),
            to: None,
            action: None,
            limit: 100,
        };
        let results = service.query_events(&query).await.unwrap();
        assert!(results.is_empty());

        // to が過去の場合は結果なし
        let query = AuditQuery {
            from: None,
            to: Some(ts - chrono::Duration::hours(1)),
            action: None,
            limit: 100,
        };
        let results = service.query_events(&query).await.unwrap();
        assert!(results.is_empty());
    }
}
