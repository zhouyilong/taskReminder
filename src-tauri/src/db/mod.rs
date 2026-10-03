//! SQLite 读写：`DbManager` 的连接池、初始化与凭据库在这里，各业务表的读写按模块拆分
//! （`tasks`、`recurring`、`records`、`sticky`、`settings`、`tombstones`、`import`、`migrations`）。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::NaiveDateTime;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::backup::ImportSummary;
use crate::errors::AppError;
use crate::kinds::{ReminderAction, ReminderKind, RepeatMode, TaskStatus, TaskType};
use crate::models::{
    default_quick_add_shortcut, normalize_completed_retention_days, normalize_priority,
    tags_from_db, tags_to_db, AppSettings, RecurringTask, ReminderRecord, StickyNote, Task,
    DEFAULT_COMPLETED_RETENTION_DAYS,
};
use crate::quiet_hours;
use crate::secrets::{self, SecretStore, Secrets};
use crate::time::{self, format_datetime, now_string, parse_datetime_any};

mod import;
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

/// 墓碑（软删除行）的保留天数。开启云同步时保留更久，
/// 让较长时间未同步的设备也能收到删除，而不是把旧数据重新上传“复活”。
pub const TOMBSTONE_RETENTION_DAYS_LOCAL: i64 = 7;
pub const TOMBSTONE_RETENTION_DAYS_SYNC: i64 = 60;

pub fn tombstone_retention_days(sync_enabled: bool) -> i64 {
    if sync_enabled {
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

#[derive(Clone)]
pub struct DbManager {
    pool: Pool<SqliteConnectionManager>,
    db_path: PathBuf,
    /// 存放 WebDAV 密码与同步密码的凭据库；None 时存本地数据库。
    secret_store: Option<Arc<dyn SecretStore>>,
    /// 已从凭据库读到（或写入）的密码，避免每次读写设置都访问凭据库。
    secret_cache: Arc<Mutex<Option<Secrets>>>,
}

impl DbManager {
    pub fn new(db_path: PathBuf) -> Result<Self, AppError> {
        // 测试不访问系统凭据库。
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
        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::new(manager).map_err(|e| AppError::Database(e.to_string()))?;
        let db = DbManager {
            pool,
            db_path,
            secret_store,
            secret_cache: Arc::new(Mutex::new(None)),
        };
        db.init()?;
        if let Err(err) = db.migrate_secrets_to_store() {
            eprintln!(
                "[secrets] 迁移密码到凭据库失败，继续使用本地数据库: {}",
                err
            );
        }
        Ok(db)
    }

    /// 把仍存在数据库中的密码移入凭据库（升级后首次启动、或上次写入凭据库失败时）。
    fn migrate_secrets_to_store(&self) -> Result<(), AppError> {
        let Some(store) = &self.secret_store else {
            return Ok(());
        };
        let conn = self.get_conn()?;
        let (storage, device_id, webdav_password, sync_passphrase): (String, String, String, String) =
            conn.query_row(
                "SELECT secret_storage, webdav_device_id, COALESCE(webdav_password, ''), sync_passphrase
                 FROM settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let plain = Secrets {
            webdav_password,
            sync_passphrase,
        };
        if storage == secrets::STORAGE_KEYRING || plain.is_empty() {
            return Ok(());
        }
        secrets::write_secrets(store.as_ref(), &device_id, &plain).map_err(AppError::System)?;
        conn.execute(
            "UPDATE settings SET webdav_password = '', sync_passphrase = '', secret_storage = ? WHERE id = 1",
            [secrets::STORAGE_KEYRING],
        )?;
        *self.secret_cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(plain);
        Ok(())
    }

    /// 凭据库中的密码：优先用缓存；读取失败时返回 None（设置中显示为空，同步会报认证失败）。
    fn keyring_secrets(&self, device_id: &str) -> Option<Secrets> {
        let store = self.secret_store.as_ref()?;
        let mut cache = self.secret_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = cache.as_ref() {
            return Some(cached.clone());
        }
        match secrets::read_secrets(store.as_ref(), device_id) {
            Ok(value) => {
                *cache = Some(value.clone());
                Some(value)
            }
            Err(err) => {
                eprintln!("[secrets] 读取凭据库失败: {}", err);
                None
            }
        }
    }

    /// 决定本次保存时密码的去向，返回（写入数据库的 WebDAV 密码, 同步密码, 存放位置）。
    fn resolve_secret_storage(
        &self,
        conn: &Connection,
        settings: &AppSettings,
    ) -> Result<(String, String, &'static str), AppError> {
        let incoming = Secrets {
            webdav_password: settings.webdav_password.clone(),
            sync_passphrase: settings.sync_passphrase.clone(),
        };
        let plain = |secrets: Secrets| {
            (
                secrets.webdav_password,
                secrets.sync_passphrase,
                secrets::STORAGE_DB,
            )
        };
        let Some(store) = &self.secret_store else {
            return Ok(plain(incoming));
        };
        let (storage, db_password, db_passphrase): (String, String, String) = conn.query_row(
            "SELECT secret_storage, COALESCE(webdav_password, ''), sync_passphrase FROM settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let mut cache = self.secret_cache.lock().unwrap_or_else(|e| e.into_inner());
        if storage == secrets::STORAGE_KEYRING {
            match cache.as_ref() {
                Some(cached) if *cached == incoming => {
                    return Ok((String::new(), String::new(), secrets::STORAGE_KEYRING))
                }
                // 凭据库读取失败时设置里的密码是空的，不能拿空值覆盖凭据库。
                None if incoming.is_empty() => {
                    return Ok((String::new(), String::new(), secrets::STORAGE_KEYRING))
                }
                _ => {}
            }
        } else if db_password == incoming.webdav_password
            && db_passphrase == incoming.sync_passphrase
        {
            // 密码没变：保持在数据库中，不在每次保存设置时重试凭据库（启动时会再迁移）。
            return Ok(plain(incoming));
        }
        match secrets::write_secrets(store.as_ref(), &settings.webdav_device_id, &incoming) {
            Ok(()) => {
                *cache = Some(incoming);
                Ok((String::new(), String::new(), secrets::STORAGE_KEYRING))
            }
            Err(err) => {
                eprintln!("[secrets] 写入凭据库失败，改存本地数据库: {}", err);
                *cache = None;
                Ok(plain(incoming))
            }
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.db_path.clone()
    }

    fn get_conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, AppError> {
        self.pool
            .get()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    fn init(&self) -> Result<(), AppError> {
        let mut conn = self.get_conn()?;
        self.apply_pragmas(&conn)?;
        self.ensure_version_table(&conn)?;
        self.apply_migrations(&mut conn)?;
        self.ensure_settings_row(&conn)?;
        // 设备 ID 用于同步锁与凭据库条目名，必须稳定：为空时生成一个并保存。
        conn.execute(
            "UPDATE settings SET webdav_device_id = ?
             WHERE id = 1 AND (webdav_device_id IS NULL OR TRIM(webdav_device_id) = '')",
            [Uuid::new_v4().to_string()],
        )?;
        Ok(())
    }

    fn apply_pragmas(&self, conn: &Connection) -> Result<(), AppError> {
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

    fn ensure_settings_row(&self, conn: &Connection) -> Result<(), AppError> {
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
