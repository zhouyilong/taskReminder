//! SQLite 双库底座：`DbManager` 管理设备级配置库（固定为 `taskreminder.db`）与可热切换的活动业务库
//! （本地模式为 `taskreminder.db`，云端模式为 `accounts/<user_id>.db`）。
//!
//! 各业务表的读写按模块拆分（`tasks`、`recurring`、`records`、`sticky`、`settings`、`tombstones`、`import`、`labels`、`cloud_sync`、`migrations`）。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use chrono::NaiveDateTime;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::attention;
use crate::backup::ImportSummary;
use crate::errors::AppError;
use crate::kinds::{AppMode, ReminderAction, ReminderKind, RepeatMode, TaskStatus, TaskType};
use crate::models::{
    default_quick_add_shortcut, normalize_attention_repeat_minutes, normalize_auto_backup_keep,
    normalize_completed_retention_days, normalize_priority, normalize_project, tags_from_db,
    tags_to_db, AppSettings, BusinessDataCounts, NotificationPrefs, RecurringTask, ReminderRecord,
    StickyNote, Task, TodoStickyState, DEFAULT_AUTO_BACKUP_KEEP, DEFAULT_COMPLETED_RETENTION_DAYS,
};
use crate::paths;
use crate::quiet_hours;
use crate::secrets::{self, SecretStore};
use crate::time::{self, format_datetime, now_string, parse_datetime_any};

mod cloud_sync;
mod import;
mod labels;
mod migrations;
mod records;
mod recurring;
mod settings;
mod sticky;
mod tasks;
mod tombstones;

#[cfg(test)]
pub(crate) use migrations::create_schema_up_to;
#[cfg(test)]
use migrations::execute_sql_script;
pub use tasks::{TaskBatchOp, TaskMeta};

/// 墓碑（软删除行）的保留天数。云端模式保留更久，
/// 让较长时间未同步的设备也能收到删除，而不是把旧数据重新上传“复活”。
pub const TOMBSTONE_RETENTION_DAYS_LOCAL: i64 = 7;
pub const TOMBSTONE_RETENTION_DAYS_SYNC: i64 = 60;

pub fn tombstone_retention_days(cloud_mode: bool) -> i64 {
    if cloud_mode {
        TOMBSTONE_RETENTION_DAYS_SYNC
    } else {
        TOMBSTONE_RETENTION_DAYS_LOCAL
    }
}

/// 回收站“永久删除”时写入的删除时间：早于任何保留期，下一次清理即会物理删除。
pub const EXPIRED_TOMBSTONE_TIME: &str = "1970-01-01T00:00:00";

/// 回收站涉及的表。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashTable {
    Tasks,
    RecurringTasks,
}

impl TrashTable {
    fn name(self) -> &'static str {
        match self {
            TrashTable::Tasks => "tasks",
            TrashTable::RecurringTasks => "recurring_tasks",
        }
    }
}

fn tombstone_cutoff(retention_days: i64) -> String {
    format_datetime(&(time::now() - chrono::Duration::days(retention_days.max(1))))
}

struct ActiveDb {
    pool: Pool<SqliteConnectionManager>,
    path: PathBuf,
    is_cloud: bool,
    account_id: Option<String>,
}

#[derive(Clone)]
pub struct DbManager {
    /// 设备级配置库连接池（固定指向 `taskreminder.db`，不受业务库切换影响）。
    settings_pool: Pool<SqliteConnectionManager>,
    /// 本地主库路径（`taskreminder.db`）。
    local_db_path: PathBuf,
    /// 当前激活的业务库（本地模式与 `settings_pool` 共用；云端模式指向 `accounts/<user_id>.db`）。
    active: Arc<RwLock<ActiveDb>>,
    /// 存放云端 Refresh Token 的系统凭据库；None 时存本地数据库。
    secret_store: Option<Arc<dyn SecretStore>>,
    /// 已从凭据库读到（或写入）的 Refresh Token 缓存。
    token_cache: Arc<Mutex<Option<String>>>,
}

impl DbManager {
    pub fn new(db_path: PathBuf) -> Result<Self, AppError> {
        let store = if cfg!(test) {
            None
        } else {
            secrets::platform_store()
        };
        Self::with_secret_store(db_path, store)
    }

    pub fn with_secret_store(
        db_path: PathBuf,
        secret_store: Option<Arc<dyn SecretStore>>,
    ) -> Result<Self, AppError> {
        let settings_pool = Self::open_and_init_pool(&db_path)?;
        let active = ActiveDb {
            pool: settings_pool.clone(),
            path: db_path.clone(),
            is_cloud: false,
            account_id: None,
        };
        let db = DbManager {
            settings_pool,
            local_db_path: db_path,
            active: Arc::new(RwLock::new(active)),
            secret_store,
            token_cache: Arc::new(Mutex::new(None)),
        };
        db.cleanup_and_migrate_secrets();
        Ok(db)
    }

    fn open_and_init_pool(path: &Path) -> Result<Pool<SqliteConnectionManager>, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::new(manager).map_err(|e| AppError::Database(e.to_string()))?;
        let mut conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
        Self::apply_pragmas_on(&conn)?;
        Self::ensure_version_table_on(&conn)?;
        Self::apply_migrations_on(&mut conn)?;
        Self::ensure_settings_row_on(&conn)?;
        // 设备 ID 用于会话标识与凭据库条目名，必须稳定：为空时生成一个并保存。
        conn.execute(
            "UPDATE settings SET webdav_device_id = ?
             WHERE id = 1 AND (webdav_device_id IS NULL OR TRIM(webdav_device_id) = '')",
            [Uuid::new_v4().to_string()],
        )?;
        Ok(pool)
    }

    /// 清理旧版 WebDAV 凭据条目，并把仍在数据库中的 Refresh Token 迁入系统凭据库。
    fn cleanup_and_migrate_secrets(&self) {
        let Some(store) = &self.secret_store else {
            return;
        };
        let Ok(conn) = self.get_settings_conn() else {
            return;
        };
        let Ok((storage, device_id, db_token)) = conn.query_row(
            "SELECT secret_storage, webdav_device_id, COALESCE(cloud_refresh_token, '')
             FROM settings WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        ) else {
            return;
        };

        // 升级到 v3.0.0 时清理旧版遗留的 WebDAV 密码条目。
        secrets::cleanup_legacy_webdav_secrets(store.as_ref(), &device_id);

        if storage == secrets::STORAGE_KEYRING || db_token.is_empty() {
            return;
        }
        match secrets::write_refresh_token(store.as_ref(), &device_id, &db_token) {
            Ok(()) => {
                let _ = conn.execute(
                    "UPDATE settings SET cloud_refresh_token = '', secret_storage = ? WHERE id = 1",
                    [secrets::STORAGE_KEYRING],
                );
                *self.token_cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(db_token);
            }
            Err(err) => {
                eprintln!(
                    "[secrets] 迁移云端令牌到凭据库失败，继续使用本地数据库: {}",
                    err
                );
            }
        }
    }

    /// 读取本机保存的云端 Refresh Token（不暴露给前端 `AppSettings`）。
    pub fn load_refresh_token(&self) -> Result<String, AppError> {
        let conn = self.get_settings_conn()?;
        let (storage, device_id, db_token): (String, String, String) = conn.query_row(
            "SELECT secret_storage, webdav_device_id, COALESCE(cloud_refresh_token, '')
             FROM settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if storage == secrets::STORAGE_KEYRING {
            if let Some(store) = &self.secret_store {
                let mut cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(cached) = cache.as_ref() {
                    return Ok(cached.clone());
                }
                match secrets::read_refresh_token(store.as_ref(), &device_id) {
                    Ok(token) => {
                        *cache = Some(token.clone());
                        return Ok(token);
                    }
                    Err(err) => {
                        eprintln!("[secrets] 读取凭据库令牌失败: {}", err);
                        return Ok(String::new());
                    }
                }
            }
        }
        Ok(db_token)
    }

    /// 保存或清空云端 Refresh Token（Windows 优先存系统凭据管理器，失败时自动退回本地库）。
    pub fn save_refresh_token(&self, token: &str) -> Result<(), AppError> {
        let conn = self.get_settings_conn()?;
        let device_id: String = conn.query_row(
            "SELECT webdav_device_id FROM settings WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        if let Some(store) = &self.secret_store {
            match secrets::write_refresh_token(store.as_ref(), &device_id, token) {
                Ok(()) => {
                    *self.token_cache.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(token.to_string());
                    conn.execute(
                        "UPDATE settings SET cloud_refresh_token = '', secret_storage = ? WHERE id = 1",
                        [secrets::STORAGE_KEYRING],
                    )?;
                    return Ok(());
                }
                Err(err) => {
                    eprintln!("[secrets] 写入凭据库令牌失败，改存本地数据库: {}", err);
                    *self.token_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
                }
            }
        }
        conn.execute(
            "UPDATE settings SET cloud_refresh_token = ?, secret_storage = ? WHERE id = 1",
            params![token, secrets::STORAGE_DB],
        )?;
        Ok(())
    }

    /// 热切换活动业务库至本地主库（`taskreminder.db`）。
    pub fn switch_to_local(&self) {
        let mut active = self.active.write().unwrap_or_else(|e| e.into_inner());
        active.pool = self.settings_pool.clone();
        active.path = self.local_db_path.clone();
        active.is_cloud = false;
        active.account_id = None;
    }

    /// 热切换活动业务库至指定云端账号的镜像库（`accounts/<user_id>.db`）。
    pub fn switch_to_account(&self, user_id: &str) -> Result<PathBuf, AppError> {
        let path = self.account_db_path(user_id);
        let pool = Self::open_and_init_pool(&path)?;
        let mut active = self.active.write().unwrap_or_else(|e| e.into_inner());
        active.pool = pool;
        active.path = path.clone();
        active.is_cloud = true;
        active.account_id = Some(user_id.to_string());
        Ok(path)
    }

    /// 删除指定云端账号的本地镜像库缓存（退出登录时调用）；若当前正激活该库则先切回本地库。
    pub fn remove_account_cache(&self, user_id: &str) {
        let is_current = {
            let active = self.active.read().unwrap_or_else(|e| e.into_inner());
            active.account_id.as_deref() == Some(user_id)
        };
        if is_current {
            self.switch_to_local();
        }
        let path = self.account_db_path(user_id);
        for suffix in ["", "-wal", "-shm"] {
            let target = PathBuf::from(format!("{}{}", path.to_string_lossy(), suffix));
            let _ = std::fs::remove_file(target);
        }
    }

    pub fn account_db_path(&self, user_id: &str) -> PathBuf {
        paths::account_db_path(&self.local_db_path, user_id)
    }

    /// 本地主库路径（`taskreminder.db`，备份目录与设备设置均位于此处）。
    pub fn local_db_path(&self) -> PathBuf {
        self.local_db_path.clone()
    }

    /// 当前活动业务库路径（本地模式为 `taskreminder.db`，云端模式为 `accounts/<user_id>.db`）。
    pub fn db_path(&self) -> PathBuf {
        self.active
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .path
            .clone()
    }

    pub fn is_cloud_active(&self) -> bool {
        self.active
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .is_cloud
    }

    /// 获取设备级配置库连接（始终指向 `taskreminder.db`）。
    pub(super) fn get_settings_conn(
        &self,
    ) -> Result<PooledConnection<SqliteConnectionManager>, AppError> {
        self.settings_pool
            .get()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// 获取当前活动业务库连接。
    pub(super) fn get_conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, AppError> {
        let pool = self
            .active
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .pool
            .clone();
        pool.get().map_err(|e| AppError::Database(e.to_string()))
    }

    /// 若当前处于云端模式，将发生变更的 `(table_name, row_id)` 记入 `pending_mutations`。
    pub(super) fn mark_pending(
        &self,
        conn: &Connection,
        table_name: &str,
        row_id: &str,
    ) -> Result<(), AppError> {
        if !self.is_cloud_active() || row_id.trim().is_empty() {
            return Ok(());
        }
        conn.execute(
            "INSERT INTO pending_mutations (table_name, row_id, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(table_name, row_id) DO UPDATE SET updated_at = excluded.updated_at",
            params![table_name, row_id, now_string()],
        )?;
        Ok(())
    }

    fn apply_pragmas_on(conn: &Connection) -> Result<(), AppError> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;\
             PRAGMA foreign_keys=ON;\
             PRAGMA busy_timeout=5000;\
             PRAGMA synchronous=NORMAL;\
             PRAGMA page_size=4096;\
             PRAGMA wal_autocheckpoint=1000;\
             PRAGMA cache_size=-2000;\
             PRAGMA temp_store=MEMORY;\
             PRAGMA mmap_size=67108864;",
        )?;
        Ok(())
    }

    fn ensure_settings_row_on(conn: &Connection) -> Result<(), AppError> {
        conn.execute(
            "INSERT OR IGNORE INTO settings (
                id, auto_start_enabled, sound_enabled, snooze_minutes,
                sticky_note_enabled, sticky_note_content, sticky_note_width, sticky_note_height,
                sticky_note_x, sticky_note_y, sticky_note_opacity, window_opacity,
                webdav_enabled, webdav_url, webdav_username, webdav_password, webdav_root_path,
                webdav_sync_interval_minutes, webdav_last_sync_time, webdav_last_local_change_time,
                webdav_last_sync_status, webdav_last_sync_error, webdav_device_id, notification_theme
            )
             VALUES (1, 0, 1, 5, 0, '', 360, 520, NULL, NULL, 0.95, 1.0, 0, '', '', '', '', 60, NULL, NULL, NULL, NULL, ?, 'app')",
            [Uuid::new_v4().to_string()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
