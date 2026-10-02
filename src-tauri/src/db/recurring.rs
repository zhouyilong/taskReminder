//! 循环提醒的读写。

use super::*;

impl DbManager {
    pub fn list_recurring_tasks(&self) -> Result<Vec<RecurringTask>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, description, type, status, created_at, completed_at,
                    interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time,
                    repeat_mode, schedule_time, schedule_weekday, schedule_day, cron_expression,
                    updated_at, deleted_at, schedule_weekdays, tags
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
                    updated_at, deleted_at, schedule_weekdays, tags
             FROM recurring_tasks WHERE id = ?",
        )?;
        let task = stmt.query_row([task_id], recurring_from_row).optional()?;
        Ok(task)
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
                updated_at, deleted_at, schedule_weekdays, tags
            )
             VALUES (?, ?, 'RECURRING', 'PENDING', ?, NULL, ?, NULL, ?, 0, ?, ?,
                     ?, ?, ?, ?, ?, ?, NULL, ?, ?)",
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
                task.schedule_weekdays,
                tags_to_db(&task.tags)
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
            tags: crate::models::normalize_tags(&task.tags),
        })
    }

    pub fn update_recurring_task(&self, task: &RecurringTask) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = now_string();
        conn.execute(
            "UPDATE recurring_tasks
             SET description = ?, interval_minutes = ?, start_time = ?, end_time = ?,
                 repeat_mode = ?, schedule_time = ?, schedule_weekday = ?, schedule_weekdays = ?,
                 schedule_day = ?, cron_expression = ?, tags = ?,
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
                tags_to_db(&task.tags),
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
                    updated_at, deleted_at, schedule_weekdays, tags
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
}

pub(super) fn recurring_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<RecurringTask, rusqlite::Error> {
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
        tags: tags_from_db(row.get::<_, Option<String>>(20)?),
    })
}
