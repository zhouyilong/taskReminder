use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use base64::Engine;
use chrono::{Local, NaiveDateTime};
use reqwest::StatusCode;
use rusqlite::{params_from_iter, types::Value, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;

use crate::db::DbManager;
use crate::errors::AppError;
use crate::models::{AppSettings, SyncStatus};

const REMOTE_DB_NAME: &str = "taskreminder.db";
const LOCK_FILE_NAME: &str = "taskreminder.lock";
const LOCK_TTL_SECONDS: i64 = 120;

// 自动同步策略：本地变更后延迟同步（debounce）+ 最小同步间隔（throttle）。
const LOCAL_CHANGE_DEBOUNCE_SECONDS: u64 = 5 * 60;
const MIN_AUTOMATIC_SYNC_INTERVAL_SECONDS: i64 = 15 * 60;
const STARTUP_SYNC_DELAY_SECONDS: u64 = 15;

const TASK_COLUMNS: &[&str] = &[
    "id",
    "description",
    "type",
    "status",
    "created_at",
    "completed_at",
    "reminder_time",
    "updated_at",
    "deleted_at",
];
const RECURRING_COLUMNS: &[&str] = &[
    "id",
    "description",
    "type",
    "status",
    "created_at",
    "completed_at",
    "interval_minutes",
    "last_triggered",
    "next_trigger",
    "is_paused",
    "start_time",
    "end_time",
    "repeat_mode",
    "schedule_time",
    "schedule_weekday",
    "schedule_day",
    "cron_expression",
    "updated_at",
    "deleted_at",
];
const RECORD_COLUMNS: &[&str] = &[
    "id",
    "reminder_id",
    "description",
    "type",
    "trigger_time",
    "close_time",
    "action",
    "updated_at",
    "deleted_at",
];

#[derive(Clone)]
pub struct CloudSyncService {
    app: AppHandle,
    db: DbManager,
    sync_in_progress: Arc<AtomicBool>,
    scheduled: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    pending: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    next_auto_sync_due: Arc<Mutex<Option<NaiveDateTime>>>,
    local_change_seq: Arc<AtomicU64>,
    dirty: Arc<AtomicBool>,
}

impl CloudSyncService {
    pub fn new(app: AppHandle, db: DbManager) -> Self {
        Self {
            app,
            db,
            sync_in_progress: Arc::new(AtomicBool::new(false)),
            scheduled: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(None)),
            next_auto_sync_due: Arc::new(Mutex::new(None)),
            local_change_seq: Arc::new(AtomicU64::new(0)),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) -> Result<(), AppError> {
        self.refresh_dirty_from_settings()?;
        self.schedule_if_needed()?;
        self.schedule_startup_sync_if_needed()?;
        Ok(())
    }

    pub fn stop(&self) {
        if let Some(handle) = self.scheduled.lock().unwrap().take() {
            handle.abort();
        }
        if let Some(handle) = self.pending.lock().unwrap().take() {
            handle.abort();
        }
        *self.next_auto_sync_due.lock().unwrap() = None;
    }

    pub fn update_settings(&self) -> Result<(), AppError> {
        self.refresh_dirty_from_settings()?;
        self.schedule_if_needed()?;
        self.schedule_startup_sync_if_needed()?;
        Ok(())
    }

    pub fn request_sync(&self, reason: &str) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        if self
            .sync_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        let service = self.clone();
        let reason = reason.to_string();
        let sync_start_seq = service.local_change_seq.load(Ordering::SeqCst);
        tauri::async_runtime::spawn(async move {
            let outcome = service.perform_sync(&reason);
            if matches!(outcome, Ok(SyncOutcome::Success)) {
                let current_seq = service.local_change_seq.load(Ordering::SeqCst);
                if current_seq == sync_start_seq {
                    service.dirty.store(false, Ordering::SeqCst);
                } else {
                    // 同步过程中出现新的本地变更，继续安排下一次自动同步。
                    let _ = service.schedule_debounced_sync();
                }
            }
            service.sync_in_progress.store(false, Ordering::SeqCst);
        });
        Ok(())
    }

    pub fn notify_local_change(&self) -> Result<(), AppError> {
        self.db.mark_local_change()?;
        self.local_change_seq.fetch_add(1, Ordering::SeqCst);
        self.dirty.store(true, Ordering::SeqCst);
        self.schedule_debounced_sync()?;
        Ok(())
    }

    pub fn get_status(&self) -> Result<SyncStatus, AppError> {
        let settings = self.db.load_settings()?;
        Ok(SyncStatus {
            status: settings
                .webdav_last_sync_status
                .unwrap_or_else(|| "未同步".to_string()),
            error: settings.webdav_last_sync_error,
            time: settings.webdav_last_sync_time,
        })
    }

    fn schedule_if_needed(&self) -> Result<(), AppError> {
        if let Some(handle) = self.scheduled.lock().unwrap().take() {
            handle.abort();
        }
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        let interval = settings.webdav_sync_interval_minutes.max(1) as u64;
        let service = self.clone();
        let handle = tauri::async_runtime::spawn(async move {
            loop {
                sleep(std::time::Duration::from_secs(interval * 60)).await;
                let _ = service.request_sync_on_interval();
            }
        });
        *self.scheduled.lock().unwrap() = Some(handle);
        Ok(())
    }

    fn schedule_startup_sync_if_needed(&self) -> Result<(), AppError> {
        if !self.dirty.load(Ordering::SeqCst) {
            return Ok(());
        }
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        let now = Local::now().naive_local();
        let mut due = now + chrono::Duration::seconds(STARTUP_SYNC_DELAY_SECONDS as i64);
        if let Some(throttle_due) = next_allowed_auto_sync_time(&settings, now) {
            if throttle_due > due {
                due = throttle_due;
            }
        }
        self.schedule_auto_sync_at(due, "startup")?;
        Ok(())
    }

    fn schedule_debounced_sync(&self) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        let now = Local::now().naive_local();
        let mut due = now + chrono::Duration::seconds(LOCAL_CHANGE_DEBOUNCE_SECONDS as i64);
        if let Some(throttle_due) = next_allowed_auto_sync_time(&settings, now) {
            if throttle_due > due {
                due = throttle_due;
            }
        }
        self.schedule_auto_sync_at(due, "debounce")?;
        Ok(())
    }

    fn schedule_auto_sync_at(&self, due: NaiveDateTime, reason: &str) -> Result<(), AppError> {
        let now = Local::now().naive_local();
        if due <= now {
            return self.request_sync_if_needed(reason);
        }

        {
            let mut next_due = self.next_auto_sync_due.lock().unwrap();
            if let Some(existing) = *next_due {
                // 已有更“晚”的同步计划（通常是新的本地变更触发），避免被更早的计划覆盖。
                if existing >= due {
                    return Ok(());
                }
            }
            *next_due = Some(due);
        }

        if let Some(handle) = self.pending.lock().unwrap().take() {
            handle.abort();
        }

        let delay = (due - now)
            .to_std()
            .unwrap_or_else(|_| std::time::Duration::from_secs(0));
        let service = self.clone();
        let reason = reason.to_string();
        let handle = tauri::async_runtime::spawn(async move {
            sleep(delay).await;
            *service.next_auto_sync_due.lock().unwrap() = None;
            let _ = service.request_sync_if_needed(&reason);
        });
        *self.pending.lock().unwrap() = Some(handle);
        Ok(())
    }

    fn request_sync_on_interval(&self) -> Result<(), AppError> {
        let now = Local::now().naive_local();
        if let Some(due) = *self.next_auto_sync_due.lock().unwrap() {
            let remaining = due - now;
            // 如果 debounce 很快就会触发，就避免 interval “抢跑”；否则 interval 作为兜底依然可以触发同步。
            if remaining.num_seconds() > 0 && remaining.num_seconds() <= 30 {
                return Ok(());
            }
        }
        self.request_sync_if_needed("interval")
    }

    fn request_sync_if_needed(&self, reason: &str) -> Result<(), AppError> {
        if !self.dirty.load(Ordering::SeqCst) {
            return Ok(());
        }
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }

        // throttle：距离上一次同步太近则延后。
        let now = Local::now().naive_local();
        if let Some(throttle_due) = next_allowed_auto_sync_time(&settings, now) {
            self.schedule_auto_sync_at(throttle_due, "throttle")?;
            return Ok(());
        }

        self.request_sync(reason)
    }

    fn refresh_dirty_from_settings(&self) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        let dirty = compute_dirty_from_settings(&settings);
        self.dirty.store(dirty, Ordering::SeqCst);
        Ok(())
    }

    fn perform_sync(&self, _reason: &str) -> Result<SyncOutcome, AppError> {
        let settings = self.db.load_settings()?;
        let client = WebDavClient::new(&settings)?;
        let lock = LockInfo::new(&settings.webdav_device_id);

        let _ = self.update_sync_status("同步中", None);
        let mut remote: Option<std::path::PathBuf> = None;
        let mut snapshot: Option<std::path::PathBuf> = None;
        let mut lock_acquired = false;

        let result = (|| -> Result<SyncOutcome, AppError> {
            lock_acquired = client.try_acquire_lock(&lock)?;
            if !lock_acquired {
                let _ = self.update_sync_status("锁被占用，稍后重试", None);
                return Ok(SyncOutcome::Skipped);
            }

            if !client.exists(REMOTE_DB_NAME)? {
                let local_snapshot = export_local_snapshot(&self.db.db_path())?;
                client.upload(REMOTE_DB_NAME, &local_snapshot)?;
                snapshot = Some(local_snapshot);
                let _ = self.update_sync_status("首次同步完成", None);
                return Ok(SyncOutcome::Success);
            }

            let downloaded = download_remote(&client)?;
            remote = Some(downloaded.clone());
            merge_databases(&self.db.db_path(), &downloaded)?;
            let local_snapshot = export_local_snapshot(&self.db.db_path())?;
            client.upload(REMOTE_DB_NAME, &local_snapshot)?;
            snapshot = Some(local_snapshot);
            let _ = self.update_sync_status("同步成功", None);
            let _ = self.app.emit("data-updated", ());
            Ok(SyncOutcome::Success)
        })();

        let outcome = match result {
            Ok(outcome) => outcome,
            Err(err) => {
                let _ = self.update_sync_status("同步失败", Some(err.to_string()));
                SyncOutcome::Failed
            }
        };

        if lock_acquired {
            client.release_lock();
        }
        cleanup_temp_file(&remote);
        cleanup_temp_file(&snapshot);
        Ok(outcome)
    }

    fn update_sync_status(&self, status: &str, error: Option<String>) -> Result<(), AppError> {
        let settings = self.db.update_sync_status(status, error)?;
        let payload = SyncStatus {
            status: settings
                .webdav_last_sync_status
                .unwrap_or_else(|| status.to_string()),
            error: settings.webdav_last_sync_error.clone(),
            time: settings.webdav_last_sync_time.clone(),
        };
        let _ = self.app.emit("sync-status", payload);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncOutcome {
    Success,
    Skipped,
    Failed,
}

fn is_success_status(status: &Option<String>) -> bool {
    matches!(status.as_deref(), Some("同步成功") | Some("首次同步完成"))
}

fn compute_dirty_from_settings(settings: &AppSettings) -> bool {
    let Some(local_change) = settings.webdav_last_local_change_time.as_deref() else {
        return false;
    };

    // 没有成功同步过，或者上一次处于失败/进行中状态：视为还有未同步变更。
    if !is_success_status(&settings.webdav_last_sync_status) {
        return true;
    }

    let Some(last_sync) = settings.webdav_last_sync_time.as_deref() else {
        return true;
    };

    match (
        parse_datetime_any(local_change),
        parse_datetime_any(last_sync),
    ) {
        (Some(local_dt), Some(sync_dt)) => local_dt > sync_dt,
        _ => true,
    }
}

fn next_allowed_auto_sync_time(
    settings: &AppSettings,
    now: NaiveDateTime,
) -> Option<NaiveDateTime> {
    let last_sync = settings
        .webdav_last_sync_time
        .as_deref()
        .and_then(parse_datetime_any)?;
    let next_allowed = last_sync + chrono::Duration::seconds(MIN_AUTOMATIC_SYNC_INTERVAL_SECONDS);
    if next_allowed > now {
        Some(next_allowed)
    } else {
        None
    }
}

pub fn test_webdav(settings: &AppSettings) -> Result<(bool, String), AppError> {
    let client = WebDavClient::new(settings)?;
    client.test_connection()
}

fn export_local_snapshot(db_path: &std::path::Path) -> Result<std::path::PathBuf, AppError> {
    let snapshot =
        std::env::temp_dir().join(format!("taskreminder-snapshot-{}.db", uuid::Uuid::new_v4()));
    let conn = Connection::open(db_path)?;
    let escaped = snapshot.to_string_lossy().replace('"', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}'", escaped))?;
    Ok(snapshot)
}

fn download_remote(client: &WebDavClient) -> Result<std::path::PathBuf, AppError> {
    let target =
        std::env::temp_dir().join(format!("taskreminder-remote-{}.db", uuid::Uuid::new_v4()));
    client.download(REMOTE_DB_NAME, &target)?;
    Ok(target)
}

fn merge_databases(
    local_path: &std::path::Path,
    remote_path: &std::path::Path,
) -> Result<(), AppError> {
    let mut local = Connection::open(local_path)?;
    let remote = Connection::open(remote_path)?;
    ensure_sync_columns(&local)?;
    ensure_sync_columns(&remote)?;

    let tx = local.transaction()?;
    merge_table(&tx, &remote, "tasks", TASK_COLUMNS, "created_at")?;
    merge_table(
        &tx,
        &remote,
        "recurring_tasks",
        RECURRING_COLUMNS,
        "created_at",
    )?;
    merge_table(
        &tx,
        &remote,
        "reminder_records",
        RECORD_COLUMNS,
        "trigger_time",
    )?;
    tx.commit()?;
    Ok(())
}

fn merge_table(
    local: &Connection,
    remote: &Connection,
    table: &str,
    columns: &[&str],
    fallback_time_column: &str,
) -> Result<(), AppError> {
    let local_rows = load_rows(local, table, columns, fallback_time_column)?;
    let remote_rows = load_rows(remote, table, columns, fallback_time_column)?;
    let mut all_ids = local_rows.keys().cloned().collect::<Vec<_>>();
    for id in remote_rows.keys() {
        if !all_ids.contains(id) {
            all_ids.push(id.clone());
        }
    }
    if all_ids.is_empty() {
        return Ok(());
    }

    let placeholders = vec!["?"; columns.len()].join(", ");
    let sql = format!(
        "REPLACE INTO {} ({}) VALUES ({})",
        table,
        columns.join(", "),
        placeholders
    );

    let mut stmt = local.prepare(&sql)?;
    for id in all_ids {
        let row = choose_row(local_rows.get(&id), remote_rows.get(&id));
        if let Some(row) = row {
            let params = params_from_iter(row.values.iter());
            stmt.execute(params)?;
        }
    }
    Ok(())
}

fn choose_row<'a>(local: Option<&'a RowData>, remote: Option<&'a RowData>) -> Option<&'a RowData> {
    match (local, remote) {
        (Some(l), None) => Some(l),
        (None, Some(r)) => Some(r),
        (Some(l), Some(r)) => match (l.compare_time, r.compare_time) {
            (Some(lc), Some(rc)) => {
                if rc > lc {
                    Some(r)
                } else {
                    Some(l)
                }
            }
            (Some(_), None) => Some(l),
            (None, Some(_)) => Some(r),
            _ => Some(l),
        },
        _ => None,
    }
}

#[derive(Clone)]
struct RowData {
    values: Vec<Value>,
    compare_time: Option<NaiveDateTime>,
}

fn load_rows(
    conn: &Connection,
    table: &str,
    columns: &[&str],
    fallback_time_column: &str,
) -> Result<HashMap<String, RowData>, AppError> {
    let sql = format!("SELECT {} FROM {}", columns.join(", "), table);
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;

    let id_index = columns.iter().position(|c| *c == "id").unwrap_or(0);
    let updated_index = columns.iter().position(|c| *c == "updated_at");
    let deleted_index = columns.iter().position(|c| *c == "deleted_at");
    let fallback_index = columns.iter().position(|c| *c == fallback_time_column);

    let mut map = HashMap::new();
    while let Some(row) = rows.next()? {
        let mut values = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            let value: Value = row.get(i)?;
            values.push(value);
        }
        let id_value = value_to_string(&values[id_index]);
        if let Some(id) = id_value {
            let updated = updated_index.and_then(|idx| value_to_string(&values[idx]));
            let deleted = deleted_index.and_then(|idx| value_to_string(&values[idx]));
            let fallback = fallback_index.and_then(|idx| value_to_string(&values[idx]));
            let normalized =
                normalize_compare_time(updated.clone(), deleted.clone(), fallback.clone());
            if let (Some(idx), Some(value)) = (updated_index, normalized.clone()) {
                values[idx] = Value::Text(value);
            }
            let compare = normalized.and_then(|value| parse_datetime_any(&value));
            map.insert(
                id,
                RowData {
                    values,
                    compare_time: compare,
                },
            );
        }
    }
    Ok(map)
}

fn normalize_compare_time(
    updated: Option<String>,
    deleted: Option<String>,
    fallback: Option<String>,
) -> Option<String> {
    if let Some(u) = updated {
        if !u.is_empty() {
            return Some(u);
        }
    }
    if let Some(d) = deleted {
        if !d.is_empty() {
            return Some(d);
        }
    }
    fallback
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(text.clone()),
        Value::Integer(num) => Some(num.to_string()),
        Value::Real(num) => Some(num.to_string()),
        _ => None,
    }
}

fn parse_datetime_any(value: &str) -> Option<NaiveDateTime> {
    let candidates = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ];
    for fmt in candidates {
        if let Ok(dt) = NaiveDateTime::parse_from_str(value, fmt) {
            return Some(dt);
        }
    }
    None
}

fn ensure_sync_columns(conn: &Connection) -> Result<(), AppError> {
    ensure_column(conn, "tasks", "updated_at", "TEXT")?;
    ensure_column(conn, "tasks", "deleted_at", "TEXT")?;
    ensure_column(conn, "recurring_tasks", "updated_at", "TEXT")?;
    ensure_column(conn, "recurring_tasks", "deleted_at", "TEXT")?;
    ensure_column(
        conn,
        "recurring_tasks",
        "repeat_mode",
        "TEXT NOT NULL DEFAULT 'INTERVAL_RANGE'",
    )?;
    ensure_column(conn, "recurring_tasks", "schedule_time", "TEXT")?;
    ensure_column(conn, "recurring_tasks", "schedule_weekday", "INTEGER")?;
    ensure_column(conn, "recurring_tasks", "schedule_day", "INTEGER")?;
    ensure_column(conn, "recurring_tasks", "cron_expression", "TEXT")?;
    ensure_column(conn, "reminder_records", "updated_at", "TEXT")?;
    ensure_column(conn, "reminder_records", "deleted_at", "TEXT")?;
    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    column_type: &str,
) -> Result<(), AppError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get("name")?;
        if name.eq_ignore_ascii_case(column) {
            return Ok(());
        }
    }
    conn.execute(
        &format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            table, column, column_type
        ),
        [],
    )?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone)]
struct LockInfo {
    #[serde(rename = "deviceId", alias = "device_id")]
    device_id: String,
    #[serde(rename = "expiresAt", alias = "expires_at")]
    expires_at: i64,
}

impl LockInfo {
    fn new(device_id: &str) -> Self {
        let expires_at = chrono::Utc::now().timestamp_millis() + LOCK_TTL_SECONDS * 1000;
        Self {
            device_id: device_id.to_string(),
            expires_at,
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at_millis() <= chrono::Utc::now().timestamp_millis()
    }

    fn expires_at_millis(&self) -> i64 {
        if self.expires_at > 1_000_000_000_000 {
            self.expires_at
        } else {
            self.expires_at * 1000
        }
    }
}

struct WebDavClient {
    base_url: String,
    auth_header: Option<String>,
    client: reqwest::blocking::Client,
}

impl WebDavClient {
    fn new(settings: &AppSettings) -> Result<Self, AppError> {
        let base_url = build_base_url(&settings.webdav_url, &settings.webdav_root_path);
        let auth_header = build_auth_header(&settings.webdav_username, &settings.webdav_password);
        Ok(Self {
            base_url,
            auth_header,
            client: reqwest::blocking::Client::builder()
                .build()
                .map_err(|e| AppError::Sync(e.to_string()))?,
        })
    }

    fn test_connection(&self) -> Result<(bool, String), AppError> {
        let mut req = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                &self.base_url,
            )
            .header("Depth", "0");
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        let status = resp.status();
        if status == StatusCode::MULTI_STATUS || status == StatusCode::OK {
            return Ok((true, "连接成功".to_string()));
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Ok((false, "认证失败".to_string()));
        }
        Ok((false, format!("连接失败，状态码: {}", status)))
    }

    fn exists(&self, name: &str) -> Result<bool, AppError> {
        let url = build_url(&self.base_url, name);
        let mut req = self.client.head(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        Ok(resp.status() == StatusCode::OK)
    }

    fn download(&self, name: &str, target: &std::path::Path) -> Result<(), AppError> {
        let url = build_url(&self.base_url, name);
        let mut req = self.client.get(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Sync(format!(
                "下载失败，状态码: {}",
                resp.status()
            )));
        }
        let bytes = resp.bytes().map_err(|e| AppError::Sync(e.to_string()))?;
        std::fs::write(target, bytes)?;
        Ok(())
    }

    fn upload(&self, name: &str, source: &std::path::Path) -> Result<(), AppError> {
        let url = build_url(&self.base_url, name);
        let data = std::fs::read(source)?;
        let mut req = self
            .client
            .put(url)
            .body(data)
            .header("Content-Type", "application/octet-stream");
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Sync(format!(
                "上传失败，状态码: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn try_acquire_lock(&self, info: &LockInfo) -> Result<bool, AppError> {
        if let Some(existing) = self.get_lock()? {
            if !existing.is_expired() && existing.device_id != info.device_id {
                return Ok(false);
            }
        }
        let url = build_url(&self.base_url, LOCK_FILE_NAME);
        let body = serde_json::to_vec(info).map_err(|e| AppError::Sync(e.to_string()))?;
        let mut req = self
            .client
            .put(url)
            .body(body)
            .header("Content-Type", "application/json");
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Sync(format!(
                "写入锁失败，状态码: {}",
                resp.status()
            )));
        }
        Ok(true)
    }

    fn get_lock(&self) -> Result<Option<LockInfo>, AppError> {
        let url = build_url(&self.base_url, LOCK_FILE_NAME);
        let mut req = self.client.get(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = match req.send() {
            Ok(resp) => resp,
            Err(_) => return Ok(None),
        };
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Ok(None);
        }
        let bytes = match resp.bytes() {
            Ok(bytes) => bytes,
            Err(_) => return Ok(None),
        };
        let lock: LockInfo = match serde_json::from_slice(&bytes) {
            Ok(lock) => lock,
            Err(_) => return Ok(None),
        };
        Ok(Some(lock))
    }

    fn release_lock(&self) {
        let url = build_url(&self.base_url, LOCK_FILE_NAME);
        let mut req = self.client.delete(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let _ = req.send();
    }
}

fn build_base_url(url: &str, root: &str) -> String {
    let mut base = url.trim().trim_end_matches('/').to_string();
    let mut root = root.trim().to_string();
    if !root.is_empty() {
        if !root.starts_with('/') {
            root = format!("/{}", root);
        }
        root = root.trim_end_matches('/').to_string();
        base.push_str(&root);
    }
    base
}

fn build_url(base: &str, name: &str) -> String {
    if name.is_empty() {
        return base.to_string();
    }
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        name.trim_start_matches('/')
    )
}

fn build_auth_header(username: &str, password: &str) -> Option<String> {
    if username.trim().is_empty() {
        return None;
    }
    let token =
        base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", username, password));
    Some(format!("Basic {}", token))
}

fn cleanup_temp_file(path: &Option<std::path::PathBuf>) {
    if let Some(path) = path {
        let _ = std::fs::remove_file(path);
    }
}
