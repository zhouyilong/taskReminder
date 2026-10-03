//! 一次性待办的读写。

use super::*;

/// 待办的组织信息：标签、优先级与项目。
#[derive(Clone, Debug, Default)]
pub struct TaskMeta {
    pub tags: Vec<String>,
    pub priority: i64,
    /// 项目（v2.2），空字符串为未分组。
    pub project: String,
}

/// 待办的批量操作（`batch_update_tasks`）。
#[derive(Debug, Clone, PartialEq)]
pub enum TaskBatchOp {
    Complete,
    Delete,
    /// 在原有标签后追加（规范化、去重，最多 10 个）。
    AddTags(Vec<String>),
    SetPriority(i64),
    /// 移到项目（空字符串为移出项目）。
    SetProject(String),
    /// `None` 表示清除提醒。
    SetReminder(Option<String>),
}

/// `task_from_row` 读取的列（顺序与下标一致）。
macro_rules! task_columns {
    () => {
        "id, description, sticky_content, type, status, created_at, completed_at, reminder_time,
         updated_at, deleted_at, tags, priority, due_at, sticky_color, sort_order, project"
    };
}

impl DbManager {
    pub fn list_active_tasks(&self) -> Result<Vec<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(concat!(
            "SELECT ",
            task_columns!(),
            "
             FROM tasks
             WHERE deleted_at IS NULL AND status != 'COMPLETED'
             ORDER BY created_at ASC"
        ))?;
        let rows = stmt.query_map([], task_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn list_completed_tasks(&self) -> Result<Vec<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(concat!(
            "SELECT ",
            task_columns!(),
            "
             FROM tasks
             WHERE deleted_at IS NULL AND status = 'COMPLETED'
             ORDER BY completed_at DESC"
        ))?;
        let rows = stmt.query_map([], task_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_task(&self, task_id: &str) -> Result<Option<Task>, AppError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(concat!(
            "SELECT ",
            task_columns!(),
            "
             FROM tasks WHERE id = ?"
        ))?;
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
        let project = normalize_project(&meta.project);
        conn.execute(
            "INSERT INTO tasks (id, description, type, status, created_at, completed_at, reminder_time, sticky_content, updated_at, deleted_at, tags, priority, project)
             VALUES (?, ?, 'ONE_TIME', 'PENDING', ?, NULL, NULL, ?, ?, NULL, ?, ?, ?)",
            params![id, description, now, note, now, tags, priority, project],
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
            due_at: None,
            sticky_color: String::new(),
            sort_order: None,
            project,
        })
    }

    /// 更新待办。`meta` 为 `None` 时保留原有标签与优先级（如稍后提醒只改时间）；
    /// 项目不经过这里，用 `set_task_project` 单独设置。
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
        let mut stmt = conn.prepare(concat!(
            "SELECT ",
            task_columns!(),
            "
             FROM tasks
             WHERE deleted_at IS NOT NULL AND deleted_at >= ?
             ORDER BY deleted_at DESC"
        ))?;
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

    /// 设置截止时间（`None` 清除）。时间不变时不写库。
    pub fn set_task_due(&self, task_id: &str, due_at: Option<&str>) -> Result<bool, AppError> {
        let conn = self.get_conn()?;
        let changed = conn.execute(
            "UPDATE tasks SET due_at = ?1, updated_at = ?2
             WHERE id = ?3 AND deleted_at IS NULL AND due_at IS NOT ?1",
            params![due_at, now_string(), task_id],
        )?;
        Ok(changed > 0)
    }

    /// 设置项目（空字符串为移出项目）。项目不变时不写库。
    pub fn set_task_project(&self, task_id: &str, project: &str) -> Result<bool, AppError> {
        let conn = self.get_conn()?;
        let changed = conn.execute(
            "UPDATE tasks SET project = ?1, updated_at = ?2
             WHERE id = ?3 AND deleted_at IS NULL AND project IS NOT ?1",
            params![normalize_project(project), now_string(), task_id],
        )?;
        Ok(changed > 0)
    }

    /// 写入手动排序位置（拖拽排序，可能同时给多条重新编号），只写确实变化的行。
    pub fn set_task_sort_orders(&self, orders: &[(String, f64)]) -> Result<usize, AppError> {
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        let now = now_string();
        let mut changed = 0;
        for (id, order) in orders {
            if !order.is_finite() {
                return Err(AppError::Invalid("排序位置无效".to_string()));
            }
            changed += tx.execute(
                "UPDATE tasks SET sort_order = ?1, updated_at = ?2
                 WHERE id = ?3 AND deleted_at IS NULL AND sort_order IS NOT ?1",
                params![order, now, id],
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    /// 在一个事务中对多条待办执行同一操作，返回实际改动的 id（已删除、不存在的跳过；
    /// 完成时跳过已完成的）。时间戳统一，只产生一次写入。
    pub fn batch_update_tasks(
        &self,
        ids: &[String],
        op: &TaskBatchOp,
    ) -> Result<Vec<String>, AppError> {
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        let now = now_string();
        let mut changed = Vec::new();
        for id in ids {
            let current: Option<(String, Option<String>)> = tx
                .query_row(
                    "SELECT status, tags FROM tasks WHERE id = ? AND deleted_at IS NULL",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((status, tags)) = current else {
                continue;
            };
            let rows = match op {
                TaskBatchOp::Complete if status == "COMPLETED" => 0,
                TaskBatchOp::Complete => tx.execute(
                    "UPDATE tasks SET status = 'COMPLETED', completed_at = ?1, sticky_is_open = 0, updated_at = ?1
                     WHERE id = ?2",
                    params![now, id],
                )?,
                TaskBatchOp::Delete => tx.execute(
                    "UPDATE tasks SET deleted_at = ?1, sticky_is_open = 0, updated_at = ?1 WHERE id = ?2",
                    params![now, id],
                )?,
                TaskBatchOp::AddTags(extra) => {
                    let mut merged = tags_from_db(tags);
                    let before = merged.clone();
                    merged.extend(extra.iter().cloned());
                    let merged = crate::models::normalize_tags(&merged);
                    if merged == before {
                        0
                    } else {
                        tx.execute(
                            "UPDATE tasks SET tags = ?1, updated_at = ?2 WHERE id = ?3",
                            params![merged.join(","), now, id],
                        )?
                    }
                }
                TaskBatchOp::SetPriority(priority) => tx.execute(
                    "UPDATE tasks SET priority = ?1, updated_at = ?2 WHERE id = ?3 AND priority != ?1",
                    params![normalize_priority(*priority), now, id],
                )?,
                TaskBatchOp::SetProject(project) => tx.execute(
                    "UPDATE tasks SET project = ?1, updated_at = ?2 WHERE id = ?3 AND project IS NOT ?1",
                    params![normalize_project(project), now, id],
                )?,
                TaskBatchOp::SetReminder(reminder) => tx.execute(
                    "UPDATE tasks SET reminder_time = ?1, updated_at = ?2
                     WHERE id = ?3 AND reminder_time IS NOT ?1",
                    params![reminder, now, id],
                )?,
            };
            if rows > 0 {
                changed.push(id.clone());
            }
        }
        tx.commit()?;
        Ok(changed)
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
        due_at: row.get(12)?,
        sticky_color: row.get::<_, Option<String>>(13)?.unwrap_or_default(),
        sort_order: row.get(14)?,
        project: normalize_project(&row.get::<_, Option<String>>(15)?.unwrap_or_default()),
    })
}
