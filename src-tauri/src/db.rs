use std::path::PathBuf;

use chrono::{Local, NaiveDateTime};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{AppSettings, RecurringTask, ReminderRecord, Task};

#[derive(Clone)]
pub struct DbManager {
    pool: Pool<SqliteConnectionManager>,
    db_path: PathBuf,
}

impl DbManager {
    pub fn new(db_path: PathBuf) -> Result<Self, AppError> {
        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::new(manager).map_err(|e| AppError::Database(e.to_string()))?;
        let db = DbManager { pool, db_path };
        db.init()?;
        Ok(db)
    }

    pub fn db_path(&self) -> PathBuf {
        self.db_path.clone()
    }

    fn get_conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, AppError> {
        self.pool.get().map_err(|e| AppError::Database(e.to_string()))
    }

    fn init(&self) -> Result<(), AppError> {
        let mut conn = self.get_conn()?;
        self.apply_pragmas(&conn)?;
        self.ensure_version_table(&conn)?;
        self.apply_migrations(&mut conn)?;
        self.ensure_settings_row(&conn)?;
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
            .query_row("SELECT version FROM schema_version WHERE id = 1", [], |row| row.get(0))
            .optional()?;
        Ok(version.unwrap_or_else(|| "0.0.0".to_string()))
    }

    fn ensure_settings_row(&self, conn: &Connection) -> Result<(), AppError> {
        conn.execute(
            "INSERT OR IGNORE INTO settings (id, auto_start_enabled, sound_enabled, snooze_minutes, webdav_enabled, webdav_url, webdav_username, webdav_password, webdav_root_path, webdav_sync_interval_minutes, webdav_last_sync_time, webdav_last_local_change_time, webdav_last_sync_status, webdav_last_sync_error, webdav_device_id, notification_theme)
             VALUES (1, 0, 1, 5, 0, '', '', '', '', 60, NULL, NULL, NULL, NULL, ?, 'app')",
            [Uuid::new_v4().to_string()],
        )?;
        Ok(())
    }

    pub fn list_active_tasks(&self) -> Result<Vec<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at
             FROM tasks
             WHERE deleted_at IS NULL AND status != 'COMPLETED'
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| task_from_row(row))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn list_completed_tasks(&self) -> Result<Vec<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at
             FROM tasks
             WHERE deleted_at IS NULL AND status = 'COMPLETED'
             ORDER BY completed_at DESC",
        )?;
        let rows = stmt.query_map([], |row| task_from_row(row))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_task(&self, task_id: &str) -> Result<Option<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at
             FROM tasks WHERE id = ?",
        )?;
        let task = stmt
            .query_row([task_id], |row| task_from_row(row))
            .optional()?;
        Ok(task)
    }

    pub fn list_recurring_tasks(&self) -> Result<Vec<RecurringTask>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at,
                    interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time,
                    updated_at, deleted_at
             FROM recurring_tasks
             WHERE deleted_at IS NULL
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| recurring_from_row(row))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_recurring_task(&self, task_id: &str) -> Result<Option<RecurringTask>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at,
                    interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time,
                    updated_at, deleted_at
             FROM recurring_tasks WHERE id = ?",
        )?;
        let task = stmt
            .query_row([task_id], |row| recurring_from_row(row))
            .optional()?;
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
        let rows = stmt.query_map([], |row| record_from_row(row))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_reminder_record(&self, record_id: &str) -> Result<Option<ReminderRecord>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, reminder_id, description, type, trigger_time, close_time, action, updated_at, deleted_at
             FROM reminder_records WHERE id = ?",
        )?;
        let record = stmt
            .query_row([record_id], |row| record_from_row(row))
            .optional()?;
        Ok(record)
    }

    pub fn create_task(&self, description: &str) -> Result<Task, AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tasks (id, description, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at)
             VALUES (?, ?, 'ONE_TIME', 'PENDING', ?, NULL, NULL, ?, NULL)",
            params![id, description, now, now],
        )?;
        Ok(Task {
            id,
            description: description.to_string(),
            task_type: "ONE_TIME".to_string(),
            status: "PENDING".to_string(),
            created_at: now.clone(),
            completed_at: None,
            reminder_time: None,
            updated_at: Some(now),
            deleted_at: None,
        })
    }

    pub fn update_task(&self, task_id: &str, description: &str, reminder_time: Option<String>) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks SET description = ?, reminder_time = ?, updated_at = ? WHERE id = ?",
            params![description, reminder_time, now, task_id],
        )?;
        Ok(())
    }

    pub fn complete_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE tasks SET status = 'COMPLETED', completed_at = ?, updated_at = ? WHERE id = ?",
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
            "UPDATE tasks SET deleted_at = ?, updated_at = ? WHERE id = ?",
            params![now, now, task_id],
        )?;
        Ok(())
    }

    pub fn create_recurring_task(
        &self,
        description: &str,
        interval_minutes: i64,
        start_time: Option<String>,
        end_time: Option<String>,
    ) -> Result<RecurringTask, AppError> {
        let conn = self.get_conn()?;
        let id = Uuid::new_v4().to_string();
        let now = now_string();
        let next_trigger = add_minutes(&now, interval_minutes)?;
        conn.execute(
            "INSERT INTO recurring_tasks (id, description, type, status, created_at, completed_at, interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time, updated_at, deleted_at)
             VALUES (?, ?, 'RECURRING', 'PENDING', ?, NULL, ?, NULL, ?, 0, ?, ?, ?, NULL)",
            params![id, description, now, interval_minutes, next_trigger, start_time, end_time, now],
        )?;
        Ok(RecurringTask {
            id,
            description: description.to_string(),
            task_type: "RECURRING".to_string(),
            status: "PENDING".to_string(),
            created_at: now.clone(),
            completed_at: None,
            reminder_time: None,
            updated_at: Some(now),
            deleted_at: None,
            interval_minutes,
            last_triggered: None,
            next_trigger,
            is_paused: false,
            start_time,
            end_time,
        })
    }

    pub fn update_recurring_task(&self, task: &RecurringTask) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE recurring_tasks
             SET description = ?, interval_minutes = ?, start_time = ?, end_time = ?, is_paused = ?, next_trigger = ?, last_triggered = ?, updated_at = ?
             WHERE id = ?",
            params![
                task.description,
                task.interval_minutes,
                task.start_time,
                task.end_time,
                if task.is_paused { 1 } else { 0 },
                task.next_trigger,
                task.last_triggered,
                now,
                task.id
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

    pub fn resume_recurring_task(&self, task_id: &str) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE recurring_tasks SET is_paused = 0, updated_at = ? WHERE id = ?",
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

    pub fn create_reminder_record(
        &self,
        reminder_id: &str,
        description: &str,
        reminder_type: &str,
    ) -> Result<ReminderRecord, AppError> {
        let conn = self.get_conn()?;
        let id = Uuid::new_v4().to_string();
        let now = now_string();
        conn.execute(
            "INSERT INTO reminder_records (id, reminder_id, description, type, trigger_time, close_time, action, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, NULL, 'PENDING', ?, NULL)",
            params![id, reminder_id, description, reminder_type, now, now],
        )?;
        Ok(ReminderRecord {
            id,
            reminder_id: reminder_id.to_string(),
            description: description.to_string(),
            reminder_type: reminder_type.to_string(),
            trigger_time: now.clone(),
            close_time: None,
            action: "PENDING".to_string(),
            updated_at: Some(now),
            deleted_at: None,
        })
    }

    pub fn update_reminder_record_action(
        &self,
        record_id: &str,
        action: &str,
    ) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE reminder_records SET action = ?, close_time = ?, updated_at = ? WHERE id = ?",
            params![action, now, now, record_id],
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

    pub fn load_settings(&self) -> Result<AppSettings, AppError> {
        let conn = self.get_conn()?;
        let sql = "SELECT auto_start_enabled, sound_enabled, snooze_minutes,
                   webdav_enabled, webdav_url, webdav_username, webdav_password,
                   webdav_root_path, webdav_sync_interval_minutes, webdav_last_sync_time,
                   webdav_last_local_change_time, webdav_last_sync_status, webdav_last_sync_error,
                   webdav_device_id, notification_theme
                   FROM settings WHERE id = 1";
        let mut stmt = conn.prepare(sql)?;
        let row = stmt.query_row([], |row| {
            let webdav_url: String = row.get::<_, Option<String>>(4)?.unwrap_or_default();
            let webdav_username: String = row.get::<_, Option<String>>(5)?.unwrap_or_default();
            let webdav_password: String = row.get::<_, Option<String>>(6)?.unwrap_or_default();
            let webdav_root_path: String = row.get::<_, Option<String>>(7)?.unwrap_or_default();
            let webdav_device_id: String = row
                .get::<_, Option<String>>(13)?
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let notification_theme: String = row
                .get::<_, Option<String>>(14)?
                .unwrap_or_else(|| "app".to_string());
            Ok(AppSettings {
                auto_start_enabled: row.get::<_, i64>(0)? == 1,
                sound_enabled: row.get::<_, i64>(1)? == 1,
                snooze_minutes: row.get(2)?,
                webdav_enabled: row.get::<_, i64>(3)? == 1,
                webdav_url,
                webdav_username,
                webdav_password,
                webdav_root_path,
                webdav_sync_interval_minutes: row.get(8)?,
                webdav_last_sync_time: row.get(9)?,
                webdav_last_local_change_time: row.get(10)?,
                webdav_last_sync_status: row.get(11)?,
                webdav_last_sync_error: row.get(12)?,
                webdav_device_id,
                notification_theme,
            })
        })?;
        Ok(row)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE settings
             SET auto_start_enabled = ?, sound_enabled = ?, snooze_minutes = ?,
                 webdav_enabled = ?, webdav_url = ?, webdav_username = ?, webdav_password = ?,
                 webdav_root_path = ?, webdav_sync_interval_minutes = ?, webdav_last_sync_time = ?,
                 webdav_last_local_change_time = ?, webdav_last_sync_status = ?, webdav_last_sync_error = ?,
                 webdav_device_id = ?, notification_theme = ?
             WHERE id = 1",
            params![
                if settings.auto_start_enabled { 1 } else { 0 },
                if settings.sound_enabled { 1 } else { 0 },
                settings.snooze_minutes,
                if settings.webdav_enabled { 1 } else { 0 },
                settings.webdav_url,
                settings.webdav_username,
                settings.webdav_password,
                settings.webdav_root_path,
                settings.webdav_sync_interval_minutes,
                settings.webdav_last_sync_time,
                settings.webdav_last_local_change_time,
                settings.webdav_last_sync_status,
                settings.webdav_last_sync_error,
                settings.webdav_device_id,
                settings.notification_theme,
            ],
        )?;
        Ok(())
    }

    pub fn update_sync_status(&self, status: &str, error: Option<String>) -> Result<AppSettings, AppError> {
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

    pub fn cleanup_data(&self) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = Local::now().naive_local();
        let completed_cutoff = (now - chrono::Duration::days(30)).format("%Y-%m-%dT%H:%M:%S").to_string();
        let deleted_cutoff = (now - chrono::Duration::days(7)).format("%Y-%m-%dT%H:%M:%S").to_string();

        conn.execute(
            "DELETE FROM tasks WHERE deleted_at IS NOT NULL AND deleted_at < ?",
            [deleted_cutoff.clone()],
        )?;
        conn.execute(
            "DELETE FROM tasks WHERE status = 'COMPLETED' AND deleted_at IS NULL AND completed_at IS NOT NULL AND completed_at < ?",
            [completed_cutoff],
        )?;
        conn.execute(
            "DELETE FROM tasks WHERE id IN (\n                SELECT id FROM tasks\n                WHERE status = 'COMPLETED' AND deleted_at IS NULL\n                ORDER BY completed_at DESC\n                LIMIT -1 OFFSET 100\n            )",
            [],
        )?;
        conn.execute(
            "DELETE FROM recurring_tasks WHERE deleted_at IS NOT NULL AND deleted_at < ?",
            [deleted_cutoff],
        )?;
        Ok(())
    }

    pub fn optimize_database(&self) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        conn.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);\n             ANALYZE;\n             PRAGMA optimize;",
        )?;
        Ok(())
    }
}

fn task_from_row(row: &rusqlite::Row<'_>) -> Result<Task, rusqlite::Error> {
    Ok(Task {
        id: row.get(0)?,
        description: row.get(1)?,
        task_type: row.get(2)?,
        status: row.get(3)?,
        created_at: row.get(4)?,
        completed_at: row.get(5)?,
        reminder_time: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
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
        updated_at: row.get(12)?,
        deleted_at: row.get(13)?,
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

fn now_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn parse_datetime(value: &str) -> Result<NaiveDateTime, AppError> {
    parse_datetime_any(value).ok_or_else(|| AppError::Invalid(format!("无法解析时间: {}", value)))
}

fn add_minutes(base: &str, minutes: i64) -> Result<String, AppError> {
    let dt = parse_datetime(base)? + chrono::Duration::minutes(minutes);
    Ok(dt.format("%Y-%m-%dT%H:%M:%S").to_string())
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
