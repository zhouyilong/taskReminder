//! 墓碑、回收站与定期清理。

use super::*;

impl DbManager {
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

    /// 定期清理：
    /// 1. 完成超过 `completed_retention_days` 天的任务转为墓碑（软删除），
    ///    而不是直接物理删除——否则云同步合并时远端仍有该行，会被重新插回本地。
    ///    默认的 30 天另外只保留最近 100 条；0 表示永久保留，不清理已完成任务；
    /// 2. 物理删除超过保留期的墓碑。
    pub fn cleanup_data(
        &self,
        tombstone_retention_days: i64,
        completed_retention_days: i64,
    ) -> Result<(), AppError> {
        let completed_retention_days = normalize_completed_retention_days(completed_retention_days);
        if completed_retention_days > 0 {
            self.tombstone_old_completed_tasks(completed_retention_days)?;
        }
        self.purge_expired_tombstones(tombstone_retention_days)?;
        Ok(())
    }

    fn tombstone_old_completed_tasks(&self, retention_days: i64) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        let now = time::now();
        let now_text = format_datetime(&now);
        let completed_cutoff = format_datetime(&(now - chrono::Duration::days(retention_days)));

        conn.execute(
            "UPDATE tasks SET deleted_at = ?1, updated_at = ?1
             WHERE status = 'COMPLETED' AND deleted_at IS NULL
               AND completed_at IS NOT NULL AND completed_at < ?2",
            params![now_text, completed_cutoff],
        )?;
        if retention_days != DEFAULT_COMPLETED_RETENTION_DAYS {
            return Ok(());
        }
        conn.execute(
            "UPDATE tasks SET deleted_at = ?1, updated_at = ?1 WHERE id IN (
                SELECT id FROM tasks
                WHERE status = 'COMPLETED' AND deleted_at IS NULL
                ORDER BY completed_at DESC
                LIMIT -1 OFFSET 100
            )",
            params![now_text],
        )?;
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

    pub fn optimize_database(&self) -> Result<(), AppError> {
        let conn = self.get_conn()?;
        conn.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);\n             ANALYZE;\n             PRAGMA optimize;",
        )?;
        Ok(())
    }
}
