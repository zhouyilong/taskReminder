//! 导入（JSON 备份）的按行合并。

use super::*;

impl DbManager {
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
                "INSERT INTO recurring_tasks (
                    id, description, type, status, created_at, completed_at, interval_minutes,
                    last_triggered, next_trigger, is_paused, start_time, end_time,
                    repeat_mode, schedule_time, schedule_weekday, schedule_day, cron_expression,
                    updated_at, deleted_at, schedule_weekdays
                 )
                 VALUES (?, ?, 'RECURRING', 'PENDING', ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    description = excluded.description, type = excluded.type, status = excluded.status,
                    created_at = excluded.created_at, completed_at = excluded.completed_at,
                    interval_minutes = excluded.interval_minutes, last_triggered = excluded.last_triggered,
                    next_trigger = excluded.next_trigger, is_paused = excluded.is_paused,
                    start_time = excluded.start_time, end_time = excluded.end_time,
                    repeat_mode = excluded.repeat_mode, schedule_time = excluded.schedule_time,
                    schedule_weekday = excluded.schedule_weekday, schedule_day = excluded.schedule_day,
                    cron_expression = excluded.cron_expression, updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at, schedule_weekdays = excluded.schedule_weekdays",
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
                "INSERT INTO reminder_records (
                    id, reminder_id, description, type, trigger_time, close_time, action, updated_at, deleted_at
                 )
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    reminder_id = excluded.reminder_id, description = excluded.description,
                    type = excluded.type, trigger_time = excluded.trigger_time,
                    close_time = excluded.close_time, action = excluded.action,
                    updated_at = excluded.updated_at, deleted_at = excluded.deleted_at",
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ImportOutcome {
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
pub(super) fn import_outcome(
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
