//! 一次性待办的读写。

use super::*;

/// 待办的组织信息：标签与优先级。
#[derive(Clone, Debug, Default)]
pub struct TaskMeta {
    pub tags: Vec<String>,
    pub priority: i64,
}

impl DbManager {
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
}

pub(super) fn task_from_row(row: &rusqlite::Row<'_>) -> Result<Task, rusqlite::Error> {
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
