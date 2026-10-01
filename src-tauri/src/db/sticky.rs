//! 便签（待办的便签内容、窗口位置与尺寸）的读写。

use super::*;

pub(super) const STICKY_NOTE_DEFAULT_POS_X: f64 = 48.0;

pub(super) const STICKY_NOTE_DEFAULT_POS_Y: f64 = 76.0;

pub(super) const STICKY_NOTE_ITEM_DEFAULT_WIDTH: f64 = 284.0;

pub(super) const STICKY_NOTE_ITEM_DEFAULT_HEIGHT: f64 = 280.0;

pub(super) const STICKY_NOTE_ITEM_MIN_WIDTH: f64 = 220.0;

pub(super) const STICKY_NOTE_ITEM_MIN_HEIGHT: f64 = 180.0;

pub(super) fn normalize_sticky_item_width(width: Option<f64>) -> f64 {
    width
        .filter(|value| value.is_finite())
        .unwrap_or(STICKY_NOTE_ITEM_DEFAULT_WIDTH)
        .max(STICKY_NOTE_ITEM_MIN_WIDTH)
}

pub(super) fn normalize_sticky_item_height(height: Option<f64>) -> f64 {
    height
        .filter(|value| value.is_finite())
        .unwrap_or(STICKY_NOTE_ITEM_DEFAULT_HEIGHT)
        .max(STICKY_NOTE_ITEM_MIN_HEIGHT)
}

impl DbManager {
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
}

pub(super) fn sticky_note_from_task_row(
    row: &rusqlite::Row<'_>,
) -> Result<StickyNote, rusqlite::Error> {
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
