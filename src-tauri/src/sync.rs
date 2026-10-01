use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use base64::Engine;
use chrono::NaiveDateTime;
use reqwest::StatusCode;
use rusqlite::{params_from_iter, types::Value, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;

use crate::db::{DbManager, TOMBSTONE_RETENTION_DAYS_SYNC};
use crate::errors::AppError;
use crate::models::{AppSettings, SyncStatus};
use crate::sync_crypto::{self, KdfParams};
use crate::sync_schema::{
    adopt_remote_columns, ensure_sync_columns, quote_ident, table_columns, SyncTable, SYNC_TABLES,
};
use crate::time::{self, parse_datetime_any};

const REMOTE_DB_NAME: &str = "taskreminder.db";
/// 开启端到端加密后的远端文件；`REMOTE_DB_NAME` 处改放占位说明，旧版本读到后同步失败而不会上传明文。
const REMOTE_ENC_NAME: &str = "taskreminder.db.enc";
const PLACEHOLDER_MARKER: &[u8] = b"TaskReminder-Encrypted-Placeholder";
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
const LOCK_FILE_NAME: &str = "taskreminder.lock";
const LOCK_TTL_SECONDS: i64 = 120;

// 自动同步策略：本地变更后延迟同步（debounce）+ 最小同步间隔（throttle）。
const LOCAL_CHANGE_DEBOUNCE_SECONDS: u64 = 5 * 60;
const MIN_AUTOMATIC_SYNC_INTERVAL_SECONDS: i64 = 15 * 60;
const STARTUP_SYNC_DELAY_SECONDS: u64 = 15;

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
        // perform_sync 用 reqwest::blocking 并做 Argon2 派生，必须放在阻塞线程池：在异步任务里
        // 会占住运行时的工作线程，debug 构建中 reqwest 还会直接 panic（同步从未执行）。
        tauri::async_runtime::spawn_blocking(move || {
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
        let _ = self.app.emit("data-updated", ());
        self.schedule_debounced_sync()?;
        Ok(())
    }

    pub fn get_status(&self) -> Result<SyncStatus, AppError> {
        let settings = self.db.load_settings()?;
        Ok(SyncStatus {
            status: status_code(settings.webdav_last_sync_status.as_deref()),
            error: settings.webdav_last_sync_error,
            time: settings.webdav_last_sync_time,
        })
    }

    /// 更换同步密码：与普通同步互斥（同一个进行中标记与远端锁），用已保存的连接设置执行
    /// `change_passphrase_on_remote`。阻塞调用（网络与 Argon2），命令中放到后台线程执行。
    pub fn change_passphrase(&self, current: &str, new: &str) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Err(AppError::Invalid(
                "请先启用并保存 WebDAV 同步设置".to_string(),
            ));
        }
        validate_passphrase_change(&settings, current, new)?;
        if self
            .sync_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AppError::Invalid("正在同步，请稍后再试".to_string()));
        }

        let _ = self.update_sync_status(SyncState::Syncing, None);
        let mut lock_busy = false;
        let result = (|| -> Result<(), AppError> {
            let client = WebDavClient::new(&settings)?;
            let lock = LockInfo::new(&settings.webdav_device_id);
            if !client.try_acquire_lock(&lock)? {
                lock_busy = true;
                return Err(AppError::Sync("其他设备正在同步，请稍后再试".to_string()));
            }
            let result =
                change_passphrase_on_remote(&client, &self.db, &settings, new, KdfParams::DEFAULT);
            client.release_lock();
            result.map(|_| ())
        })();

        match &result {
            Ok(()) => {
                let _ = self.update_sync_status(SyncState::Success, None);
                let _ = self.refresh_dirty_from_settings();
                let _ = self.app.emit("data-updated", ());
            }
            Err(_) if lock_busy => {
                let _ = self.update_sync_status(SyncState::LockBusy, None);
            }
            Err(err) => {
                let _ = self.update_sync_status(failure_state(err), Some(err.to_string()));
            }
        }
        self.sync_in_progress.store(false, Ordering::SeqCst);
        result
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
        let now = time::now();
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
        let now = time::now();
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
        let now = time::now();
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
        let now = time::now();
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
        // 手动同步（`request_sync`）不受影响；修改同步密码后由 `save_settings` 立即触发一次。
        if auto_sync_paused(&settings) {
            return Ok(());
        }

        // throttle：距离上一次同步太近则延后。
        let now = time::now();
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

        let _ = self.update_sync_status(SyncState::Syncing, None);
        let mut lock_acquired = false;

        let result = (|| -> Result<SyncOutcome, AppError> {
            lock_acquired = client.try_acquire_lock(&lock)?;
            if !lock_acquired {
                let _ = self.update_sync_status(SyncState::LockBusy, None);
                return Ok(SyncOutcome::Skipped);
            }

            let result = sync_with_remote(&client, &self.db, &settings, KdfParams::DEFAULT)?;
            match result {
                RemoteSyncResult::FirstUpload => {
                    let _ = self.update_sync_status(SyncState::FirstSync, None);
                }
                RemoteSyncResult::Merged => {
                    let _ = self.update_sync_status(SyncState::Success, None);
                    let _ = self.app.emit("data-updated", ());
                }
            }
            Ok(SyncOutcome::Success)
        })();

        let outcome = match result {
            Ok(outcome) => outcome,
            Err(err) => {
                let _ = self.update_sync_status(failure_state(&err), Some(err.to_string()));
                SyncOutcome::Failed
            }
        };

        if lock_acquired {
            client.release_lock();
        }
        Ok(outcome)
    }

    fn update_sync_status(&self, state: SyncState, error: Option<String>) -> Result<(), AppError> {
        let settings = self.db.update_sync_status(state.code(), error)?;
        let payload = SyncStatus {
            status: status_code(settings.webdav_last_sync_status.as_deref()),
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

/// 同步状态码：存库与发给前端的都是状态码，文案只在前端（`src/syncStatus.ts`）映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    Never,
    Syncing,
    Success,
    FirstSync,
    LockBusy,
    Failed,
    /// 远端加密数据无法用本机的同步密码解密，通常是在其他设备上更换了同步密码。
    /// 此状态下暂停自动同步，直到修改同步密码或手动同步。
    PassphraseMismatch,
}

impl SyncState {
    pub fn code(self) -> &'static str {
        match self {
            SyncState::Never => "never",
            SyncState::Syncing => "syncing",
            SyncState::Success => "success",
            SyncState::FirstSync => "first_sync",
            SyncState::LockBusy => "lock_busy",
            SyncState::Failed => "failed",
            SyncState::PassphraseMismatch => "passphrase_mismatch",
        }
    }

    /// 解析存库的状态；兼容 2.0.1 之前直接存中文文案的旧值。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "" | "never" | "未同步" => Some(SyncState::Never),
            "syncing" | "同步中" => Some(SyncState::Syncing),
            "success" | "同步成功" => Some(SyncState::Success),
            "first_sync" | "首次同步完成" => Some(SyncState::FirstSync),
            "lock_busy" | "锁被占用，稍后重试" => Some(SyncState::LockBusy),
            "failed" | "同步失败" => Some(SyncState::Failed),
            "passphrase_mismatch" => Some(SyncState::PassphraseMismatch),
            _ => None,
        }
    }

    fn is_success(self) -> bool {
        matches!(self, SyncState::Success | SyncState::FirstSync)
    }
}

/// 把存库的状态转成状态码；无法识别的值原样返回，前端按原文显示。
fn status_code(stored: Option<&str>) -> String {
    let raw = stored.unwrap_or("");
    SyncState::parse(raw)
        .map(|state| state.code().to_string())
        .unwrap_or_else(|| raw.to_string())
}

fn failure_state(err: &AppError) -> SyncState {
    match err {
        AppError::SyncPassphrase(_) => SyncState::PassphraseMismatch,
        _ => SyncState::Failed,
    }
}

/// 同步密码不匹配时暂停自动同步：每次都会失败，还要白白做一次 Argon2 派生。
fn auto_sync_paused(settings: &AppSettings) -> bool {
    settings
        .webdav_last_sync_status
        .as_deref()
        .and_then(SyncState::parse)
        == Some(SyncState::PassphraseMismatch)
}

/// 保存设置时，同步密码不匹配状态下改了同步密码：需要立即同步一次以确认并恢复自动同步。
pub fn should_resync_after_passphrase_edit(previous: &AppSettings, saved: &AppSettings) -> bool {
    auto_sync_paused(previous)
        && saved.webdav_enabled
        && saved.sync_encryption_enabled
        && saved.sync_passphrase != previous.sync_passphrase
}

fn is_success_status(status: &Option<String>) -> bool {
    status
        .as_deref()
        .and_then(SyncState::parse)
        .is_some_and(SyncState::is_success)
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
    let (ok, message) = client.test_connection()?;
    if !ok {
        return Ok((ok, message));
    }
    Ok(check_encryption(&client, settings))
}

/// 连接成功后检查加密设置与远端数据是否匹配，给出提示。
fn check_encryption(store: &dyn RemoteStore, settings: &AppSettings) -> (bool, String) {
    let remote_encrypted = match store.exists(REMOTE_ENC_NAME) {
        Ok(value) => value,
        Err(err) => return (false, format!("连接成功，但读取远端数据失败：{}", err)),
    };
    if !settings.sync_encryption_enabled {
        return if remote_encrypted {
            (
                false,
                "连接成功，但远端数据已加密：请开启端到端加密并填写同步密码".to_string(),
            )
        } else {
            (true, "连接成功".to_string())
        };
    }
    if let Err(err) = sync_crypto::validate_passphrase(&settings.sync_passphrase) {
        return (false, format!("连接成功，但{}", err));
    }
    if !remote_encrypted {
        return (true, "连接成功，下次同步时将加密上传".to_string());
    }
    match store
        .get(REMOTE_ENC_NAME)
        .map_err(|e| e.to_string())
        .and_then(|data| {
            sync_crypto::decrypt(&data, &settings.sync_passphrase).map_err(|e| e.to_string())
        }) {
        Ok(_) => (true, "连接成功，同步密码正确".to_string()),
        Err(message) => (false, format!("连接成功，但{}", message)),
    }
}

/// 远端存储的最小接口：WebDAV 客户端实现它，测试中用内存实现替代。
trait RemoteStore {
    fn exists(&self, name: &str) -> Result<bool, AppError>;
    fn get(&self, name: &str) -> Result<Vec<u8>, AppError>;
    fn put(&self, name: &str, data: Vec<u8>) -> Result<(), AppError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteSyncResult {
    /// 远端没有数据，上传了本地快照。
    FirstUpload,
    /// 与远端合并后上传。
    Merged,
}

fn placeholder_content() -> Vec<u8> {
    let mut data = PLACEHOLDER_MARKER.to_vec();
    data.extend_from_slice(
        "\n此目录的任务提醒同步数据已开启端到端加密，保存在 taskreminder.db.enc 中。\n\
         请把所有设备升级到 2.0 及以上版本，并在“云同步设置”中开启加密、填写相同的同步密码。\n"
            .as_bytes(),
    );
    data
}

/// 解密认证失败单独归类：界面据此提示“同步密码已在其他设备更换”，并暂停自动同步。
fn crypto_error(err: sync_crypto::CryptoError) -> AppError {
    match err {
        sync_crypto::CryptoError::Decrypt => AppError::SyncPassphrase(err.to_string()),
        other => AppError::Sync(other.to_string()),
    }
}

/// 加锁之后的同步主体：下载（必要时解密）远端快照并合并，再上传本地快照（必要时加密）。
///
/// - 远端已加密而本机未开启加密：报错，不上传，避免把明文传回服务器。
/// - 同步密码错误：解密失败即报错，不会用另一个密码覆盖远端数据。
/// - 本机开启加密而远端还是明文：合并明文后改为上传加密文件，并把明文文件替换为占位说明。
fn sync_with_remote(
    store: &dyn RemoteStore,
    db: &DbManager,
    settings: &AppSettings,
    params: KdfParams,
) -> Result<RemoteSyncResult, AppError> {
    sync_with_remote_as(store, db, settings, &settings.sync_passphrase, params)
}

/// `sync_with_remote` 的主体：用 `settings.sync_passphrase` 解密远端，用 `upload_passphrase`
/// 加密上传。平时两者相同；更换同步密码时用新密码上传（见 `change_passphrase_on_remote`）。
fn sync_with_remote_as(
    store: &dyn RemoteStore,
    db: &DbManager,
    settings: &AppSettings,
    upload_passphrase: &str,
    params: KdfParams,
) -> Result<RemoteSyncResult, AppError> {
    let encrypt = settings.sync_encryption_enabled;
    if encrypt {
        sync_crypto::validate_passphrase(&settings.sync_passphrase).map_err(crypto_error)?;
        sync_crypto::validate_passphrase(upload_passphrase).map_err(crypto_error)?;
    }

    let remote_snapshot = if store.exists(REMOTE_ENC_NAME)? {
        if !encrypt {
            return Err(AppError::Sync(
                "远端数据已开启端到端加密：请在云同步设置中开启加密并填写同步密码".to_string(),
            ));
        }
        let data = store.get(REMOTE_ENC_NAME)?;
        Some(sync_crypto::decrypt(&data, &settings.sync_passphrase).map_err(crypto_error)?)
    } else if store.exists(REMOTE_DB_NAME)? {
        let data = store.get(REMOTE_DB_NAME)?;
        if data.starts_with(PLACEHOLDER_MARKER) {
            // 加密文件被手动删除，只剩占位说明：按首次同步处理。
            None
        } else if data.starts_with(SQLITE_MAGIC) {
            Some(data)
        } else {
            return Err(AppError::Sync("远端同步文件格式无效".to_string()));
        }
    } else {
        None
    };

    let merged = remote_snapshot.is_some();
    if let Some(snapshot) = remote_snapshot {
        merge_snapshot_bytes(&db.db_path(), &snapshot)?;
        // 合并后再清理过期墓碑，随后上传的快照中也不再包含它们。
        db.purge_expired_tombstones(TOMBSTONE_RETENTION_DAYS_SYNC)?;
    }

    let local_snapshot = export_local_snapshot_bytes(&db.db_path())?;
    if encrypt {
        let data = sync_crypto::encrypt(&local_snapshot, upload_passphrase, params)
            .map_err(crypto_error)?;
        store.put(REMOTE_ENC_NAME, data)?;
        // 先传加密文件再替换明文：中途失败时远端仍有一份可用的数据。
        store.put(REMOTE_DB_NAME, placeholder_content())?;
    } else {
        store.put(REMOTE_DB_NAME, local_snapshot)?;
    }

    Ok(if merged {
        RemoteSyncResult::Merged
    } else {
        RemoteSyncResult::FirstUpload
    })
}

/// 更换同步密码前的检查（不访问网络）：已开启加密、当前密码与本机保存的一致、新密码有效且不同。
fn validate_passphrase_change(
    settings: &AppSettings,
    current: &str,
    new: &str,
) -> Result<(), AppError> {
    if !settings.sync_encryption_enabled {
        return Err(AppError::Invalid(
            "请先开启端到端加密并保存设置".to_string(),
        ));
    }
    if current != settings.sync_passphrase {
        return Err(AppError::Invalid("当前同步密码不正确".to_string()));
    }
    sync_crypto::validate_passphrase(new).map_err(|e| AppError::Invalid(e.to_string()))?;
    if new == current {
        return Err(AppError::Invalid("新密码与当前密码相同".to_string()));
    }
    Ok(())
}

/// 更换同步密码（需已持有同步锁）：用当前密码下载、解密并合并远端数据，再用新密码加密上传；
/// **上传成功后**才把新密码保存到本机。任一步失败都保留旧密码：
/// - 当前密码解密失败：不上传，远端不变；
/// - 上传失败：本机仍是旧密码，远端的 `.enc` 要么没变、要么已是新密码（此时本机同步会提示
///   密码不匹配，填入新密码即可）。
fn change_passphrase_on_remote(
    store: &dyn RemoteStore,
    db: &DbManager,
    settings: &AppSettings,
    new_passphrase: &str,
    params: KdfParams,
) -> Result<RemoteSyncResult, AppError> {
    validate_passphrase_change(settings, &settings.sync_passphrase, new_passphrase)?;
    let result = sync_with_remote_as(store, db, settings, new_passphrase, params)?;
    let mut updated = db.load_settings()?;
    updated.sync_passphrase = new_passphrase.to_string();
    db.save_settings(&updated).map_err(|err| {
        AppError::Sync(format!(
            "云端已改用新密码，但本机保存失败（{}）：请在同步密码中填写新密码后保存",
            err
        ))
    })?;
    Ok(result)
}

fn temp_db_path(kind: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("taskreminder-{}-{}.db", kind, uuid::Uuid::new_v4()))
}

/// 导出本地数据库快照并清空其中的敏感设置（WebDAV 密码、同步密码）；远端只用到数据表。
fn export_local_snapshot_bytes(db_path: &std::path::Path) -> Result<Vec<u8>, AppError> {
    let snapshot = temp_db_path("snapshot");
    let result = (|| {
        {
            let conn = Connection::open(db_path)?;
            let escaped = snapshot.to_string_lossy().replace('\'', "''");
            conn.execute_batch(&format!("VACUUM INTO '{}'", escaped))?;
        }
        {
            let conn = Connection::open(&snapshot)?;
            conn.execute(
                "UPDATE settings SET webdav_password = '', sync_passphrase = ''",
                [],
            )?;
        }
        Ok(std::fs::read(&snapshot)?)
    })();
    let _ = std::fs::remove_file(&snapshot);
    result
}

fn merge_snapshot_bytes(local_path: &std::path::Path, data: &[u8]) -> Result<(), AppError> {
    let remote = temp_db_path("remote");
    let result = std::fs::write(&remote, data)
        .map_err(AppError::from)
        .and_then(|_| merge_databases(local_path, &remote));
    let _ = std::fs::remove_file(&remote);
    result
}

pub(crate) fn merge_databases(
    local_path: &std::path::Path,
    remote_path: &std::path::Path,
) -> Result<(), AppError> {
    let mut local = Connection::open(local_path)?;
    let remote = Connection::open(remote_path)?;
    ensure_sync_columns(&local)?;
    ensure_sync_columns(&remote)?;

    let tx = local.transaction()?;
    for table in SYNC_TABLES {
        merge_table(&tx, &remote, table)?;
    }
    tx.commit()?;
    Ok(())
}

/// 按行合并一张表，较新的一方胜出（见 `choose_row`）。
///
/// 参与合并的是本机表的全部同步列，包括更新版本新增、本版本不认识的列（`adopt_remote_columns`）。
/// 胜出的行只写入它所在的库实际拥有的列：远端来自不认识某列的旧版本时，该列保留本机的值，
/// 不会被清成默认值。本次才补上的列本机还没有值，本机行胜出时这些列取远端的值。
fn merge_table(local: &Connection, remote: &Connection, table: &SyncTable) -> Result<(), AppError> {
    let adopted = adopt_remote_columns(local, remote, table)?;
    let local_columns = adopted.columns;
    let remote_names = table_columns(remote, table.name)?;
    let remote_columns: Vec<String> = local_columns
        .iter()
        .filter(|name| {
            remote_names
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(name))
        })
        .cloned()
        .collect();

    let local_rows = load_rows(
        local,
        table.name,
        &local_columns,
        table.fallback_time_column,
    )?;
    let remote_rows = load_rows(
        remote,
        table.name,
        &remote_columns,
        table.fallback_time_column,
    )?;
    let mut all_ids: Vec<&String> = local_rows.keys().collect();
    all_ids.extend(
        remote_rows
            .keys()
            .filter(|id| !local_rows.contains_key(*id)),
    );
    if all_ids.is_empty() {
        return Ok(());
    }

    let position =
        |columns: &[String], name: &str| columns.iter().position(|c| c.eq_ignore_ascii_case(name));
    let fill_from_remote: Vec<(usize, usize)> = adopted
        .newly_added
        .iter()
        .filter_map(|name| {
            Some((
                position(&local_columns, name)?,
                position(&remote_columns, name)?,
            ))
        })
        .collect();

    let mut local_stmt = local.prepare(&upsert_sql(table.name, &local_columns))?;
    let mut remote_stmt = local.prepare(&upsert_sql(table.name, &remote_columns))?;
    for id in all_ids {
        match choose_row(local_rows.get(id), remote_rows.get(id)) {
            Some(Side::Local(row)) => match remote_rows.get(id) {
                Some(remote_row) if !fill_from_remote.is_empty() => {
                    let mut values = row.values.clone();
                    for &(local_index, remote_index) in &fill_from_remote {
                        values[local_index] = remote_row.values[remote_index].clone();
                    }
                    local_stmt.execute(params_from_iter(values.iter()))?;
                }
                _ => {
                    local_stmt.execute(params_from_iter(row.values.iter()))?;
                }
            },
            Some(Side::Remote(row)) => {
                remote_stmt.execute(params_from_iter(row.values.iter()))?;
            }
            None => {}
        }
    }
    Ok(())
}

/// 插入或只更新给出的列：没有给出的列（远端没有的列）保留本机原值。
fn upsert_sql(table: &str, columns: &[String]) -> String {
    let quoted: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    let updates: Vec<String> = quoted
        .iter()
        .zip(columns)
        .filter(|(_, name)| !name.eq_ignore_ascii_case("id"))
        .map(|(q, _)| format!("{0} = excluded.{0}", q))
        .collect();
    format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT(id) DO UPDATE SET {}",
        quote_ident(table),
        quoted.join(", "),
        vec!["?"; columns.len()].join(", "),
        updates.join(", ")
    )
}

enum Side<'a> {
    Local(&'a RowData),
    Remote(&'a RowData),
}

fn choose_row<'a>(local: Option<&'a RowData>, remote: Option<&'a RowData>) -> Option<Side<'a>> {
    match (local, remote) {
        (Some(l), None) => Some(Side::Local(l)),
        (None, Some(r)) => Some(Side::Remote(r)),
        (Some(l), Some(r)) => match (l.compare_time, r.compare_time) {
            (Some(lc), Some(rc)) if rc > lc => Some(Side::Remote(r)),
            (None, Some(_)) => Some(Side::Remote(r)),
            _ => Some(Side::Local(l)),
        },
        (None, None) => None,
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
    columns: &[String],
    fallback_time_column: &str,
) -> Result<HashMap<String, RowData>, AppError> {
    let quoted: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    let sql = format!("SELECT {} FROM {}", quoted.join(", "), quote_ident(table));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;

    let position = |name: &str| columns.iter().position(|c| c.eq_ignore_ascii_case(name));
    let id_index = position("id").unwrap_or(0);
    let updated_index = position("updated_at");
    let deleted_index = position("deleted_at");
    let fallback_index = position(fallback_time_column);

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

#[derive(Serialize, Deserialize, Clone)]
struct LockInfo {
    #[serde(rename = "deviceId", alias = "device_id")]
    device_id: String,
    #[serde(rename = "expiresAt", alias = "expires_at")]
    expires_at: i64,
}

impl LockInfo {
    fn new(device_id: &str) -> Self {
        let expires_at = time::unix_millis() + LOCK_TTL_SECONDS * 1000;
        Self {
            device_id: device_id.to_string(),
            expires_at,
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at_millis() <= time::unix_millis()
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
            client: crate::http::blocking_client_builder()
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

impl RemoteStore for WebDavClient {
    fn exists(&self, name: &str) -> Result<bool, AppError> {
        let url = build_url(&self.base_url, name);
        let mut req = self.client.head(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        Ok(resp.status() == StatusCode::OK)
    }

    fn get(&self, name: &str) -> Result<Vec<u8>, AppError> {
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
        Ok(bytes.to_vec())
    }

    fn put(&self, name: &str, data: Vec<u8>) -> Result<(), AppError> {
        let url = build_url(&self.base_url, name);
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

#[cfg(test)]
#[path = "sync_compat_tests.rs"]
mod compat_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str) -> (DbManager, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "taskreminder-sync-test-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        (DbManager::new(path.clone()).unwrap(), dir)
    }

    fn set_completed(path: &std::path::Path, id: &str, days_ago: i64) {
        let completed = time::format_datetime(&(time::now() - chrono::Duration::days(days_ago)));
        let conn = Connection::open(path).unwrap();
        conn.execute(
            "UPDATE tasks SET status = 'COMPLETED', completed_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![completed, id],
        )
        .unwrap();
    }

    fn copy_task(from: &std::path::Path, to: &std::path::Path, id: &str) {
        let conn = Connection::open(to).unwrap();
        let escaped = from.to_string_lossy().replace('\'', "''");
        conn.execute_batch(&format!("ATTACH DATABASE '{}' AS src", escaped))
            .unwrap();
        conn.execute(
            "INSERT INTO tasks SELECT * FROM src.tasks WHERE id = ?",
            [id],
        )
        .unwrap();
        conn.execute_batch("DETACH DATABASE src").unwrap();
    }

    fn deleted_at(path: &std::path::Path, id: &str) -> Option<Option<String>> {
        let conn = Connection::open(path).unwrap();
        conn.query_row("SELECT deleted_at FROM tasks WHERE id = ?", [id], |row| {
            row.get::<_, Option<String>>(0)
        })
        .ok()
    }

    #[test]
    fn cleaned_up_completed_task_does_not_resurrect_after_merge() {
        let (local, local_dir) = temp_db("local");
        let (remote, remote_dir) = temp_db("remote");
        let task = local.create_task("old completed", None).unwrap();
        set_completed(&local.db_path(), &task.id, 40);
        // 远端（另一台设备上传的库）仍保留这条已完成任务。
        copy_task(&local.db_path(), &remote.db_path(), &task.id);

        local
            .cleanup_data(crate::db::TOMBSTONE_RETENTION_DAYS_SYNC)
            .unwrap();
        merge_databases(&local.db_path(), &remote.db_path()).unwrap();

        let merged = deleted_at(&local.db_path(), &task.id).expect("row exists");
        assert!(merged.is_some(), "tombstone should win over older live row");

        let _ = std::fs::remove_dir_all(local_dir);
        let _ = std::fs::remove_dir_all(remote_dir);
    }

    #[test]
    fn purged_from_trash_does_not_come_back_after_merge() {
        let (local, local_dir) = temp_db("local");
        let (remote, remote_dir) = temp_db("remote");
        let task = local.create_task("trashed", None).unwrap();
        local.delete_task(&task.id).unwrap();
        // 远端保留同一条墓碑（删除已同步过）。
        copy_task(&local.db_path(), &remote.db_path(), &task.id);
        {
            let conn = Connection::open(remote.db_path()).unwrap();
            conn.execute(
                "UPDATE tasks SET updated_at = '2000-01-01T00:00:00' WHERE id = ?",
                [&task.id],
            )
            .unwrap();
        }

        local
            .expire_tombstones(crate::db::TrashTable::Tasks, std::slice::from_ref(&task.id))
            .unwrap();
        merge_databases(&local.db_path(), &remote.db_path()).unwrap();
        // 与 perform_sync 一致：合并后、上传前清理过期墓碑。
        local
            .purge_expired_tombstones(crate::db::TOMBSTONE_RETENTION_DAYS_SYNC)
            .unwrap();
        assert!(deleted_at(&local.db_path(), &task.id).is_none());

        let _ = std::fs::remove_dir_all(local_dir);
        let _ = std::fs::remove_dir_all(remote_dir);
    }

    #[test]
    fn merge_keeps_newer_row() {
        let (local, local_dir) = temp_db("local");
        let (remote, remote_dir) = temp_db("remote");
        let task = local.create_task("shared", None).unwrap();
        copy_task(&local.db_path(), &remote.db_path(), &task.id);
        {
            let conn = Connection::open(remote.db_path()).unwrap();
            conn.execute(
                "UPDATE tasks SET description = 'remote edit', tags = '工作,周报', priority = 2, updated_at = '2999-01-01T00:00:00' WHERE id = ?",
                [&task.id],
            )
            .unwrap();
        }
        merge_databases(&local.db_path(), &remote.db_path()).unwrap();
        let merged = local.get_task(&task.id).unwrap().unwrap();
        assert_eq!(merged.description, "remote edit");
        assert_eq!(merged.tags, vec!["工作", "周报"]);
        assert_eq!(merged.priority, 2);

        let _ = std::fs::remove_dir_all(local_dir);
        let _ = std::fs::remove_dir_all(remote_dir);
    }

    fn exec(path: &std::path::Path, sql: &str) {
        Connection::open(path).unwrap().execute_batch(sql).unwrap();
    }

    fn text_column(path: &std::path::Path, table: &str, column: &str, id: &str) -> Option<String> {
        let conn = Connection::open(path).unwrap();
        conn.query_row(
            &format!("SELECT {} FROM {} WHERE id = ?", column, table),
            [id],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn has_table_column(path: &std::path::Path, table: &str, column: &str) -> bool {
        let conn = Connection::open(path).unwrap();
        table_columns(&conn, table)
            .unwrap()
            .iter()
            .any(|c| c.name == column)
    }

    /// 模拟更新版本（v2.1）的设备：多一个同步列 `due_at`。
    fn add_future_column(path: &std::path::Path) {
        exec(path, "ALTER TABLE tasks ADD COLUMN due_at TEXT");
    }

    #[test]
    fn unknown_remote_column_is_adopted_with_its_values() {
        let (local, local_dir) = temp_db("local");
        let (remote, remote_dir) = temp_db("remote");
        let task = local.create_task("shared", None).unwrap();
        copy_task(&local.db_path(), &remote.db_path(), &task.id);
        add_future_column(&remote.db_path());
        exec(
            &remote.db_path(),
            &format!(
                "UPDATE tasks SET due_at = '2026-10-08T18:00:00', updated_at = '2999-01-01T00:00:00' WHERE id = '{}'",
                task.id
            ),
        );

        merge_databases(&local.db_path(), &remote.db_path()).unwrap();
        assert!(has_table_column(&local.db_path(), "tasks", "due_at"));
        assert_eq!(
            text_column(&local.db_path(), "tasks", "due_at", &task.id).as_deref(),
            Some("2026-10-08T18:00:00")
        );
        // 新列不影响本版本读取。
        assert_eq!(
            local.get_task(&task.id).unwrap().unwrap().description,
            "shared"
        );

        let _ = std::fs::remove_dir_all(local_dir);
        let _ = std::fs::remove_dir_all(remote_dir);
    }

    #[test]
    fn local_edit_keeps_adopted_column_and_uploads_it() {
        let (local, local_dir) = temp_db("local");
        let (remote, remote_dir) = temp_db("remote");
        let task = local.create_task("shared", None).unwrap();
        copy_task(&local.db_path(), &remote.db_path(), &task.id);
        add_future_column(&remote.db_path());
        exec(
            &remote.db_path(),
            &format!(
                "UPDATE tasks SET due_at = '2026-10-08T18:00:00', updated_at = '2000-01-02T00:00:00' WHERE id = '{}'",
                task.id
            ),
        );
        merge_databases(&local.db_path(), &remote.db_path()).unwrap();

        // 本机（不认识 due_at 的版本）修改标题后再次同步：本机行胜出，due_at 仍在。
        local
            .update_task(&task.id, "local edit", None, None, None)
            .unwrap();
        merge_databases(&local.db_path(), &remote.db_path()).unwrap();
        assert_eq!(
            text_column(&local.db_path(), "tasks", "due_at", &task.id).as_deref(),
            Some("2026-10-08T18:00:00")
        );

        // 上传的快照由更新版本的设备合并：标题改动与 due_at 都在。
        let (newer, newer_dir) = temp_db("newer");
        add_future_column(&newer.db_path());
        let snapshot = export_local_snapshot_bytes(&local.db_path()).unwrap();
        merge_snapshot_bytes(&newer.db_path(), &snapshot).unwrap();
        assert_eq!(
            text_column(&newer.db_path(), "tasks", "description", &task.id).as_deref(),
            Some("local edit")
        );
        assert_eq!(
            text_column(&newer.db_path(), "tasks", "due_at", &task.id).as_deref(),
            Some("2026-10-08T18:00:00")
        );

        let _ = std::fs::remove_dir_all(local_dir);
        let _ = std::fs::remove_dir_all(remote_dir);
        let _ = std::fs::remove_dir_all(newer_dir);
    }

    #[test]
    fn older_remote_without_column_keeps_local_value() {
        let (local, local_dir) = temp_db("local");
        let (remote, remote_dir) = temp_db("remote");
        let task = local.create_task("shared", None).unwrap();
        copy_task(&local.db_path(), &remote.db_path(), &task.id);
        // 本机是更新版本（有 due_at），远端来自不认识它的旧版本，并且旧版本改过这一行。
        add_future_column(&local.db_path());
        exec(
            &local.db_path(),
            &format!(
                "UPDATE tasks SET due_at = '2026-10-08T18:00:00' WHERE id = '{}'",
                task.id
            ),
        );
        exec(
            &remote.db_path(),
            &format!(
                "UPDATE tasks SET description = 'old device edit', updated_at = '2999-01-01T00:00:00' WHERE id = '{}'",
                task.id
            ),
        );

        merge_databases(&local.db_path(), &remote.db_path()).unwrap();
        assert_eq!(
            text_column(&local.db_path(), "tasks", "description", &task.id).as_deref(),
            Some("old device edit")
        );
        assert_eq!(
            text_column(&local.db_path(), "tasks", "due_at", &task.id).as_deref(),
            Some("2026-10-08T18:00:00")
        );

        let _ = std::fs::remove_dir_all(local_dir);
        let _ = std::fs::remove_dir_all(remote_dir);
    }

    #[test]
    fn unaddable_remote_column_is_skipped_without_failing() {
        let (local, local_dir) = temp_db("local");
        let (remote, remote_dir) = temp_db("remote");
        let task = local.create_task("shared", None).unwrap();
        copy_task(&local.db_path(), &remote.db_path(), &task.id);
        // 远端表被重建为带 NOT NULL 且无默认值的新列：本机无法给已有行补值，只能跳过。
        exec(
            &remote.db_path(),
            "CREATE TABLE tasks_new AS SELECT *, 'x' AS strict_col FROM tasks;
             DROP TABLE tasks;
             CREATE TABLE tasks AS SELECT * FROM tasks_new WHERE 0;
             DROP TABLE tasks_new;",
        );
        {
            // 用完整定义重建，确保 strict_col 为 NOT NULL 无默认值。
            let conn = Connection::open(remote.db_path()).unwrap();
            conn.execute_batch("DROP TABLE tasks").unwrap();
            conn.execute_batch(
                "CREATE TABLE tasks (id TEXT PRIMARY KEY, description TEXT NOT NULL, type TEXT NOT NULL,
                 status TEXT NOT NULL, created_at TEXT NOT NULL, completed_at TEXT, reminder_time TEXT,
                 strict_col TEXT NOT NULL)",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tasks (id, description, type, status, created_at, strict_col)
                 VALUES ('remote-only', 'from remote', 'ONE_TIME', 'PENDING', '2026-01-01T00:00:00', 'v')",
                [],
            )
            .unwrap();
        }

        merge_databases(&local.db_path(), &remote.db_path()).unwrap();
        assert!(!has_table_column(&local.db_path(), "tasks", "strict_col"));
        assert!(local.get_task("remote-only").unwrap().is_some());
        assert!(local.get_task(&task.id).unwrap().is_some());

        let _ = std::fs::remove_dir_all(local_dir);
        let _ = std::fs::remove_dir_all(remote_dir);
    }

    #[test]
    fn local_only_columns_are_not_synced() {
        let (local, local_dir) = temp_db("local");
        let (remote, remote_dir) = temp_db("remote");
        let task = local.create_task("shared", None).unwrap();
        let other = local.create_task("other", None).unwrap();
        copy_task(&local.db_path(), &remote.db_path(), &task.id);
        copy_task(&local.db_path(), &remote.db_path(), &other.id);
        local.set_sticky_note_pinned(&task.id, true).unwrap();
        exec(
            &remote.db_path(),
            &format!(
                "UPDATE tasks SET description = 'remote edit', sticky_is_pinned = 1,
                                  updated_at = '2999-01-01T00:00:00' WHERE id = '{}';
                 UPDATE tasks SET description = 'remote edit', sticky_is_pinned = 0,
                                  updated_at = '2999-01-01T00:00:00' WHERE id = '{}';",
                other.id, task.id
            ),
        );
        merge_databases(&local.db_path(), &remote.db_path()).unwrap();
        // 远端行胜出：同步列取远端的值，锚定状态各自保留本机的值。
        assert_eq!(
            local.get_task(&task.id).unwrap().unwrap().description,
            "remote edit"
        );
        assert!(local.get_sticky_note_pinned(&task.id).unwrap());
        assert_eq!(
            local.get_task(&other.id).unwrap().unwrap().description,
            "remote edit"
        );
        assert!(!local.get_sticky_note_pinned(&other.id).unwrap());

        let _ = std::fs::remove_dir_all(local_dir);
        let _ = std::fs::remove_dir_all(remote_dir);
    }

    #[derive(Default)]
    struct MemoryStore {
        files: std::cell::RefCell<HashMap<String, Vec<u8>>>,
    }

    impl MemoryStore {
        fn file(&self, name: &str) -> Option<Vec<u8>> {
            self.files.borrow().get(name).cloned()
        }
    }

    impl RemoteStore for MemoryStore {
        fn exists(&self, name: &str) -> Result<bool, AppError> {
            Ok(self.files.borrow().contains_key(name))
        }

        fn get(&self, name: &str) -> Result<Vec<u8>, AppError> {
            self.file(name)
                .ok_or_else(|| AppError::Sync("not found".to_string()))
        }

        fn put(&self, name: &str, data: Vec<u8>) -> Result<(), AppError> {
            self.files.borrow_mut().insert(name.to_string(), data);
            Ok(())
        }
    }

    #[test]
    fn sync_state_parses_codes_and_legacy_text() {
        for state in [
            SyncState::Never,
            SyncState::Syncing,
            SyncState::Success,
            SyncState::FirstSync,
            SyncState::LockBusy,
            SyncState::Failed,
            SyncState::PassphraseMismatch,
        ] {
            assert_eq!(SyncState::parse(state.code()), Some(state));
        }
        assert_eq!(SyncState::parse("同步成功"), Some(SyncState::Success));
        assert_eq!(SyncState::parse("首次同步完成"), Some(SyncState::FirstSync));
        assert_eq!(SyncState::parse("同步失败"), Some(SyncState::Failed));
        assert_eq!(
            SyncState::parse("锁被占用，稍后重试"),
            Some(SyncState::LockBusy)
        );
        assert_eq!(SyncState::parse("未知"), None);

        assert_eq!(status_code(None), "never");
        assert_eq!(status_code(Some("同步成功")), "success");
        assert_eq!(status_code(Some("first_sync")), "first_sync");
        assert_eq!(status_code(Some("未知")), "未知");
    }

    #[test]
    fn dirty_check_uses_status_codes_and_legacy_text() {
        let (db, dir) = temp_db("dirty");
        let mut settings = db.load_settings().unwrap();
        settings.webdav_last_local_change_time = Some("2026-09-27T10:00:00".to_string());
        settings.webdav_last_sync_time = Some("2026-09-27T11:00:00".to_string());

        for (status, dirty) in [
            ("success", false),
            ("first_sync", false),
            ("同步成功", false),
            ("首次同步完成", false),
            ("failed", true),
            ("syncing", true),
            ("lock_busy", true),
            ("同步失败", true),
        ] {
            settings.webdav_last_sync_status = Some(status.to_string());
            assert_eq!(compute_dirty_from_settings(&settings), dirty, "{status}");
        }

        // 成功同步之后又有本地修改：仍需同步。
        settings.webdav_last_sync_status = Some("success".to_string());
        settings.webdav_last_local_change_time = Some("2026-09-27T12:00:00".to_string());
        assert!(compute_dirty_from_settings(&settings));

        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn update_sync_status_stores_code() {
        let (db, dir) = temp_db("status-code");
        let settings = db
            .update_sync_status(SyncState::Success.code(), None)
            .unwrap();
        assert_eq!(settings.webdav_last_sync_status.as_deref(), Some("success"));
        assert!(!compute_dirty_from_settings(&settings));
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn settings_for(db: &DbManager, encrypted: bool, passphrase: &str) -> AppSettings {
        let mut settings = db.load_settings().unwrap();
        settings.webdav_password = "webdav-secret".to_string();
        settings.sync_encryption_enabled = encrypted;
        settings.sync_passphrase = passphrase.to_string();
        db.save_settings(&settings).unwrap();
        settings
    }

    fn sync(
        store: &MemoryStore,
        db: &DbManager,
        settings: &AppSettings,
    ) -> Result<RemoteSyncResult, AppError> {
        sync_with_remote(store, db, settings, crate::sync_crypto::TEST_PARAMS)
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn task_titles(db: &DbManager) -> Vec<String> {
        let mut titles: Vec<String> = db
            .list_active_tasks()
            .unwrap()
            .into_iter()
            .map(|task| task.description)
            .collect();
        titles.sort();
        titles
    }

    const PASS: &str = "correct horse battery";

    #[test]
    fn plain_sync_uploads_scrubbed_snapshot_and_merges() {
        let (a, a_dir) = temp_db("plain-a");
        let (b, b_dir) = temp_db("plain-b");
        let store = MemoryStore::default();
        a.create_task("来自 A", None).unwrap();
        b.create_task("来自 B", None).unwrap();

        let settings_a = settings_for(&a, false, "");
        assert_eq!(
            sync(&store, &a, &settings_a).unwrap(),
            RemoteSyncResult::FirstUpload
        );
        let uploaded = store.file(REMOTE_DB_NAME).unwrap();
        assert!(uploaded.starts_with(SQLITE_MAGIC));
        // 上传的快照不含 WebDAV 密码。
        assert!(!contains(&uploaded, b"webdav-secret"));

        let settings_b = settings_for(&b, false, "");
        assert_eq!(
            sync(&store, &b, &settings_b).unwrap(),
            RemoteSyncResult::Merged
        );
        assert_eq!(task_titles(&b), vec!["来自 A", "来自 B"]);
        // 本机设置不受影响。
        assert_eq!(b.load_settings().unwrap().webdav_password, "webdav-secret");

        let _ = std::fs::remove_dir_all(a_dir);
        let _ = std::fs::remove_dir_all(b_dir);
    }

    #[test]
    fn encrypted_sync_roundtrips_without_plaintext_on_server() {
        let (a, a_dir) = temp_db("enc-a");
        let (b, b_dir) = temp_db("enc-b");
        let store = MemoryStore::default();
        a.create_task("机密会议", None).unwrap();

        let settings_a = settings_for(&a, true, PASS);
        assert_eq!(
            sync(&store, &a, &settings_a).unwrap(),
            RemoteSyncResult::FirstUpload
        );
        let encrypted = store.file(REMOTE_ENC_NAME).unwrap();
        assert!(crate::sync_crypto::is_encrypted(&encrypted));
        assert!(!contains(&encrypted, "机密会议".as_bytes()));
        assert!(!contains(&encrypted, PASS.as_bytes()));
        assert!(store
            .file(REMOTE_DB_NAME)
            .unwrap()
            .starts_with(PLACEHOLDER_MARKER));

        let settings_b = settings_for(&b, true, PASS);
        assert_eq!(
            sync(&store, &b, &settings_b).unwrap(),
            RemoteSyncResult::Merged
        );
        assert_eq!(task_titles(&b), vec!["机密会议"]);

        let _ = std::fs::remove_dir_all(a_dir);
        let _ = std::fs::remove_dir_all(b_dir);
    }

    #[test]
    fn enabling_encryption_migrates_plain_remote_and_blocks_plain_devices() {
        let (a, a_dir) = temp_db("mig-a");
        let (b, b_dir) = temp_db("mig-b");
        let store = MemoryStore::default();
        a.create_task("A 的待办", None).unwrap();
        b.create_task("B 的待办", None).unwrap();

        let plain_a = settings_for(&a, false, "");
        sync(&store, &a, &plain_a).unwrap();

        // B 开启加密：合并明文后改为上传加密文件，明文文件被占位说明替换。
        let encrypted_b = settings_for(&b, true, PASS);
        assert_eq!(
            sync(&store, &b, &encrypted_b).unwrap(),
            RemoteSyncResult::Merged
        );
        assert_eq!(task_titles(&b), vec!["A 的待办", "B 的待办"]);
        assert!(store.file(REMOTE_ENC_NAME).is_some());
        let placeholder = store.file(REMOTE_DB_NAME).unwrap();
        assert!(placeholder.starts_with(PLACEHOLDER_MARKER));

        // A 仍未开启加密：报错且不上传明文。
        let err = sync(&store, &a, &plain_a).unwrap_err().to_string();
        assert!(err.contains("端到端加密"), "{}", err);
        assert_eq!(store.file(REMOTE_DB_NAME).unwrap(), placeholder);

        // 旧版本会把占位说明当作数据库合并：必须失败，才不会继续上传明文。
        let placeholder_path = a_dir.join("placeholder.db");
        std::fs::write(&placeholder_path, &placeholder).unwrap();
        assert!(merge_databases(&a.db_path(), &placeholder_path).is_err());

        let _ = std::fs::remove_dir_all(a_dir);
        let _ = std::fs::remove_dir_all(b_dir);
    }

    #[test]
    fn wrong_passphrase_aborts_without_overwriting_remote() {
        let (a, a_dir) = temp_db("wrong-a");
        let (b, b_dir) = temp_db("wrong-b");
        let store = MemoryStore::default();
        a.create_task("A 的待办", None).unwrap();
        b.create_task("B 的待办", None).unwrap();
        sync(&store, &a, &settings_for(&a, true, PASS)).unwrap();
        let before = store.file(REMOTE_ENC_NAME).unwrap();

        let wrong = settings_for(&b, true, "another passphrase");
        let err = sync(&store, &b, &wrong).unwrap_err();
        assert!(matches!(err, AppError::SyncPassphrase(_)), "{:?}", err);
        assert_eq!(failure_state(&err), SyncState::PassphraseMismatch);
        assert!(err.to_string().contains("同步密码错误"), "{}", err);
        assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), before);
        assert_eq!(task_titles(&b), vec!["B 的待办"]);

        // 密码太短直接拒绝。
        let short = settings_for(&b, true, "short");
        assert!(sync(&store, &b, &short).is_err());
        assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), before);

        let _ = std::fs::remove_dir_all(a_dir);
        let _ = std::fs::remove_dir_all(b_dir);
    }

    #[test]
    fn placeholder_without_encrypted_file_counts_as_first_sync() {
        let (a, a_dir) = temp_db("ph-a");
        let store = MemoryStore::default();
        store.put(REMOTE_DB_NAME, placeholder_content()).unwrap();
        a.create_task("A 的待办", None).unwrap();

        assert_eq!(
            sync(&store, &a, &settings_for(&a, true, PASS)).unwrap(),
            RemoteSyncResult::FirstUpload
        );
        assert!(store.file(REMOTE_ENC_NAME).is_some());
        // 明文设备同样按首次同步处理，覆盖占位说明。
        let store = MemoryStore::default();
        store.put(REMOTE_DB_NAME, placeholder_content()).unwrap();
        assert_eq!(
            sync(&store, &a, &settings_for(&a, false, "")).unwrap(),
            RemoteSyncResult::FirstUpload
        );
        assert!(store
            .file(REMOTE_DB_NAME)
            .unwrap()
            .starts_with(SQLITE_MAGIC));
        // 其他格式的文件不会被当作数据库合并或覆盖。
        let store = MemoryStore::default();
        store.put(REMOTE_DB_NAME, b"garbage".to_vec()).unwrap();
        assert!(sync(&store, &a, &settings_for(&a, false, "")).is_err());
        assert_eq!(store.file(REMOTE_DB_NAME).unwrap(), b"garbage".to_vec());

        let _ = std::fs::remove_dir_all(a_dir);
    }

    #[test]
    fn connection_test_checks_passphrase() {
        let (a, a_dir) = temp_db("check-a");
        let store = MemoryStore::default();
        let plain = settings_for(&a, false, "");
        assert!(check_encryption(&store, &plain).0);

        sync(&store, &a, &settings_for(&a, true, PASS)).unwrap();
        let (ok, message) = check_encryption(&store, &plain);
        assert!(!ok && message.contains("已加密"), "{}", message);
        let (ok, message) = check_encryption(&store, &settings_for(&a, true, PASS));
        assert!(ok && message.contains("密码正确"), "{}", message);
        let (ok, message) = check_encryption(&store, &settings_for(&a, true, "wrong passphrase"));
        assert!(!ok && message.contains("密码错误"), "{}", message);

        let _ = std::fs::remove_dir_all(a_dir);
    }

    const NEW_PASS: &str = "new staple passphrase";

    fn change(
        store: &dyn RemoteStore,
        db: &DbManager,
        settings: &AppSettings,
        new: &str,
    ) -> Result<RemoteSyncResult, AppError> {
        change_passphrase_on_remote(store, db, settings, new, crate::sync_crypto::TEST_PARAMS)
    }

    /// 上传总是失败的远端，用来验证失败时本机仍保留旧密码。
    struct FailingPutStore<'a>(&'a MemoryStore);

    impl RemoteStore for FailingPutStore<'_> {
        fn exists(&self, name: &str) -> Result<bool, AppError> {
            self.0.exists(name)
        }

        fn get(&self, name: &str) -> Result<Vec<u8>, AppError> {
            self.0.get(name)
        }

        fn put(&self, _name: &str, _data: Vec<u8>) -> Result<(), AppError> {
            Err(AppError::Sync("upload failed".to_string()))
        }
    }

    #[test]
    fn changing_passphrase_reencrypts_and_other_device_recovers() {
        let (a, a_dir) = temp_db("change-a");
        let (b, b_dir) = temp_db("change-b");
        let store = MemoryStore::default();
        a.create_task("A 的待办", None).unwrap();
        b.create_task("B 的待办", None).unwrap();
        sync(&store, &a, &settings_for(&a, true, PASS)).unwrap();
        sync(&store, &b, &settings_for(&b, true, PASS)).unwrap();

        // A 更换密码前又改了数据：先合并远端（B 的待办），再用新密码上传。
        a.create_task("A 更换前新增", None).unwrap();
        let settings_a = settings_for(&a, true, PASS);
        assert_eq!(
            change(&store, &a, &settings_a, NEW_PASS).unwrap(),
            RemoteSyncResult::Merged
        );
        assert_eq!(a.load_settings().unwrap().sync_passphrase, NEW_PASS);
        assert_eq!(
            task_titles(&a),
            vec!["A 更换前新增", "A 的待办", "B 的待办"]
        );
        let encrypted = store.file(REMOTE_ENC_NAME).unwrap();
        assert!(crate::sync_crypto::decrypt(&encrypted, NEW_PASS).is_ok());
        assert!(crate::sync_crypto::decrypt(&encrypted, PASS).is_err());
        assert!(!contains(&encrypted, NEW_PASS.as_bytes()));
        assert!(store
            .file(REMOTE_DB_NAME)
            .unwrap()
            .starts_with(PLACEHOLDER_MARKER));

        // B 仍用旧密码：得到“密码不匹配”，不覆盖远端。
        let err = sync(&store, &b, &settings_for(&b, true, PASS)).unwrap_err();
        assert_eq!(failure_state(&err), SyncState::PassphraseMismatch);
        assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), encrypted);

        // B 填入新密码后恢复同步。
        assert_eq!(
            sync(&store, &b, &settings_for(&b, true, NEW_PASS)).unwrap(),
            RemoteSyncResult::Merged
        );
        assert_eq!(
            task_titles(&b),
            vec!["A 更换前新增", "A 的待办", "B 的待办"]
        );

        let _ = std::fs::remove_dir_all(a_dir);
        let _ = std::fs::remove_dir_all(b_dir);
    }

    #[test]
    fn passphrase_change_is_validated_before_touching_remote() {
        let (a, a_dir) = temp_db("change-check");
        let plain = settings_for(&a, false, "");
        assert!(validate_passphrase_change(&plain, "", NEW_PASS).is_err());
        let settings = settings_for(&a, true, PASS);
        for (current, new, expected) in [
            ("not the current one", NEW_PASS, "不正确"),
            (PASS, "short", "至少"),
            (PASS, PASS, "相同"),
        ] {
            let err = validate_passphrase_change(&settings, current, new)
                .unwrap_err()
                .to_string();
            assert!(err.contains(expected), "{} -> {}", new, err);
        }
        assert!(validate_passphrase_change(&settings, PASS, NEW_PASS).is_ok());
        let _ = std::fs::remove_dir_all(a_dir);
    }

    #[test]
    fn passphrase_change_with_undecryptable_remote_keeps_everything() {
        let (a, a_dir) = temp_db("change-wrong-a");
        let (b, b_dir) = temp_db("change-wrong-b");
        let store = MemoryStore::default();
        // 云端已被另一台设备用别的密码加密（例如那台设备先改过密码）。
        b.create_task("B 的待办", None).unwrap();
        sync(&store, &b, &settings_for(&b, true, "some other passphrase")).unwrap();
        let before = store.file(REMOTE_ENC_NAME).unwrap();

        a.create_task("A 的待办", None).unwrap();
        let err = change(&store, &a, &settings_for(&a, true, PASS), NEW_PASS).unwrap_err();
        assert_eq!(failure_state(&err), SyncState::PassphraseMismatch);
        assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), before);
        assert_eq!(a.load_settings().unwrap().sync_passphrase, PASS);
        assert_eq!(task_titles(&a), vec!["A 的待办"]);

        let _ = std::fs::remove_dir_all(a_dir);
        let _ = std::fs::remove_dir_all(b_dir);
    }

    #[test]
    fn failed_upload_keeps_local_passphrase() {
        let (a, a_dir) = temp_db("change-upload");
        let store = MemoryStore::default();
        a.create_task("A 的待办", None).unwrap();
        sync(&store, &a, &settings_for(&a, true, PASS)).unwrap();
        let before = store.file(REMOTE_ENC_NAME).unwrap();

        let failing = FailingPutStore(&store);
        assert!(change(&failing, &a, &settings_for(&a, true, PASS), NEW_PASS).is_err());
        assert_eq!(a.load_settings().unwrap().sync_passphrase, PASS);
        assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), before);
        // 仍可用旧密码正常同步。
        assert!(sync(&store, &a, &settings_for(&a, true, PASS)).is_ok());

        let _ = std::fs::remove_dir_all(a_dir);
    }

    #[test]
    fn passphrase_change_on_empty_remote_uploads_with_new_passphrase() {
        let (a, a_dir) = temp_db("change-empty");
        let store = MemoryStore::default();
        a.create_task("A 的待办", None).unwrap();
        assert_eq!(
            change(&store, &a, &settings_for(&a, true, PASS), NEW_PASS).unwrap(),
            RemoteSyncResult::FirstUpload
        );
        let encrypted = store.file(REMOTE_ENC_NAME).unwrap();
        assert!(crate::sync_crypto::decrypt(&encrypted, NEW_PASS).is_ok());
        let _ = std::fs::remove_dir_all(a_dir);
    }

    #[test]
    fn mismatch_pauses_auto_sync_until_passphrase_is_edited() {
        let (a, a_dir) = temp_db("pause");
        let mut previous = settings_for(&a, true, PASS);
        previous.webdav_enabled = true;
        previous.webdav_last_sync_status = Some("failed".to_string());
        assert!(!auto_sync_paused(&previous));

        previous.webdav_last_sync_status = Some(SyncState::PassphraseMismatch.code().to_string());
        assert!(auto_sync_paused(&previous));
        // 仍不匹配时，未改密码的保存（例如只改了同步频率）不会触发同步。
        assert!(!should_resync_after_passphrase_edit(&previous, &previous));

        let mut saved = previous.clone();
        saved.sync_passphrase = NEW_PASS.to_string();
        assert!(should_resync_after_passphrase_edit(&previous, &saved));
        saved.webdav_enabled = false;
        assert!(!should_resync_after_passphrase_edit(&previous, &saved));

        let _ = std::fs::remove_dir_all(a_dir);
    }
}
