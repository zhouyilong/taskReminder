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
    default_quick_add_shortcut, normalize_priority, tags_from_db, tags_to_db, AppSettings,
    RecurringTask, ReminderRecord, StickyNote, Task,
};
use crate::quiet_hours;
use crate::secrets::{self, SecretStore, Secrets};
use crate::time::{self, format_datetime, now_string, parse_datetime_any};

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

fn normalize_sticky_note_opacity(opacity: Option<f64>) -> f64 {
    let value = opacity.filter(|v| v.is_finite()).unwrap_or(0.95);
    value.clamp(0.35, 1.0)
}

fn normalize_window_opacity(opacity: Option<f64>) -> f64 {
    let value = opacity.filter(|v| v.is_finite()).unwrap_or(1.0);
    value.clamp(0.3, 1.0)
}

const STICKY_NOTE_DEFAULT_POS_X: f64 = 48.0;
const STICKY_NOTE_DEFAULT_POS_Y: f64 = 76.0;
const STICKY_NOTE_ITEM_DEFAULT_WIDTH: f64 = 284.0;
const STICKY_NOTE_ITEM_DEFAULT_HEIGHT: f64 = 280.0;
const STICKY_NOTE_ITEM_MIN_WIDTH: f64 = 220.0;
const STICKY_NOTE_ITEM_MIN_HEIGHT: f64 = 180.0;

fn normalize_sticky_item_width(width: Option<f64>) -> f64 {
    width
        .filter(|value| value.is_finite())
        .unwrap_or(STICKY_NOTE_ITEM_DEFAULT_WIDTH)
        .max(STICKY_NOTE_ITEM_MIN_WIDTH)
}

fn normalize_sticky_item_height(height: Option<f64>) -> f64 {
    height
        .filter(|value| value.is_finite())
        .unwrap_or(STICKY_NOTE_ITEM_DEFAULT_HEIGHT)
        .max(STICKY_NOTE_ITEM_MIN_HEIGHT)
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

    fn ensure_version_table(&self, conn: &Connection) -> Result<(), AppError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                version TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                description TEXT
            );",
        )?;

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap_or(0);

        if count == 0 {
            conn.execute(
                "INSERT INTO schema_version (id, version, applied_at, description) VALUES (1, '0.0.0', ?, 'Initial')",
                [now_string()],
            )?;
        }
        Ok(())
    }

    fn apply_migrations(&self, conn: &mut Connection) -> Result<(), AppError> {
        let current = self.get_current_version(conn)?;
        let scripts = migration_scripts();

        for script in scripts {
            if compare_version(&script.version, &current) == std::cmp::Ordering::Greater {
                let tx = conn.transaction()?;
                execute_sql_script(&tx, script.sql)?;
                tx.execute(
                    "UPDATE schema_version SET version = ?, applied_at = ?, description = ? WHERE id = 1",
                    params![script.version, now_string(), script.description],
                )?;
                tx.commit()?;
            }
        }
        Ok(())
    }

    fn get_current_version(&self, conn: &Connection) -> Result<String, AppError> {
        let version: Option<String> = conn
            .query_row(
                "SELECT version FROM schema_version WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(version.unwrap_or_else(|| "0.0.0".to_string()))
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

    pub fn list_active_tasks(&self) -> Result<Vec<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, sticky_content, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at, tags, priority
             FROM tasks
             WHERE deleted_at IS NULL AND status != 'COMPLETED'
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], task_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn list_completed_tasks(&self) -> Result<Vec<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, sticky_content, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at, tags, priority
             FROM tasks
             WHERE deleted_at IS NULL AND status = 'COMPLETED'
             ORDER BY completed_at DESC",
        )?;
        let rows = stmt.query_map([], task_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_task(&self, task_id: &str) -> Result<Option<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, sticky_content, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at, tags, priority
             FROM tasks WHERE id = ?",
        )?;
        let task = stmt.query_row([task_id], task_from_row).optional()?;
        Ok(task)
    }

    pub fn list_recurring_tasks(&self) -> Result<Vec<RecurringTask>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at,
                    interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time,
                    repeat_mode, schedule_time, schedule_weekday, schedule_day, cron_expression,
                    updated_at, deleted_at, schedule_weekdays
             FROM recurring_tasks
             WHERE deleted_at IS NULL
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], recurring_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_recurring_task(&self, task_id: &str) -> Result<Option<RecurringTask>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at,
                    interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time,
                    repeat_mode, schedule_time, schedule_weekday, schedule_day, cron_expression,
                    updated_at, deleted_at, schedule_weekdays
             FROM recurring_tasks WHERE id = ?",
        )?;
        let task = stmt.query_row([task_id], recurring_from_row).optional()?;
        Ok(task)
    }

    pub fn list_reminder_records(&self) -> Result<Vec<ReminderRecord>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, reminder_id, description, type, trigger_time, close_time, action, updated_at, deleted_at
             FROM reminder_records
             WHERE deleted_at IS NULL
             ORDER BY trigger_time DESC",
        )?;
        let rows = stmt.query_map([], record_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_reminder_record(&self, record_id: &str) -> Result<Option<ReminderRecord>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, reminder_id, description, type, trigger_time, close_time, action, updated_at, deleted_at
             FROM reminder_records WHERE id = ?",
        )?;
        let record = stmt.query_row([record_id], record_from_row).optional()?;
        Ok(record)
    }

    /// 该提醒在 `since` 之后（含）是否已经触发过。
    /// 包含已软删除的记录：用户删掉提醒记录不代表希望再弹一次。
    pub fn has_reminder_record_since(
        &self,
        reminder_id: &str,
        since: &NaiveDateTime,
    ) -> Result<bool, AppError> {
        let conn = self.get_conn()?;
        let mut stmt =
            conn.prepare("SELECT trigger_time FROM reminder_records WHERE reminder_id = ?")?;
        let rows = stmt.query_map([reminder_id], |row| row.get::<_, String>(0))?;
        for trigger_time in rows.filter_map(Result::ok) {
            if parse_datetime_any(&trigger_time).is_some_and(|value| value >= *since) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(test)]
    pub fn create_task(
        &self,
        description: &str,
        sticky_content: Option<&str>,
    ) -> Result<Task, AppError> {
        self.create_task_with_meta(description, sticky_content, &TaskMeta::default())
    }

    pub fn create_task_with_meta(
        &self,
        description: &str,
        sticky_content: Option<&str>,
        meta: &TaskMeta,
    ) -> Result<Task, AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        let id = Uuid::new_v4().to_string();
        let note = sticky_content.unwrap_or("").trim().to_string();
        let tags = tags_to_db(&meta.tags);
        let priority = normalize_priority(meta.priority);
        conn.execute(
            "INSERT INTO tasks (id, description, type, status, created_at, completed_at, reminder_time, sticky_content, updated_at, deleted_at, tags, priority)
             VALUES (?, ?, 'ONE_TIME', 'PENDING', ?, NULL, NULL, ?, ?, NULL, ?, ?)",
            params![id, description, now, note, now, tags, priority],
        )?;
        Ok(Task {
            id,
            description: description.to_string(),
            sticky_content: if note.is_empty() { None } else { Some(note) },
            task_type: TaskType::OneTime,
            status: TaskStatus::Pending,
            created_at: now.clone(),
            completed_at: None,
            reminder_time: None,
            updated_at: Some(now),
            deleted_at: None,
            tags: tags_from_db(Some(tags)),
            priority,
        })
    }

    /// 更新待办。`meta` 为 `None` 时保留原有标签与优先级（如稍后提醒只改时间）。
    pub fn update_task(
        &self,
        task_id: &str,
        description: &str,
        sticky_content: Option<String>,
        reminder_time: Option<String>,
        meta: Option<&TaskMeta>,
    ) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        let note = sticky_content
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        let tags = meta.map(|meta| tags_to_db(&meta.tags));
        let priority = meta.map(|meta| normalize_priority(meta.priority));
        conn.execute(
            "UPDATE tasks
             SET description = ?, sticky_content = ?, reminder_time = ?,
                 tags = COALESCE(?, tags), priority = COALESCE(?, priority), updated_at = ?
             WHERE id = ?",
            params![
                description,
                note,
                reminder_time,
                tags,
                priority,
                now,
                task_id
            ],
        )?;
        Ok(())
    }

    pub fn complete_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            // 完成后便签随之关闭：已完成的待办不再恢复便签窗口，取消完成也不会自动重新打开。
            "UPDATE tasks SET status = 'COMPLETED', completed_at = ?, sticky_is_open = 0, updated_at = ? WHERE id = ?",
            params![now, now, task_id],
        )?;
        Ok(())
    }

    pub fn uncomplete_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks SET status = 'PENDING', completed_at = NULL, updated_at = ? WHERE id = ?",
            params![now, task_id],
        )?;
        Ok(())
    }

    pub fn delete_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            // 同时关闭便签，从回收站恢复时不会突然弹出便签窗口。
            "UPDATE tasks SET deleted_at = ?, sticky_is_open = 0, updated_at = ? WHERE id = ?",
            params![now, now, task_id],
        )?;
        Ok(())
    }

    /// 回收站中的待办：已软删除且仍在墓碑保留期内的行，按删除时间倒序。
    pub fn list_deleted_tasks(&self, retention_days: i64) -> Result<Vec<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, sticky_content, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at, tags, priority
             FROM tasks
             WHERE deleted_at IS NOT NULL AND deleted_at >= ?
             ORDER BY deleted_at DESC",
        )?;
        let cutoff = tombstone_cutoff(retention_days);
        let rows = stmt.query_map([cutoff.as_str()], task_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// 从回收站恢复待办。只清除墓碑，保留原来的完成状态与提醒时间。
    pub fn restore_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks SET deleted_at = NULL, updated_at = ? WHERE id = ? AND deleted_at IS NOT NULL",
            params![now, task_id],
        )?;
        Ok(())
    }

    pub fn create_recurring_task(&self, task: &RecurringTask) -> Result<RecurringTask, AppError> {
        let conn = self.get_conn()?;
        let id = Uuid::new_v4().to_string();
        let now = now_string();
        conn.execute(
            "INSERT INTO recurring_tasks (
                id, description, type, status, created_at, completed_at, interval_minutes,
                last_triggered, next_trigger, is_paused, start_time, end_time,
                repeat_mode, schedule_time, schedule_weekday, schedule_day, cron_expression,
                updated_at, deleted_at, schedule_weekdays
            )
             VALUES (?, ?, 'RECURRING', 'PENDING', ?, NULL, ?, NULL, ?, 0, ?, ?,
                     ?, ?, ?, ?, ?, ?, NULL, ?)",
            params![
                id,
                task.description.as_str(),
                now,
                task.interval_minutes,
                task.next_trigger.as_str(),
                task.start_time.as_deref(),
                task.end_time.as_deref(),
                task.repeat_mode.as_str(),
                task.schedule_time.as_deref(),
                task.schedule_weekday,
                task.schedule_day,
                task.cron_expression.as_deref(),
                now,
                task.schedule_weekdays
            ],
        )?;
        Ok(RecurringTask {
            id,
            description: task.description.clone(),
            task_type: TaskType::Recurring,
            status: TaskStatus::Pending,
            created_at: now.clone(),
            completed_at: None,
            reminder_time: None,
            updated_at: Some(now),
            deleted_at: None,
            interval_minutes: task.interval_minutes,
            last_triggered: None,
            next_trigger: task.next_trigger.clone(),
            is_paused: false,
            start_time: task.start_time.clone(),
            end_time: task.end_time.clone(),
            repeat_mode: task.repeat_mode.clone(),
            schedule_time: task.schedule_time.clone(),
            schedule_weekday: task.schedule_weekday,
            schedule_weekdays: task.schedule_weekdays,
            schedule_day: task.schedule_day,
            cron_expression: task.cron_expression.clone(),
        })
    }

    pub fn update_recurring_task(&self, task: &RecurringTask) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE recurring_tasks
             SET description = ?, interval_minutes = ?, start_time = ?, end_time = ?,
                 repeat_mode = ?, schedule_time = ?, schedule_weekday = ?, schedule_weekdays = ?,
                 schedule_day = ?, cron_expression = ?,
                 is_paused = ?, next_trigger = ?, last_triggered = ?, updated_at = ?
             WHERE id = ?",
            params![
                task.description.as_str(),
                task.interval_minutes,
                task.start_time.as_deref(),
                task.end_time.as_deref(),
                task.repeat_mode.as_str(),
                task.schedule_time.as_deref(),
                task.schedule_weekday,
                task.schedule_weekdays,
                task.schedule_day,
                task.cron_expression.as_deref(),
                if task.is_paused { 1 } else { 0 },
                task.next_trigger.as_str(),
                task.last_triggered.as_deref(),
                now,
                task.id.as_str()
            ],
        )?;
        Ok(())
    }

    pub fn pause_recurring_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE recurring_tasks SET is_paused = 1, updated_at = ? WHERE id = ?",
            params![now, task_id],
        )?;
        Ok(())
    }

    pub fn delete_recurring_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE recurring_tasks SET deleted_at = ?, updated_at = ? WHERE id = ?",
            params![now, now, task_id],
        )?;
        Ok(())
    }

    pub fn list_deleted_recurring_tasks(
        &self,
        retention_days: i64,
    ) -> Result<Vec<RecurringTask>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at,
                    interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time,
                    repeat_mode, schedule_time, schedule_weekday, schedule_day, cron_expression,
                    updated_at, deleted_at, schedule_weekdays
             FROM recurring_tasks
             WHERE deleted_at IS NOT NULL AND deleted_at >= ?
             ORDER BY deleted_at DESC",
        )?;
        let cutoff = tombstone_cutoff(retention_days);
        let rows = stmt.query_map([cutoff.as_str()], recurring_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// 从回收站恢复循环提醒。调用方负责重新计算下次触发时间并写回。
    pub fn restore_recurring_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE recurring_tasks SET deleted_at = NULL, updated_at = ? WHERE id = ? AND deleted_at IS NOT NULL",
            params![now, task_id],
        )?;
        Ok(())
    }

    /// 在回收站中“永久删除”：把墓碑的 `deleted_at` 改为已过期的时间并刷新 `updated_at`。
    ///
    /// 不直接物理删除：开启同步时远端仍有较新的墓碑，合并后会重新出现在回收站。
    /// 刷新 `updated_at` 让本地行在合并中胜出，随后由 `purge_expired_tombstones`
    /// 在本地与远端一起删除。未开启同步时调用方可以立即清理。
    pub fn expire_tombstones(&self, table: TrashTable, ids: &[String]) -> Result<(), AppError> {
        let mut conn = self.get_conn()?;
        let now = now_string();
        let sql = format!(
            "UPDATE {} SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NOT NULL",
            table.name()
        );
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute(&sql, params![EXPIRED_TOMBSTONE_TIME, now, id])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn create_reminder_record(
        &self,
        reminder_id: &str,
        description: &str,
        reminder_type: ReminderKind,
    ) -> Result<ReminderRecord, AppError> {
        let conn = self.get_conn()?;
        let id = Uuid::new_v4().to_string();
        let now = now_string();
        conn.execute(
            "INSERT INTO reminder_records (id, reminder_id, description, type, trigger_time, close_time, action, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, NULL, 'PENDING', ?, NULL)",
            params![id, reminder_id, description, &reminder_type, now, now],
        )?;
        Ok(ReminderRecord {
            id,
            reminder_id: reminder_id.to_string(),
            description: description.to_string(),
            reminder_type,
            trigger_time: now.clone(),
            close_time: None,
            action: ReminderAction::Pending,
            updated_at: Some(now),
            deleted_at: None,
        })
    }

    pub fn update_reminder_record_action(
        &self,
        record_id: &str,
        action: ReminderAction,
    ) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE reminder_records SET action = ?, close_time = ?, updated_at = ? WHERE id = ?",
            params![&action, now, now, record_id],
        )?;
        Ok(())
    }

    pub fn delete_reminder_record(&self, record_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE reminder_records SET deleted_at = ?, updated_at = ? WHERE id = ?",
            params![now, now, record_id],
        )?;
        Ok(())
    }

    pub fn delete_reminder_records(&self, ids: &[String]) -> Result<(), AppError> {
        let mut conn = self.get_conn()?;
        let now = now_string();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE reminder_records SET deleted_at = ?, updated_at = ? WHERE id = ?",
                params![now, now, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_sticky_notes(&self) -> Result<Vec<StickyNote>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, sticky_content, sticky_pos_x, sticky_pos_y, sticky_width, sticky_height, sticky_is_open, sticky_is_pinned, created_at, updated_at, reminder_time
             FROM tasks
             WHERE deleted_at IS NULL AND status != 'COMPLETED'
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], sticky_note_from_task_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_sticky_note(&self, note_id: &str) -> Result<Option<StickyNote>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, sticky_content, sticky_pos_x, sticky_pos_y, sticky_width, sticky_height, sticky_is_open, sticky_is_pinned, created_at, updated_at, reminder_time
             FROM tasks
             WHERE id = ?",
        )?;
        stmt.query_row([note_id], sticky_note_from_task_row)
            .optional()
            .map_err(AppError::from)
    }

    pub fn open_sticky_note(
        &self,
        note_id: &str,
        title: Option<String>,
        default_x: Option<f64>,
        default_y: Option<f64>,
    ) -> Result<StickyNote, AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        let existing = self.get_sticky_note(note_id)?;
        let keep_existing_position = existing
            .as_ref()
            .map(|note| {
                note.is_open
                    || (note.pos_x - STICKY_NOTE_DEFAULT_POS_X).abs() > f64::EPSILON
                    || (note.pos_y - STICKY_NOTE_DEFAULT_POS_Y).abs() > f64::EPSILON
            })
            .unwrap_or(false);
        let x = if keep_existing_position {
            existing
                .as_ref()
                .map(|note| note.pos_x)
                .unwrap_or(STICKY_NOTE_DEFAULT_POS_X)
        } else {
            default_x
                .filter(|value| value.is_finite())
                .or_else(|| existing.as_ref().map(|note| note.pos_x))
                .unwrap_or(STICKY_NOTE_DEFAULT_POS_X)
        };
        // 负坐标（主屏左侧 / 上方的副屏）原样保留；屏幕外的位置在显示窗口时校正。
        let y = if keep_existing_position {
            existing
                .as_ref()
                .map(|note| note.pos_y)
                .unwrap_or(STICKY_NOTE_DEFAULT_POS_Y)
        } else {
            default_y
                .filter(|value| value.is_finite())
                .or_else(|| existing.as_ref().map(|note| note.pos_y))
                .unwrap_or(STICKY_NOTE_DEFAULT_POS_Y)
        };
        let width = existing
            .as_ref()
            .map(|note| normalize_sticky_item_width(Some(note.width)))
            .unwrap_or(STICKY_NOTE_ITEM_DEFAULT_WIDTH);
        let height = existing
            .as_ref()
            .map(|note| normalize_sticky_item_height(Some(note.height)))
            .unwrap_or(STICKY_NOTE_ITEM_DEFAULT_HEIGHT);
        let resolved_title = title
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| existing.as_ref().map(|note| note.title.clone()))
            .unwrap_or_else(|| "待办便签".to_string());
        conn.execute(
            "UPDATE tasks
             SET description = ?, sticky_pos_x = ?, sticky_pos_y = ?, sticky_width = ?, sticky_height = ?, sticky_is_open = 1, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
            params![resolved_title, x, y, width, height, now, note_id],
        )?;
        if let Some(note) = existing {
            return Ok(StickyNote {
                title: resolved_title,
                pos_x: x,
                pos_y: y,
                width,
                height,
                is_open: true,
                updated_at: now,
                ..note
            });
        }
        self.get_sticky_note(note_id)?
            .ok_or_else(|| AppError::Database("找不到对应待办".to_string()))
    }

    pub fn create_custom_sticky_note(
        &self,
        title: &str,
        content: Option<&str>,
        default_x: Option<f64>,
        default_y: Option<f64>,
        default_width: Option<f64>,
        default_height: Option<f64>,
    ) -> Result<StickyNote, AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        let id = Uuid::new_v4().to_string();
        let x = default_x
            .filter(|value| value.is_finite())
            .unwrap_or(STICKY_NOTE_DEFAULT_POS_X)
            .max(0.0);
        let y = default_y
            .filter(|value| value.is_finite())
            .unwrap_or(STICKY_NOTE_DEFAULT_POS_Y)
            .max(0.0);
        let width = default_width
            .filter(|v| v.is_finite() && *v >= STICKY_NOTE_ITEM_MIN_WIDTH)
            .unwrap_or(STICKY_NOTE_ITEM_DEFAULT_WIDTH);
        let height = default_height
            .filter(|v| v.is_finite() && *v >= STICKY_NOTE_ITEM_MIN_HEIGHT)
            .unwrap_or(STICKY_NOTE_ITEM_DEFAULT_HEIGHT);
        let resolved_title = if title.trim().is_empty() {
            "新便签".to_string()
        } else {
            title.trim().to_string()
        };
        let resolved_content = content.unwrap_or("").to_string();
        conn.execute(
            "INSERT INTO tasks (
                id, description, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at,
                sticky_content, sticky_pos_x, sticky_pos_y, sticky_width, sticky_height, sticky_is_open
            )
             VALUES (?, ?, 'ONE_TIME', 'PENDING', ?, NULL, NULL, ?, NULL, ?, ?, ?, ?, ?, 1)",
            params![id, resolved_title, now, now, resolved_content, x, y, width, height],
        )?;
        Ok(StickyNote {
            task_id: id,
            title: resolved_title,
            note_type: "TASK".to_string(),
            content: resolved_content,
            pos_x: x,
            pos_y: y,
            width,
            height,
            is_open: true,
            is_pinned: false,
            created_at: now.clone(),
            updated_at: now,
            reminder_time: None,
        })
    }

    pub fn save_sticky_note_content(&self, task_id: &str, content: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks
             SET sticky_content = ?, sticky_is_open = 1, updated_at = ?
             WHERE id = ?",
            params![content, now, task_id],
        )?;
        Ok(())
    }

    pub fn update_sticky_note_title(&self, task_id: &str, title: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        let resolved_title = if title.trim().is_empty() {
            "便签".to_string()
        } else {
            title.trim().to_string()
        };
        conn.execute(
            "UPDATE tasks
             SET description = ?,
                 sticky_content = CASE
                     WHEN TRIM(COALESCE(sticky_content, '')) = '' THEN ?
                     ELSE sticky_content
                 END,
                 sticky_is_open = 1,
                 updated_at = ?
             WHERE id = ?",
            params![resolved_title, resolved_title, now, task_id],
        )?;
        Ok(())
    }

    pub fn set_task_reminder_time(
        &self,
        task_id: &str,
        reminder_time: Option<&str>,
    ) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks SET reminder_time = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            params![reminder_time, now, task_id],
        )?;
        Ok(())
    }

    pub fn move_sticky_note(&self, task_id: &str, x: f64, y: f64) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks
             SET sticky_pos_x = ?1, sticky_pos_y = ?2, sticky_is_open = 1, updated_at = ?3
             WHERE id = ?4
               AND (ABS(sticky_pos_x - ?1) >= 0.5 OR ABS(sticky_pos_y - ?2) >= 0.5 OR sticky_is_open != 1)",
            params![x, y, now, task_id],
        )?;
        Ok(())
    }

    pub fn resize_sticky_note(
        &self,
        task_id: &str,
        width: f64,
        height: f64,
    ) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks
             SET sticky_width = ?1, sticky_height = ?2, sticky_is_open = 1, updated_at = ?3
             WHERE id = ?4
               AND (ABS(sticky_width - ?1) >= 0.5 OR ABS(sticky_height - ?2) >= 0.5 OR sticky_is_open != 1)",
            params![
                normalize_sticky_item_width(Some(width)),
                normalize_sticky_item_height(Some(height)),
                now,
                task_id
            ],
        )?;
        Ok(())
    }

    /// 是否开启便签贴边吸附。只读一列，拖动结束时调用，避免 `load_settings` 访问凭据库。
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn sticky_snap_enabled(&self) -> Result<bool, AppError> {
        let conn = self.get_conn()?;
        let value: Option<i64> = conn
            .query_row(
                "SELECT sticky_snap_enabled FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.unwrap_or(1) == 1)
    }

    /// 是否开启节假日数据在线更新。只读一列，后台检查时调用，避免 `load_settings` 访问凭据库。
    pub fn holiday_auto_update_enabled(&self) -> Result<bool, AppError> {
        let conn = self.get_conn()?;
        let value: Option<i64> = conn
            .query_row(
                "SELECT holiday_auto_update FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.unwrap_or(1) == 1)
    }

    pub fn set_sticky_note_pinned(&self, task_id: &str, pinned: bool) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks
             SET sticky_is_pinned = ?, sticky_is_open = 1, updated_at = ?
             WHERE id = ?",
            params![if pinned { 1 } else { 0 }, now, task_id],
        )?;
        Ok(())
    }

    pub fn get_sticky_note_pinned(&self, task_id: &str) -> Result<bool, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT sticky_is_pinned
             FROM tasks
             WHERE id = ?",
        )?;
        let pinned = stmt
            .query_row([task_id], |row| row.get::<_, Option<i64>>(0))
            .optional()?
            .flatten()
            .unwrap_or(0);
        Ok(pinned == 1)
    }

    pub fn close_sticky_note(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks SET sticky_is_open = 0, updated_at = ? WHERE id = ?",
            params![now, task_id],
        )?;
        Ok(())
    }

    pub fn load_settings(&self) -> Result<AppSettings, AppError> {
        let conn = self.get_conn()?;
        let sql = "SELECT auto_start_enabled, sound_enabled, snooze_minutes,
                   sticky_note_enabled, sticky_note_content, sticky_note_width, sticky_note_height, sticky_note_x, sticky_note_y, sticky_note_opacity, window_opacity,
                   webdav_enabled, webdav_url, webdav_username, webdav_password,
                   webdav_root_path, webdav_sync_interval_minutes, webdav_last_sync_time,
                   webdav_last_local_change_time, webdav_last_sync_status, webdav_last_sync_error,
                   webdav_device_id, notification_theme, quick_add_enabled, quick_add_shortcut,
                   sync_encryption_enabled, sync_passphrase,
                   quiet_hours_enabled, quiet_hours_start, quiet_hours_end,
                   native_notification_enabled, sticky_toggle_shortcut, secret_storage,
                   sticky_snap_enabled, holiday_auto_update
                   FROM settings WHERE id = 1";
        let mut stmt = conn.prepare(sql)?;
        let row = stmt.query_row([], |row| {
            let sticky_note_width = row.get::<_, Option<i64>>(5)?.unwrap_or(360).max(260);
            let sticky_note_height = row.get::<_, Option<i64>>(6)?.unwrap_or(520).max(320);
            let sticky_note_opacity = normalize_sticky_note_opacity(row.get(9)?);
            let window_opacity = normalize_window_opacity(row.get(10)?);
            let webdav_url: String = row.get::<_, Option<String>>(12)?.unwrap_or_default();
            let webdav_username: String = row.get::<_, Option<String>>(13)?.unwrap_or_default();
            let webdav_password: String = row.get::<_, Option<String>>(14)?.unwrap_or_default();
            let webdav_root_path: String = row.get::<_, Option<String>>(15)?.unwrap_or_default();
            let webdav_device_id: String = row
                .get::<_, Option<String>>(21)?
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let notification_theme: String = row
                .get::<_, Option<String>>(22)?
                .unwrap_or_else(|| "app".to_string());
            let quick_add_shortcut: String = row
                .get::<_, Option<String>>(24)?
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(default_quick_add_shortcut);
            Ok(AppSettings {
                auto_start_enabled: row.get::<_, i64>(0)? == 1,
                sound_enabled: row.get::<_, i64>(1)? == 1,
                snooze_minutes: row.get(2)?,
                sticky_note_enabled: row.get::<_, i64>(3)? == 1,
                sticky_note_content: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                sticky_note_width,
                sticky_note_height,
                sticky_note_x: row.get(7)?,
                sticky_note_y: row.get(8)?,
                sticky_note_opacity,
                window_opacity,
                webdav_enabled: row.get::<_, i64>(11)? == 1,
                webdav_url,
                webdav_username,
                webdav_password,
                webdav_root_path,
                webdav_sync_interval_minutes: row.get(16)?,
                webdav_last_sync_time: row.get(17)?,
                webdav_last_local_change_time: row.get(18)?,
                webdav_last_sync_status: row.get(19)?,
                webdav_last_sync_error: row.get(20)?,
                webdav_device_id,
                notification_theme,
                quick_add_enabled: row.get::<_, Option<i64>>(23)?.unwrap_or(1) == 1,
                quick_add_shortcut,
                sync_encryption_enabled: row.get::<_, Option<i64>>(25)?.unwrap_or(0) == 1,
                sync_passphrase: row.get::<_, Option<String>>(26)?.unwrap_or_default(),
                quiet_hours_enabled: row.get::<_, Option<i64>>(27)?.unwrap_or(0) == 1,
                quiet_hours_start: quiet_hours::normalize_clock(
                    &row.get::<_, Option<String>>(28)?.unwrap_or_default(),
                    quiet_hours::DEFAULT_START,
                ),
                quiet_hours_end: quiet_hours::normalize_clock(
                    &row.get::<_, Option<String>>(29)?.unwrap_or_default(),
                    quiet_hours::DEFAULT_END,
                ),
                native_notification_enabled: row.get::<_, Option<i64>>(30)?.unwrap_or(0) == 1,
                sticky_toggle_shortcut: row
                    .get::<_, Option<String>>(31)?
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                secret_storage: row
                    .get::<_, Option<String>>(32)?
                    .unwrap_or_else(|| secrets::STORAGE_DB.to_string()),
                sticky_snap_enabled: row.get::<_, Option<i64>>(33)?.unwrap_or(1) == 1,
                holiday_auto_update: row.get::<_, Option<i64>>(34)?.unwrap_or(1) == 1,
            })
        })?;
        let mut settings = row;
        if settings.secret_storage == secrets::STORAGE_KEYRING {
            if let Some(stored) = self.keyring_secrets(&settings.webdav_device_id) {
                settings.webdav_password = stored.webdav_password;
                settings.sync_passphrase = stored.sync_passphrase;
            }
        }
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let (webdav_password, sync_passphrase, secret_storage) =
            self.resolve_secret_storage(&conn, settings)?;
        conn.execute(
            "UPDATE settings
             SET auto_start_enabled = ?, sound_enabled = ?, snooze_minutes = ?,
                 sticky_note_enabled = ?, sticky_note_content = ?, sticky_note_width = ?, sticky_note_height = ?,
                 sticky_note_x = ?, sticky_note_y = ?, sticky_note_opacity = ?, window_opacity = ?,
                 webdav_enabled = ?, webdav_url = ?, webdav_username = ?, webdav_password = ?,
                 webdav_root_path = ?, webdav_sync_interval_minutes = ?, webdav_last_sync_time = ?,
                 webdav_last_local_change_time = ?, webdav_last_sync_status = ?, webdav_last_sync_error = ?,
                 webdav_device_id = ?, notification_theme = ?,
                 quick_add_enabled = ?, quick_add_shortcut = ?,
                 sync_encryption_enabled = ?, sync_passphrase = ?,
                 quiet_hours_enabled = ?, quiet_hours_start = ?, quiet_hours_end = ?,
                 native_notification_enabled = ?, sticky_toggle_shortcut = ?, secret_storage = ?,
                 sticky_snap_enabled = ?, holiday_auto_update = ?
             WHERE id = 1",
            params![
                if settings.auto_start_enabled { 1 } else { 0 },
                if settings.sound_enabled { 1 } else { 0 },
                settings.snooze_minutes,
                if settings.sticky_note_enabled { 1 } else { 0 },
                settings.sticky_note_content,
                settings.sticky_note_width.max(260),
                settings.sticky_note_height.max(320),
                settings.sticky_note_x,
                settings.sticky_note_y,
                normalize_sticky_note_opacity(Some(settings.sticky_note_opacity)),
                normalize_window_opacity(Some(settings.window_opacity)),
                if settings.webdav_enabled { 1 } else { 0 },
                settings.webdav_url,
                settings.webdav_username,
                webdav_password,
                settings.webdav_root_path,
                settings.webdav_sync_interval_minutes,
                settings.webdav_last_sync_time,
                settings.webdav_last_local_change_time,
                settings.webdav_last_sync_status,
                settings.webdav_last_sync_error,
                settings.webdav_device_id,
                settings.notification_theme,
                if settings.quick_add_enabled { 1 } else { 0 },
                settings.quick_add_shortcut.trim(),
                if settings.sync_encryption_enabled { 1 } else { 0 },
                sync_passphrase,
                if settings.quiet_hours_enabled { 1 } else { 0 },
                quiet_hours::normalize_clock(&settings.quiet_hours_start, quiet_hours::DEFAULT_START),
                quiet_hours::normalize_clock(&settings.quiet_hours_end, quiet_hours::DEFAULT_END),
                if settings.native_notification_enabled { 1 } else { 0 },
                settings.sticky_toggle_shortcut.trim(),
                secret_storage,
                if settings.sticky_snap_enabled { 1 } else { 0 },
                if settings.holiday_auto_update { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn update_sync_status(
        &self,
        status: &str,
        error: Option<String>,
    ) -> Result<AppSettings, AppError> {
        let mut settings = self.load_settings()?;
        let now = now_string();
        settings.webdav_last_sync_time = Some(now.clone());
        settings.webdav_last_sync_status = Some(status.to_string());
        settings.webdav_last_sync_error = error;
        self.save_settings(&settings)?;
        Ok(settings)
    }

    pub fn mark_local_change(&self) -> Result<(), AppError> {
        let mut settings = self.load_settings()?;
        settings.webdav_last_local_change_time = Some(now_string());
        self.save_settings(&settings)?;
        Ok(())
    }

    /// 定期清理：
    /// 1. 超过 30 天、或超出最近 100 条的已完成任务转为墓碑（软删除），
    ///    而不是直接物理删除——否则云同步合并时远端仍有该行，会被重新插回本地；
    /// 2. 物理删除超过保留期的墓碑。
    pub fn cleanup_data(&self, tombstone_retention_days: i64) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = time::now();
        let now_text = format_datetime(&now);
        let completed_cutoff = format_datetime(&(now - chrono::Duration::days(30)));

        conn.execute(
            "UPDATE tasks SET deleted_at = ?1, updated_at = ?1
             WHERE status = 'COMPLETED' AND deleted_at IS NULL
               AND completed_at IS NOT NULL AND completed_at < ?2",
            params![now_text, completed_cutoff],
        )?;
        conn.execute(
            "UPDATE tasks SET deleted_at = ?1, updated_at = ?1 WHERE id IN (
                SELECT id FROM tasks
                WHERE status = 'COMPLETED' AND deleted_at IS NULL
                ORDER BY completed_at DESC
                LIMIT -1 OFFSET 100
            )",
            params![now_text],
        )?;
        drop(conn);
        self.purge_expired_tombstones(tombstone_retention_days)?;
        Ok(())
    }

    /// 物理删除 `deleted_at` 早于保留期的墓碑行。
    ///
    /// 云同步在合并后、上传前也会调用它，使本地与远端一起删除墓碑；
    /// 若只在本地删除，下一次合并又会把远端的墓碑插回来。
    pub fn purge_expired_tombstones(&self, retention_days: i64) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let cutoff = tombstone_cutoff(retention_days);
        for table in ["tasks", "recurring_tasks", "reminder_records"] {
            conn.execute(
                &format!(
                    "DELETE FROM {} WHERE deleted_at IS NOT NULL AND deleted_at < ?",
                    table
                ),
                [cutoff.as_str()],
            )?;
        }
        Ok(())
    }

    /// 按 id 合并导入的数据：本地没有的插入；本地已有时只有导入的版本更新（`updated_at` 更晚）才覆盖。
    /// 与云同步的“较新的版本胜出”一致，不会覆盖本地更新的修改，也不会复活本地更晚删除的行。
    pub fn import_rows(
        &self,
        tasks: &[Task],
        recurring: &[RecurringTask],
        records: &[ReminderRecord],
    ) -> Result<ImportSummary, AppError> {
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        let mut summary = ImportSummary::default();

        for task in tasks {
            let valid = !task.id.trim().is_empty()
                && !task.description.trim().is_empty()
                && task.status.is_known();
            let outcome = if valid {
                import_outcome(&tx, "tasks", &task.id, &task.updated_at, &task.created_at)?
            } else {
                ImportOutcome::Skipped
            };
            if outcome == ImportOutcome::Skipped {
                summary.skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT INTO tasks (id, description, sticky_content, type, status, created_at, completed_at,
                                    reminder_time, updated_at, deleted_at, tags, priority)
                 VALUES (?1, ?2, ?3, 'ONE_TIME', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET
                    description = excluded.description, sticky_content = excluded.sticky_content,
                    status = excluded.status, created_at = excluded.created_at,
                    completed_at = excluded.completed_at, reminder_time = excluded.reminder_time,
                    updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                    tags = excluded.tags, priority = excluded.priority",
                params![
                    task.id,
                    task.description.trim(),
                    task.sticky_content.clone().unwrap_or_default(),
                    task.status,
                    task.created_at,
                    task.completed_at,
                    task.reminder_time,
                    task.updated_at.clone().unwrap_or_else(|| task.created_at.clone()),
                    task.deleted_at,
                    tags_to_db(&task.tags),
                    normalize_priority(task.priority),
                ],
            )?;
            summary.count(outcome);
        }

        for task in recurring {
            let valid = !task.id.trim().is_empty() && !task.description.trim().is_empty();
            let outcome = if valid {
                import_outcome(
                    &tx,
                    "recurring_tasks",
                    &task.id,
                    &task.updated_at,
                    &task.created_at,
                )?
            } else {
                ImportOutcome::Skipped
            };
            if outcome == ImportOutcome::Skipped {
                summary.skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO recurring_tasks (
                    id, description, type, status, created_at, completed_at, interval_minutes,
                    last_triggered, next_trigger, is_paused, start_time, end_time,
                    repeat_mode, schedule_time, schedule_weekday, schedule_day, cron_expression,
                    updated_at, deleted_at, schedule_weekdays
                 )
                 VALUES (?, ?, 'RECURRING', 'PENDING', ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    task.id,
                    task.description.trim(),
                    task.created_at,
                    task.interval_minutes.max(1),
                    task.last_triggered,
                    task.next_trigger,
                    if task.is_paused { 1 } else { 0 },
                    task.start_time,
                    task.end_time,
                    task.repeat_mode,
                    task.schedule_time,
                    task.schedule_weekday,
                    task.schedule_day,
                    task.cron_expression,
                    task.updated_at.clone().unwrap_or_else(|| task.created_at.clone()),
                    task.deleted_at,
                    task.schedule_weekdays,
                ],
            )?;
            summary.count(outcome);
        }

        for record in records {
            let valid = !record.id.trim().is_empty() && !record.reminder_id.trim().is_empty();
            let outcome = if valid {
                import_outcome(
                    &tx,
                    "reminder_records",
                    &record.id,
                    &record.updated_at,
                    &record.trigger_time,
                )?
            } else {
                ImportOutcome::Skipped
            };
            if outcome == ImportOutcome::Skipped {
                summary.skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO reminder_records (
                    id, reminder_id, description, type, trigger_time, close_time, action, updated_at, deleted_at
                 )
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    record.id,
                    record.reminder_id,
                    record.description,
                    record.reminder_type,
                    record.trigger_time,
                    record.close_time,
                    record.action,
                    record.updated_at.clone().unwrap_or_else(|| record.trigger_time.clone()),
                    record.deleted_at,
                ],
            )?;
            summary.count(outcome);
        }

        tx.commit()?;
        Ok(summary)
    }

    pub fn optimize_database(&self) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        conn.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);\n             ANALYZE;\n             PRAGMA optimize;",
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImportOutcome {
    Inserted,
    Updated,
    Skipped,
}

impl ImportSummary {
    fn count(&mut self, outcome: ImportOutcome) {
        match outcome {
            ImportOutcome::Inserted => self.inserted += 1,
            ImportOutcome::Updated => self.updated += 1,
            ImportOutcome::Skipped => self.skipped += 1,
        }
    }
}

/// 比较导入行与本地行的更新时间，决定插入、覆盖还是跳过。
fn import_outcome(
    conn: &Connection,
    table: &str,
    id: &str,
    updated_at: &Option<String>,
    fallback_time: &str,
) -> Result<ImportOutcome, AppError> {
    let local: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            &format!("SELECT updated_at, deleted_at FROM {} WHERE id = ?", table),
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((local_updated, local_deleted)) = local else {
        return Ok(ImportOutcome::Inserted);
    };
    let incoming = updated_at
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_time);
    let existing = local_updated.or(local_deleted);
    match (
        parse_datetime_any(incoming),
        existing.as_deref().and_then(parse_datetime_any),
    ) {
        (Some(incoming), Some(existing)) if incoming > existing => Ok(ImportOutcome::Updated),
        (Some(_), None) => Ok(ImportOutcome::Updated),
        _ => Ok(ImportOutcome::Skipped),
    }
}

/// 待办的组织信息：标签与优先级。
#[derive(Clone, Debug, Default)]
pub struct TaskMeta {
    pub tags: Vec<String>,
    pub priority: i64,
}

fn task_from_row(row: &rusqlite::Row<'_>) -> Result<Task, rusqlite::Error> {
    Ok(Task {
        id: row.get(0)?,
        description: row.get(1)?,
        sticky_content: row.get::<_, Option<String>>(2)?,
        task_type: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
        completed_at: row.get(6)?,
        reminder_time: row.get(7)?,
        updated_at: row.get(8)?,
        deleted_at: row.get(9)?,
        tags: tags_from_db(row.get::<_, Option<String>>(10)?),
        priority: normalize_priority(row.get::<_, Option<i64>>(11)?.unwrap_or(0)),
    })
}

fn recurring_from_row(row: &rusqlite::Row<'_>) -> Result<RecurringTask, rusqlite::Error> {
    Ok(RecurringTask {
        id: row.get(0)?,
        description: row.get(1)?,
        task_type: row.get(2)?,
        status: row.get(3)?,
        created_at: row.get(4)?,
        completed_at: row.get(5)?,
        reminder_time: None,
        interval_minutes: row.get(6)?,
        last_triggered: row.get(7)?,
        next_trigger: row.get(8)?,
        is_paused: row.get::<_, i64>(9)? == 1,
        start_time: row.get(10)?,
        end_time: row.get(11)?,
        // 空值按区间间隔（V1.4.1 迁移的默认值），不认识的模式原样保留。
        repeat_mode: row
            .get::<_, Option<RepeatMode>>(12)?
            .unwrap_or(RepeatMode::IntervalRange),
        schedule_time: row.get(13)?,
        schedule_weekday: row.get(14)?,
        schedule_weekdays: row.get(19)?,
        schedule_day: row.get(15)?,
        cron_expression: row.get(16)?,
        updated_at: row.get(17)?,
        deleted_at: row.get(18)?,
    })
}

fn record_from_row(row: &rusqlite::Row<'_>) -> Result<ReminderRecord, rusqlite::Error> {
    Ok(ReminderRecord {
        id: row.get(0)?,
        reminder_id: row.get(1)?,
        description: row.get(2)?,
        reminder_type: row.get(3)?,
        trigger_time: row.get(4)?,
        close_time: row.get(5)?,
        action: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
    })
}

fn sticky_note_from_task_row(row: &rusqlite::Row<'_>) -> Result<StickyNote, rusqlite::Error> {
    Ok(StickyNote {
        task_id: row.get(0)?,
        title: row
            .get::<_, Option<String>>(1)?
            .unwrap_or_else(|| "便签".to_string()),
        note_type: "TASK".to_string(),
        content: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        pos_x: row
            .get::<_, Option<f64>>(3)?
            .unwrap_or(STICKY_NOTE_DEFAULT_POS_X),
        pos_y: row
            .get::<_, Option<f64>>(4)?
            .unwrap_or(STICKY_NOTE_DEFAULT_POS_Y),
        width: normalize_sticky_item_width(row.get(5)?),
        height: normalize_sticky_item_height(row.get(6)?),
        is_open: row.get::<_, i64>(7)? == 1,
        is_pinned: row.get::<_, Option<i64>>(8)?.unwrap_or(0) == 1,
        created_at: row.get(9)?,
        updated_at: row.get::<_, Option<String>>(10)?.unwrap_or_else(now_string),
        reminder_time: row.get(11)?,
    })
}

struct MigrationScript {
    version: String,
    description: String,
    sql: &'static str,
}

fn migration_scripts() -> Vec<MigrationScript> {
    vec![
        MigrationScript {
            version: "1.1.0".to_string(),
            description: "initial schema".to_string(),
            sql: include_str!("../migrations/V1.1.0__initial_schema.sql"),
        },
        MigrationScript {
            version: "1.2.0".to_string(),
            description: "add webdav settings".to_string(),
            sql: include_str!("../migrations/V1.2.0__add_webdav_settings.sql"),
        },
        MigrationScript {
            version: "1.3.0".to_string(),
            description: "add sync metadata".to_string(),
            sql: include_str!("../migrations/V1.3.0__add_sync_metadata.sql"),
        },
        MigrationScript {
            version: "1.4.0".to_string(),
            description: "add notification theme".to_string(),
            sql: include_str!("../migrations/V1.4.0__add_notification_theme.sql"),
        },
        MigrationScript {
            version: "1.4.1".to_string(),
            description: "add recurring modes".to_string(),
            sql: include_str!("../migrations/V1.4.1__add_recurring_modes.sql"),
        },
        MigrationScript {
            version: "1.4.2".to_string(),
            description: "add sticky note settings".to_string(),
            sql: include_str!("../migrations/V1.4.2__add_sticky_note_settings.sql"),
        },
        MigrationScript {
            version: "1.4.3".to_string(),
            description: "add sticky notes table".to_string(),
            sql: include_str!("../migrations/V1.4.3__add_sticky_notes_table.sql"),
        },
        MigrationScript {
            version: "1.4.4".to_string(),
            description: "add sticky note title and type".to_string(),
            sql: include_str!("../migrations/V1.4.4__add_sticky_note_meta.sql"),
        },
        MigrationScript {
            version: "1.4.5".to_string(),
            description: "merge sticky notes into tasks".to_string(),
            sql: include_str!("../migrations/V1.4.5__merge_sticky_notes_into_tasks.sql"),
        },
        MigrationScript {
            version: "1.4.6".to_string(),
            description: "add sticky note opacity".to_string(),
            sql: include_str!("../migrations/V1.4.6__add_sticky_note_opacity.sql"),
        },
        MigrationScript {
            version: "1.4.7".to_string(),
            description: "add sticky note item size".to_string(),
            sql: include_str!("../migrations/V1.4.7__add_sticky_note_item_size.sql"),
        },
        MigrationScript {
            version: "1.4.8".to_string(),
            description: "add window opacity".to_string(),
            sql: include_str!("../migrations/V1.4.8__add_window_opacity.sql"),
        },
        MigrationScript {
            version: "1.4.9".to_string(),
            description: "add sticky note pin state".to_string(),
            sql: include_str!("../migrations/V1.4.9__add_sticky_note_pin_state.sql"),
        },
        MigrationScript {
            version: "1.6.0".to_string(),
            description: "add weekday mask and quick add shortcut".to_string(),
            sql: include_str!("../migrations/V1.6.0__add_weekday_mask_and_quick_add.sql"),
        },
        MigrationScript {
            version: "2.0.0".to_string(),
            description: "add task tags and priority".to_string(),
            sql: include_str!("../migrations/V2.0.0__add_task_tags_and_priority.sql"),
        },
        MigrationScript {
            version: "2.0.1".to_string(),
            description: "add sync encryption".to_string(),
            sql: include_str!("../migrations/V2.0.1__add_sync_encryption.sql"),
        },
        MigrationScript {
            version: "2.0.2".to_string(),
            description: "add quiet hours, native notification and sticky toggle shortcut"
                .to_string(),
            sql: include_str!(
                "../migrations/V2.0.2__add_quiet_hours_notification_and_sticky_shortcut.sql"
            ),
        },
        MigrationScript {
            version: "2.0.3".to_string(),
            description: "add secret storage".to_string(),
            sql: include_str!("../migrations/V2.0.3__add_secret_storage.sql"),
        },
        MigrationScript {
            version: "2.0.4".to_string(),
            description: "add sticky snap".to_string(),
            sql: include_str!("../migrations/V2.0.4__add_sticky_snap.sql"),
        },
        MigrationScript {
            version: "2.0.5".to_string(),
            description: "add holiday auto update".to_string(),
            sql: include_str!("../migrations/V2.0.5__add_holiday_auto_update.sql"),
        },
    ]
}

fn compare_version(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<i32> {
        v.split('.')
            .map(|part| part.parse::<i32>().unwrap_or(0))
            .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    for i in 0..3 {
        let left = *av.get(i).unwrap_or(&0);
        let right = *bv.get(i).unwrap_or(&0);
        if left != right {
            return left.cmp(&right);
        }
    }
    std::cmp::Ordering::Equal
}

fn execute_sql_script(conn: &Connection, sql: &str) -> Result<(), AppError> {
    let mut cleaned = String::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }

    for statement in cleaned.split(';') {
        let trimmed = statement.trim();
        if trimmed.is_empty() {
            continue;
        }
        conn.execute_batch(trimmed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::RepeatMode;

    fn temp_db() -> (DbManager, PathBuf) {
        let dir = std::env::temp_dir().join(format!("taskreminder-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = DbManager::new(dir.join("test.db")).unwrap();
        (db, dir)
    }

    fn days_ago(days: i64) -> String {
        format_datetime(&(time::now() - chrono::Duration::days(days)))
    }

    fn task_row(db: &DbManager, id: &str) -> Option<(String, Option<String>)> {
        let conn = db.get_conn().unwrap();
        conn.query_row(
            "SELECT status, deleted_at FROM tasks WHERE id = ?",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .unwrap()
    }

    fn sample_recurring() -> RecurringTask {
        RecurringTask {
            id: String::new(),
            description: "weekly".to_string(),
            task_type: TaskType::Recurring,
            status: TaskStatus::Pending,
            created_at: String::new(),
            completed_at: None,
            reminder_time: None,
            updated_at: None,
            deleted_at: None,
            interval_minutes: 60,
            last_triggered: None,
            next_trigger: String::new(),
            is_paused: false,
            start_time: None,
            end_time: None,
            repeat_mode: RepeatMode::Daily,
            schedule_time: Some("09:00".to_string()),
            schedule_weekday: None,
            schedule_weekdays: None,
            schedule_day: None,
            cron_expression: None,
        }
    }

    #[test]
    fn cleanup_tombstones_old_completed_tasks_instead_of_deleting() {
        let (db, dir) = temp_db();
        let old = db.create_task("old", None).unwrap();
        let recent = db.create_task("recent", None).unwrap();
        {
            let conn = db.get_conn().unwrap();
            conn.execute(
                "UPDATE tasks SET status = 'COMPLETED', completed_at = ? WHERE id = ?",
                params![days_ago(40), old.id],
            )
            .unwrap();
            conn.execute(
                "UPDATE tasks SET status = 'COMPLETED', completed_at = ? WHERE id = ?",
                params![days_ago(1), recent.id],
            )
            .unwrap();
        }

        db.cleanup_data(TOMBSTONE_RETENTION_DAYS_LOCAL).unwrap();

        // 过期的已完成任务仍保留一行墓碑，云同步才能把删除传播到其他设备。
        let (_, deleted_at) = task_row(&db, &old.id).expect("old task row kept as tombstone");
        assert!(deleted_at.is_some());
        let (_, deleted_at) = task_row(&db, &recent.id).unwrap();
        assert!(deleted_at.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn purge_removes_only_expired_tombstones() {
        let (db, dir) = temp_db();
        let expired = db.create_task("expired", None).unwrap();
        let fresh = db.create_task("fresh", None).unwrap();
        let alive = db.create_task("alive", None).unwrap();
        {
            let conn = db.get_conn().unwrap();
            conn.execute(
                "UPDATE tasks SET deleted_at = ? WHERE id = ?",
                params![days_ago(10), expired.id],
            )
            .unwrap();
            conn.execute(
                "UPDATE tasks SET deleted_at = ? WHERE id = ?",
                params![days_ago(2), fresh.id],
            )
            .unwrap();
        }

        db.purge_expired_tombstones(7).unwrap();
        assert!(task_row(&db, &expired.id).is_none());
        assert!(task_row(&db, &fresh.id).is_some());
        assert!(task_row(&db, &alive.id).is_some());

        // 开启同步时保留期更长，10 天前的墓碑不应被删除。
        let kept = db.create_task("kept", None).unwrap();
        {
            let conn = db.get_conn().unwrap();
            conn.execute(
                "UPDATE tasks SET deleted_at = ? WHERE id = ?",
                params![days_ago(10), kept.id],
            )
            .unwrap();
        }
        db.purge_expired_tombstones(tombstone_retention_days(true))
            .unwrap();
        assert!(task_row(&db, &kept.id).is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn trash_lists_restores_and_expires_tombstones() {
        let (db, dir) = temp_db();
        let task = db.create_task("trash me", None).unwrap();
        let too_old = db.create_task("too old", None).unwrap();
        let alive = db.create_task("alive", None).unwrap();
        db.delete_task(&task.id).unwrap();
        {
            let conn = db.get_conn().unwrap();
            conn.execute(
                "UPDATE tasks SET deleted_at = ? WHERE id = ?",
                params![days_ago(10), too_old.id],
            )
            .unwrap();
        }

        // 只列出保留期内的墓碑，不含正常行。
        let ids: Vec<String> = db
            .list_deleted_tasks(7)
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids, vec![task.id.clone()]);
        assert!(!ids.contains(&alive.id));

        db.restore_task(&task.id).unwrap();
        let (_, deleted_at) = task_row(&db, &task.id).unwrap();
        assert!(deleted_at.is_none());
        assert!(db.list_deleted_tasks(7).unwrap().is_empty());

        // 永久删除：从回收站消失，行仍在（等待同步传播），清理后物理删除。
        db.delete_task(&task.id).unwrap();
        let before = db.get_task(&task.id).unwrap().unwrap().updated_at;
        db.expire_tombstones(TrashTable::Tasks, std::slice::from_ref(&task.id))
            .unwrap();
        assert!(db.list_deleted_tasks(7).unwrap().is_empty());
        let expired = db.get_task(&task.id).unwrap().unwrap();
        assert_eq!(expired.deleted_at.as_deref(), Some(EXPIRED_TOMBSTONE_TIME));
        assert!(expired.updated_at >= before);
        db.purge_expired_tombstones(60).unwrap();
        assert!(db.get_task(&task.id).unwrap().is_none());

        // 不能用“永久删除”删掉正常行。
        db.expire_tombstones(TrashTable::Tasks, std::slice::from_ref(&alive.id))
            .unwrap();
        assert!(db
            .get_task(&alive.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn trash_handles_recurring_tasks() {
        let (db, dir) = temp_db();
        let mut draft = sample_recurring();
        draft.next_trigger = "2026-01-01T09:00:00".to_string();
        let task = db.create_recurring_task(&draft).unwrap();
        db.delete_recurring_task(&task.id).unwrap();
        assert!(db.list_recurring_tasks().unwrap().is_empty());
        assert_eq!(db.list_deleted_recurring_tasks(7).unwrap().len(), 1);

        db.restore_recurring_task(&task.id).unwrap();
        assert_eq!(db.list_recurring_tasks().unwrap().len(), 1);
        assert!(db.list_deleted_recurring_tasks(7).unwrap().is_empty());

        db.delete_recurring_task(&task.id).unwrap();
        db.expire_tombstones(TrashTable::RecurringTasks, std::slice::from_ref(&task.id))
            .unwrap();
        assert!(db.list_deleted_recurring_tasks(60).unwrap().is_empty());
        db.purge_expired_tombstones(60).unwrap();
        assert!(db.get_recurring_task(&task.id).unwrap().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn purge_also_clears_deleted_reminder_records() {
        let (db, dir) = temp_db();
        let record = db
            .create_reminder_record("task-1", "desc", ReminderKind::Task)
            .unwrap();
        {
            let conn = db.get_conn().unwrap();
            conn.execute(
                "UPDATE reminder_records SET deleted_at = ? WHERE id = ?",
                params![days_ago(30), record.id],
            )
            .unwrap();
        }
        db.purge_expired_tombstones(7).unwrap();
        assert!(db.get_reminder_record(&record.id).unwrap().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn recurring_weekday_mask_roundtrip_and_quick_add_defaults() {
        let (db, dir) = temp_db();
        let mut draft = sample_recurring();
        draft.next_trigger = "2026-09-28T09:00:00".to_string();
        draft.repeat_mode = RepeatMode::Weekly;
        draft.schedule_weekday = Some(1);
        draft.schedule_weekdays = Some(0b10101);
        let created = db.create_recurring_task(&draft).unwrap();
        let loaded = db.get_recurring_task(&created.id).unwrap().unwrap();
        assert_eq!(loaded.schedule_weekdays, Some(0b10101));

        let mut updated = loaded.clone();
        updated.schedule_weekdays = Some(0b1100000);
        db.update_recurring_task(&updated).unwrap();
        let loaded = db.get_recurring_task(&created.id).unwrap().unwrap();
        assert_eq!(loaded.schedule_weekdays, Some(0b1100000));

        let mut settings = db.load_settings().unwrap();
        assert!(settings.quick_add_enabled);
        assert_eq!(settings.quick_add_shortcut, "CommandOrControl+Alt+N");
        settings.quick_add_enabled = false;
        settings.quick_add_shortcut = "Alt+Space".to_string();
        db.save_settings(&settings).unwrap();
        let settings = db.load_settings().unwrap();
        assert!(!settings.quick_add_enabled);
        assert_eq!(settings.quick_add_shortcut, "Alt+Space");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn task_tags_and_priority_roundtrip() {
        let (db, dir) = temp_db();
        let meta = TaskMeta {
            tags: vec![
                "#工作".to_string(),
                " 周报 ".to_string(),
                "工作".to_string(),
                "a,b".to_string(),
                String::new(),
            ],
            priority: 7,
        };
        let task = db.create_task_with_meta("写周报", None, &meta).unwrap();
        assert_eq!(task.tags, vec!["工作", "周报", "ab"]);
        assert_eq!(task.priority, 3);
        let loaded = db.get_task(&task.id).unwrap().unwrap();
        assert_eq!(loaded.tags, task.tags);
        assert_eq!(loaded.priority, 3);

        // 不传 meta 时保留原有标签与优先级。
        db.update_task(&task.id, "写周报 v2", None, None, None)
            .unwrap();
        let loaded = db.get_task(&task.id).unwrap().unwrap();
        assert_eq!(loaded.description, "写周报 v2");
        assert_eq!(loaded.tags, vec!["工作", "周报", "ab"]);
        assert_eq!(loaded.priority, 3);

        let cleared = TaskMeta {
            tags: Vec::new(),
            priority: -1,
        };
        db.update_task(&task.id, "写周报 v2", None, None, Some(&cleared))
            .unwrap();
        let loaded = db.get_task(&task.id).unwrap().unwrap();
        assert!(loaded.tags.is_empty());
        assert_eq!(loaded.priority, 0);

        // 旧版本创建的行没有标签：默认空。
        let plain = db.create_task("plain", None).unwrap();
        let loaded = db.get_task(&plain.id).unwrap().unwrap();
        assert!(loaded.tags.is_empty());
        assert_eq!(loaded.priority, 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn has_reminder_record_since_compares_trigger_time() {
        let (db, dir) = temp_db();
        let record = db
            .create_reminder_record("task-1", "desc", ReminderKind::Task)
            .unwrap();
        let trigger = parse_datetime_any(&record.trigger_time).unwrap();

        assert!(db
            .has_reminder_record_since("task-1", &(trigger - chrono::Duration::minutes(5)))
            .unwrap());
        assert!(db.has_reminder_record_since("task-1", &trigger).unwrap());
        assert!(!db
            .has_reminder_record_since("task-1", &(trigger + chrono::Duration::minutes(5)))
            .unwrap());
        assert!(!db
            .has_reminder_record_since("task-2", &(trigger - chrono::Duration::minutes(5)))
            .unwrap());

        // 已软删除的记录也算触发过，删除记录不应导致重复弹出。
        db.delete_reminder_record(&record.id).unwrap();
        assert!(db.has_reminder_record_since("task-1", &trigger).unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn sticky_row(db: &DbManager, id: &str) -> (f64, f64, f64, f64, String) {
        let conn = db.get_conn().unwrap();
        conn.query_row(
            "SELECT sticky_pos_x, sticky_pos_y, sticky_width, sticky_height, updated_at FROM tasks WHERE id = ?",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap()
    }

    fn backdate_task(db: &DbManager, id: &str) {
        let conn = db.get_conn().unwrap();
        conn.execute(
            "UPDATE tasks SET updated_at = '2020-01-01 00:00:00' WHERE id = ?",
            [id],
        )
        .unwrap();
    }

    #[test]
    fn sticky_move_keeps_negative_coordinates_and_skips_unchanged() {
        let (db, dir) = temp_db();
        let note = db
            .create_custom_sticky_note("note", None, Some(48.0), Some(76.0), None, None)
            .unwrap();
        let id = note.task_id.clone();

        // 主屏左侧副屏的负坐标原样保存。
        db.move_sticky_note(&id, -1500.0, -100.0).unwrap();
        let (x, y, _, _, _) = sticky_row(&db, &id);
        assert_eq!((x, y), (-1500.0, -100.0));

        // 位置不变（打开窗口时 set_position 触发的 Moved）不改 updated_at，避免产生同步改动。
        backdate_task(&db, &id);
        db.move_sticky_note(&id, -1500.2, -100.0).unwrap();
        let (_, _, _, _, updated_at) = sticky_row(&db, &id);
        assert_eq!(updated_at, "2020-01-01 00:00:00");

        db.move_sticky_note(&id, 200.0, -100.0).unwrap();
        let (x, _, _, _, updated_at) = sticky_row(&db, &id);
        assert_eq!(x, 200.0);
        assert_ne!(updated_at, "2020-01-01 00:00:00");

        // 尺寸同理。
        let (_, _, width, height, _) = sticky_row(&db, &id);
        backdate_task(&db, &id);
        db.resize_sticky_note(&id, width, height).unwrap();
        assert_eq!(sticky_row(&db, &id).4, "2020-01-01 00:00:00");
        db.resize_sticky_note(&id, width + 40.0, height).unwrap();
        assert_eq!(sticky_row(&db, &id).2, width + 40.0);
        assert_ne!(sticky_row(&db, &id).4, "2020-01-01 00:00:00");

        // 已关闭的便签被移动时重新记为打开。
        db.close_sticky_note(&id).unwrap();
        backdate_task(&db, &id);
        db.move_sticky_note(&id, 200.0, -100.0).unwrap();
        assert!(db.get_sticky_note(&id).unwrap().unwrap().is_open);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_repeat_mode_survives_read_and_write() {
        // 模拟 v2.1 新增的模式经同步写入本机数据库。
        let (db, dir) = temp_db();
        let created = db.create_recurring_task(&sample_recurring()).unwrap();
        {
            let conn = db.get_conn().unwrap();
            conn.execute(
                "UPDATE recurring_tasks SET repeat_mode = 'BIWEEKLY', schedule_time = '09:00', cron_expression = 'custom' WHERE id = ?",
                [&created.id],
            )
            .unwrap();
        }
        let mut task = db.get_recurring_task(&created.id).unwrap().unwrap();
        assert_eq!(
            task.repeat_mode,
            RepeatMode::Unknown("BIWEEKLY".to_string())
        );
        // 暂停等只改其他字段的写入不会改动模式与规则字段。
        task.is_paused = true;
        db.update_recurring_task(&task).unwrap();
        let conn = db.get_conn().unwrap();
        let (mode, time, cron): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT repeat_mode, schedule_time, cron_expression FROM recurring_tasks WHERE id = ?",
                [&created.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(mode, "BIWEEKLY");
        assert_eq!(time.as_deref(), Some("09:00"));
        assert_eq!(cron.as_deref(), Some("custom"));
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ipc_json_is_unchanged_by_typed_fields() {
        let (db, dir) = temp_db();
        let task = db.create_task("待办", None).unwrap();
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["type"], "ONE_TIME");
        assert_eq!(json["status"], "PENDING");
        db.complete_task(&task.id).unwrap();
        let done = serde_json::to_value(db.get_task(&task.id).unwrap().unwrap()).unwrap();
        assert_eq!(done["status"], "COMPLETED");

        let recurring = db.create_recurring_task(&sample_recurring()).unwrap();
        let json = serde_json::to_value(&recurring).unwrap();
        assert_eq!(json["type"], "RECURRING");
        assert_eq!(json["repeatMode"], recurring.repeat_mode.as_str());

        let record = db
            .create_reminder_record("task-1", "desc", ReminderKind::Task)
            .unwrap();
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["type"], "TASK");
        assert_eq!(json["action"], "PENDING");

        // 前端发来的字符串照常解析。
        let parsed: ReminderRecord = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.action, ReminderAction::Pending);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tombstone_cutoff_counts_back_from_now() {
        let _now = time::fix_now("2026-09-30T12:00:00");
        assert_eq!(tombstone_cutoff(7), "2026-09-23T12:00:00");
        assert_eq!(tombstone_cutoff(60), "2026-08-01T12:00:00");
        // 至少保留 1 天。
        assert_eq!(tombstone_cutoff(0), "2026-09-29T12:00:00");
    }

    #[test]
    fn deleting_or_completing_task_closes_its_sticky_note() {
        let (db, dir) = temp_db();
        let deleted = db
            .create_custom_sticky_note("删除", Some("内容"), None, None, None, None)
            .unwrap();
        let completed = db
            .create_custom_sticky_note("完成", Some("内容"), None, None, None, None)
            .unwrap();
        assert!(
            db.get_sticky_note(&deleted.task_id)
                .unwrap()
                .unwrap()
                .is_open
        );

        db.delete_task(&deleted.task_id).unwrap();
        assert!(
            !db.get_sticky_note(&deleted.task_id)
                .unwrap()
                .unwrap()
                .is_open
        );
        // 从回收站恢复时不会重新弹出便签。
        db.restore_task(&deleted.task_id).unwrap();
        assert!(
            !db.get_sticky_note(&deleted.task_id)
                .unwrap()
                .unwrap()
                .is_open
        );

        db.complete_task(&completed.task_id).unwrap();
        assert!(
            !db.get_sticky_note(&completed.task_id)
                .unwrap()
                .unwrap()
                .is_open
        );
        db.uncomplete_task(&completed.task_id).unwrap();
        assert!(
            !db.get_sticky_note(&completed.task_id)
                .unwrap()
                .unwrap()
                .is_open
        );
        // 便签内容保留，可以手动重新打开。
        assert_eq!(
            db.get_sticky_note(&completed.task_id)
                .unwrap()
                .unwrap()
                .content,
            "内容"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reopening_sticky_note_keeps_negative_coordinates() {
        let (db, dir) = temp_db();
        let note = db
            .create_custom_sticky_note("note", None, Some(48.0), Some(76.0), None, None)
            .unwrap();
        db.move_sticky_note(&note.task_id, -1500.0, -100.0).unwrap();
        db.close_sticky_note(&note.task_id).unwrap();
        let reopened = db
            .open_sticky_note(&note.task_id, None, Some(48.0), Some(76.0))
            .unwrap();
        assert_eq!((reopened.pos_x, reopened.pos_y), (-1500.0, -100.0));
        let stored = db.get_sticky_note(&note.task_id).unwrap().unwrap();
        assert_eq!((stored.pos_x, stored.pos_y), (-1500.0, -100.0));
        assert!(stored.is_open);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sticky_snap_setting_defaults_on_and_roundtrips() {
        let (db, dir) = temp_db();
        assert!(db.sticky_snap_enabled().unwrap());
        let mut settings = db.load_settings().unwrap();
        assert!(settings.sticky_snap_enabled);
        settings.sticky_snap_enabled = false;
        db.save_settings(&settings).unwrap();
        assert!(!db.load_settings().unwrap().sticky_snap_enabled);
        assert!(!db.sticky_snap_enabled().unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn holiday_auto_update_defaults_on_and_roundtrips() {
        let (db, dir) = temp_db();
        assert!(db.holiday_auto_update_enabled().unwrap());
        let mut settings = db.load_settings().unwrap();
        assert!(settings.holiday_auto_update);
        settings.holiday_auto_update = false;
        db.save_settings(&settings).unwrap();
        assert!(!db.load_settings().unwrap().holiday_auto_update);
        assert!(!db.holiday_auto_update_enabled().unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }
}
