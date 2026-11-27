//! 云端镜像库的同步状态（`cloud_sync_state`）、待推送队列（`pending_mutations`）、
//! 全量导出与远端增量 LWW 应用。

use super::*;
use task_reminder_core::lww::{effective_compare_time, is_incoming_newer};
use task_reminder_core::protocol::{
    SyncChangeSet, SyncRecurringTaskRow, SyncReminderRecordRow, SyncTaskRow,
};

#[derive(Clone, Debug, Default)]
pub struct CloudSyncStateRow {
    pub last_server_seq: i64,
    pub last_sync_time: Option<String>,
}

impl DbManager {
    pub fn get_cloud_sync_state(&self) -> Result<CloudSyncStateRow, AppError> {
        let conn = self.get_conn()?;
        let row = conn
            .query_row(
                "SELECT last_server_seq, last_sync_time
                 FROM cloud_sync_state WHERE id = 1",
                [],
                |row| {
                    Ok(CloudSyncStateRow {
                        last_server_seq: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                        last_sync_time: row.get(1)?,
                    })
                },
            )
            .optional()?;
        Ok(row.unwrap_or_default())
    }

    pub fn set_last_server_seq(&self, seq: i64) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE cloud_sync_state SET last_server_seq = MAX(last_server_seq, ?), last_sync_time = ? WHERE id = 1",
            params![seq, now_string()],
        )?;
        Ok(())
    }

    pub fn set_cloud_sync_status(&self, status: &str, error: Option<&str>) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE cloud_sync_state SET last_sync_status = ?, last_sync_error = ?, last_sync_time = ? WHERE id = 1",
            params![status, error, now_string()],
        )?;
        Ok(())
    }

    pub fn set_clock_offset_ms(&self, offset_ms: i64) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE cloud_sync_state SET clock_offset_ms = ? WHERE id = 1",
            [offset_ms],
        )?;
        Ok(())
    }

    pub fn pending_mutation_count(&self) -> Result<usize, AppError> {
        if !self.is_cloud_active() {
            return Ok(0);
        }
        let conn = self.get_conn()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM pending_mutations", [], |row| {
            row.get(0)
        })?;
        Ok(count.max(0) as usize)
    }

    pub fn list_pending_mutations(&self) -> Result<Vec<(String, String)>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn
            .prepare("SELECT table_name, row_id FROM pending_mutations ORDER BY updated_at ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn clear_pending_mutations(&self, keys: &[(String, String)]) -> Result<(), AppError> {
        if keys.is_empty() {
            return Ok(());
        }
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        for (table, id) in keys {
            tx.execute(
                "DELETE FROM pending_mutations WHERE table_name = ? AND row_id = ?",
                params![table, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// 按待推送键列表从当前活动库加载完整的同步行载荷。
    pub fn load_sync_rows_for_keys(
        &self,
        keys: &[(String, String)],
    ) -> Result<SyncChangeSet, AppError> {
        let conn = self.get_conn()?;
        let mut set = SyncChangeSet::default();
        for (table, id) in keys {
            match table.as_str() {
                "tasks" => {
                    if let Some(row) = load_sync_task_by_id(&conn, id)? {
                        set.tasks.push(row);
                    }
                }
                "recurring_tasks" => {
                    if let Some(row) = load_sync_recurring_by_id(&conn, id)? {
                        set.recurring_tasks.push(row);
                    }
                }
                "reminder_records" => {
                    if let Some(row) = load_sync_record_by_id(&conn, id)? {
                        set.reminder_records.push(row);
                    }
                }
                _ => {}
            }
        }
        Ok(set)
    }

    /// 从指定 SQLite 文件导出全部 3 张业务表（含墓碑），用于模式切换全量合并。
    pub fn export_all_sync_rows_from(db_path: &Path) -> Result<SyncChangeSet, AppError> {
        let conn = Connection::open(db_path)?;
        let mut set = SyncChangeSet::default();

        {
            let mut stmt = conn.prepare(SELECT_SYNC_TASK_SQL)?;
            let rows = stmt.query_map([], sync_task_from_row)?;
            for row in rows {
                set.tasks.push(row?);
            }
        }
        {
            let mut stmt = conn.prepare(SELECT_SYNC_RECURRING_SQL)?;
            let rows = stmt.query_map([], sync_recurring_from_row)?;
            for row in rows {
                set.recurring_tasks.push(row?);
            }
        }
        {
            let mut stmt = conn.prepare(SELECT_SYNC_RECORD_SQL)?;
            let rows = stmt.query_map([], sync_record_from_row)?;
            for row in rows {
                set.reminder_records.push(row?);
            }
        }
        Ok(set)
    }

    /// 统计指定 SQLite 文件中未删除的业务数据条数（用于模式切换确认弹窗）。
    pub fn count_business_data_in(db_path: &Path) -> Result<BusinessDataCounts, AppError> {
        let conn = Connection::open(db_path)?;
        let active_tasks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL AND status != 'COMPLETED'",
            [],
            |r| r.get(0),
        )?;
        let completed_tasks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL AND status = 'COMPLETED'",
            [],
            |r| r.get(0),
        )?;
        let recurring_tasks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM recurring_tasks WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        let reminder_records: i64 = conn.query_row(
            "SELECT COUNT(*) FROM reminder_records WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(BusinessDataCounts {
            active_tasks: active_tasks.max(0) as usize,
            completed_tasks: completed_tasks.max(0) as usize,
            recurring_tasks: recurring_tasks.max(0) as usize,
            reminder_records: reminder_records.max(0) as usize,
        })
    }

    /// 将服务端拉取/合并返回的远端行按 LWW 写入当前活动库（不写 `pending_mutations`，避免回声循环）。
    /// 返回实际写入（新增或覆盖）的行数。
    pub fn apply_remote_changes(&self, changes: &SyncChangeSet) -> Result<usize, AppError> {
        if changes.is_empty() {
            return Ok(0);
        }
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        let mut applied = 0usize;

        for row in &changes.tasks {
            if should_apply_remote(&tx, "tasks", &row.id, "created_at", row.compare_time())? {
                upsert_sync_task(&tx, row)?;
                applied += 1;
            }
        }
        for row in &changes.recurring_tasks {
            if should_apply_remote(
                &tx,
                "recurring_tasks",
                &row.id,
                "created_at",
                row.compare_time(),
            )? {
                upsert_sync_recurring(&tx, row)?;
                applied += 1;
            }
        }
        for row in &changes.reminder_records {
            if should_apply_remote(
                &tx,
                "reminder_records",
                &row.id,
                "trigger_time",
                row.compare_time(),
            )? {
                upsert_sync_record(&tx, row)?;
                applied += 1;
            }
        }

        tx.commit()?;
        Ok(applied)
    }
}

fn should_apply_remote(
    conn: &Connection,
    table: &str,
    id: &str,
    fallback_col: &str,
    incoming_time: Option<NaiveDateTime>,
) -> Result<bool, AppError> {
    let existing: Option<(Option<String>, Option<String>, Option<String>)> = conn
        .query_row(
            &format!(
                "SELECT updated_at, deleted_at, {} FROM {} WHERE id = ?",
                fallback_col, table
            ),
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((updated, deleted, fallback)) = existing else {
        return Ok(true);
    };
    let existing_time =
        effective_compare_time(updated.as_deref(), deleted.as_deref(), fallback.as_deref());
    // 远端较新或时间戳相等时采用远端权威版本。
    Ok(is_incoming_newer(existing_time, incoming_time) || existing_time == incoming_time)
}

const SELECT_SYNC_TASK_SQL: &str =
    "SELECT id, description, type, status, created_at, completed_at, reminder_time,
            updated_at, deleted_at, sticky_content, sticky_pos_x, sticky_pos_y, sticky_is_open,
            sticky_width, sticky_height, tags, priority, due_at, sticky_color, sort_order,
            project, urgent
     FROM tasks";

const SELECT_SYNC_RECURRING_SQL: &str =
    "SELECT id, description, type, status, created_at, completed_at, interval_minutes,
            last_triggered, next_trigger, is_paused, start_time, end_time,
            updated_at, deleted_at, repeat_mode, schedule_time, schedule_weekday, schedule_day,
            cron_expression, schedule_weekdays, tags, ends_on, remaining_count,
            temp_trigger, temp_replaces
     FROM recurring_tasks";

const SELECT_SYNC_RECORD_SQL: &str =
    "SELECT id, reminder_id, description, type, trigger_time, close_time, action,
            updated_at, deleted_at
     FROM reminder_records";

fn sync_task_from_row(row: &rusqlite::Row<'_>) -> Result<SyncTaskRow, rusqlite::Error> {
    Ok(SyncTaskRow {
        id: row.get(0)?,
        description: row.get(1)?,
        task_type: TaskType::parse(&row.get::<_, Option<String>>(2)?.unwrap_or_default()),
        status: TaskStatus::parse(&row.get::<_, Option<String>>(3)?.unwrap_or_default()),
        created_at: row.get(4)?,
        completed_at: row.get(5)?,
        reminder_time: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
        sticky_content: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
        sticky_pos_x: row.get::<_, Option<f64>>(10)?.unwrap_or(48.0),
        sticky_pos_y: row.get::<_, Option<f64>>(11)?.unwrap_or(76.0),
        sticky_is_open: row.get::<_, Option<i64>>(12)?.unwrap_or(0) != 0,
        sticky_width: row.get::<_, Option<f64>>(13)?.unwrap_or(420.0),
        sticky_height: row.get::<_, Option<f64>>(14)?.unwrap_or(300.0),
        tags: row.get::<_, Option<String>>(15)?.unwrap_or_default(),
        priority: row.get::<_, Option<i64>>(16)?.unwrap_or(0),
        due_at: row.get(17)?,
        sticky_color: row.get::<_, Option<String>>(18)?.unwrap_or_default(),
        sort_order: row.get(19)?,
        project: row.get::<_, Option<String>>(20)?.unwrap_or_default(),
        urgent: row.get::<_, Option<i64>>(21)?.unwrap_or(0) != 0,
    })
}

fn sync_recurring_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<SyncRecurringTaskRow, rusqlite::Error> {
    Ok(SyncRecurringTaskRow {
        id: row.get(0)?,
        description: row.get(1)?,
        task_type: TaskType::parse(&row.get::<_, Option<String>>(2)?.unwrap_or_default()),
        status: TaskStatus::parse(&row.get::<_, Option<String>>(3)?.unwrap_or_default()),
        created_at: row.get(4)?,
        completed_at: row.get(5)?,
        interval_minutes: row.get(6)?,
        last_triggered: row.get(7)?,
        next_trigger: row.get(8)?,
        is_paused: row.get::<_, Option<i64>>(9)?.unwrap_or(0) != 0,
        start_time: row.get(10)?,
        end_time: row.get(11)?,
        updated_at: row.get(12)?,
        deleted_at: row.get(13)?,
        repeat_mode: RepeatMode::parse(&row.get::<_, Option<String>>(14)?.unwrap_or_default()),
        schedule_time: row.get(15)?,
        schedule_weekday: row.get(16)?,
        schedule_day: row.get(17)?,
        cron_expression: row.get(18)?,
        schedule_weekdays: row.get(19)?,
        tags: row.get::<_, Option<String>>(20)?.unwrap_or_default(),
        ends_on: row.get(21)?,
        remaining_count: row.get(22)?,
        temp_trigger: row.get(23)?,
        temp_replaces: row.get(24)?,
    })
}

fn sync_record_from_row(row: &rusqlite::Row<'_>) -> Result<SyncReminderRecordRow, rusqlite::Error> {
    Ok(SyncReminderRecordRow {
        id: row.get(0)?,
        reminder_id: row.get(1)?,
        description: row.get(2)?,
        reminder_type: ReminderKind::parse(&row.get::<_, Option<String>>(3)?.unwrap_or_default()),
        trigger_time: row.get(4)?,
        close_time: row.get(5)?,
        action: ReminderAction::parse(&row.get::<_, Option<String>>(6)?.unwrap_or_default()),
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
    })
}

fn load_sync_task_by_id(conn: &Connection, id: &str) -> Result<Option<SyncTaskRow>, AppError> {
    let sql = format!("{} WHERE id = ?", SELECT_SYNC_TASK_SQL);
    Ok(conn.query_row(&sql, [id], sync_task_from_row).optional()?)
}

fn load_sync_recurring_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<SyncRecurringTaskRow>, AppError> {
    let sql = format!("{} WHERE id = ?", SELECT_SYNC_RECURRING_SQL);
    Ok(conn
        .query_row(&sql, [id], sync_recurring_from_row)
        .optional()?)
}

fn load_sync_record_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<SyncReminderRecordRow>, AppError> {
    let sql = format!("{} WHERE id = ?", SELECT_SYNC_RECORD_SQL);
    Ok(conn
        .query_row(&sql, [id], sync_record_from_row)
        .optional()?)
}

fn upsert_sync_task(conn: &Connection, row: &SyncTaskRow) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO tasks (
            id, description, type, status, created_at, completed_at, reminder_time,
            updated_at, deleted_at, sticky_content, sticky_pos_x, sticky_pos_y, sticky_is_open,
            sticky_width, sticky_height, tags, priority, due_at, sticky_color, sort_order,
            project, urgent
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
        ON CONFLICT(id) DO UPDATE SET
            description = excluded.description,
            type = excluded.type,
            status = excluded.status,
            created_at = excluded.created_at,
            completed_at = excluded.completed_at,
            reminder_time = excluded.reminder_time,
            updated_at = excluded.updated_at,
            deleted_at = excluded.deleted_at,
            sticky_content = excluded.sticky_content,
            sticky_pos_x = excluded.sticky_pos_x,
            sticky_pos_y = excluded.sticky_pos_y,
            sticky_is_open = excluded.sticky_is_open,
            sticky_width = excluded.sticky_width,
            sticky_height = excluded.sticky_height,
            tags = excluded.tags,
            priority = excluded.priority,
            due_at = excluded.due_at,
            sticky_color = excluded.sticky_color,
            sort_order = excluded.sort_order,
            project = excluded.project,
            urgent = excluded.urgent",
        params![
            row.id,
            row.description,
            row.task_type.as_str(),
            row.status.as_str(),
            row.created_at,
            row.completed_at,
            row.reminder_time,
            row.updated_at,
            row.deleted_at,
            row.sticky_content,
            row.sticky_pos_x,
            row.sticky_pos_y,
            i64::from(row.sticky_is_open),
            row.sticky_width,
            row.sticky_height,
            row.tags,
            row.priority,
            row.due_at,
            row.sticky_color,
            row.sort_order,
            row.project,
            i64::from(row.urgent),
        ],
    )?;
    Ok(())
}

fn upsert_sync_recurring(conn: &Connection, row: &SyncRecurringTaskRow) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO recurring_tasks (
            id, description, type, status, created_at, completed_at, interval_minutes,
            last_triggered, next_trigger, is_paused, start_time, end_time,
            updated_at, deleted_at, repeat_mode, schedule_time, schedule_weekday, schedule_day,
            cron_expression, schedule_weekdays, tags, ends_on, remaining_count,
            temp_trigger, temp_replaces
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
        ON CONFLICT(id) DO UPDATE SET
            description = excluded.description,
            type = excluded.type,
            status = excluded.status,
            created_at = excluded.created_at,
            completed_at = excluded.completed_at,
            interval_minutes = excluded.interval_minutes,
            last_triggered = excluded.last_triggered,
            next_trigger = excluded.next_trigger,
            is_paused = excluded.is_paused,
            start_time = excluded.start_time,
            end_time = excluded.end_time,
            updated_at = excluded.updated_at,
            deleted_at = excluded.deleted_at,
            repeat_mode = excluded.repeat_mode,
            schedule_time = excluded.schedule_time,
            schedule_weekday = excluded.schedule_weekday,
            schedule_day = excluded.schedule_day,
            cron_expression = excluded.cron_expression,
            schedule_weekdays = excluded.schedule_weekdays,
            tags = excluded.tags,
            ends_on = excluded.ends_on,
            remaining_count = excluded.remaining_count,
            temp_trigger = excluded.temp_trigger,
            temp_replaces = excluded.temp_replaces",
        params![
            row.id,
            row.description,
            row.task_type.as_str(),
            row.status.as_str(),
            row.created_at,
            row.completed_at,
            row.interval_minutes,
            row.last_triggered,
            row.next_trigger,
            i64::from(row.is_paused),
            row.start_time,
            row.end_time,
            row.updated_at,
            row.deleted_at,
            row.repeat_mode.as_str(),
            row.schedule_time,
            row.schedule_weekday,
            row.schedule_day,
            row.cron_expression,
            row.schedule_weekdays,
            row.tags,
            row.ends_on,
            row.remaining_count,
            row.temp_trigger,
            row.temp_replaces,
        ],
    )?;
    Ok(())
}

fn upsert_sync_record(conn: &Connection, row: &SyncReminderRecordRow) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO reminder_records (
            id, reminder_id, description, type, trigger_time, close_time, action,
            updated_at, deleted_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(id) DO UPDATE SET
            reminder_id = excluded.reminder_id,
            description = excluded.description,
            type = excluded.type,
            trigger_time = excluded.trigger_time,
            close_time = excluded.close_time,
            action = excluded.action,
            updated_at = excluded.updated_at,
            deleted_at = excluded.deleted_at",
        params![
            row.id,
            row.reminder_id,
            row.description,
            row.reminder_type.as_str(),
            row.trigger_time,
            row.close_time,
            row.action.as_str(),
            row.updated_at,
            row.deleted_at,
        ],
    )?;
    Ok(())
}
