//! 提醒记录的读写。

use super::*;

impl DbManager {
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
}

pub(super) fn record_from_row(row: &rusqlite::Row<'_>) -> Result<ReminderRecord, rusqlite::Error> {
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
