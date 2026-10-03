//! 本机设置的读写（含凭据库中的密码）。

use super::*;

pub(super) fn normalize_sticky_note_opacity(opacity: Option<f64>) -> f64 {
    let value = opacity.filter(|v| v.is_finite()).unwrap_or(0.95);
    value.clamp(0.35, 1.0)
}

pub(super) fn normalize_window_opacity(opacity: Option<f64>) -> f64 {
    let value = opacity.filter(|v| v.is_finite()).unwrap_or(1.0);
    value.clamp(0.3, 1.0)
}

impl DbManager {
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

    /// 已完成待办的保留天数（0 为永久）。只读一列，定期清理时调用，避免 `load_settings` 访问凭据库。
    pub fn completed_retention_days(&self) -> Result<i64, AppError> {
        let conn = self.get_conn()?;
        let value: Option<i64> = conn
            .query_row(
                "SELECT completed_retention_days FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(normalize_completed_retention_days(
            value.unwrap_or(DEFAULT_COMPLETED_RETENTION_DAYS),
        ))
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
                   sticky_snap_enabled, holiday_auto_update, completed_retention_days
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
                completed_retention_days: normalize_completed_retention_days(
                    row.get::<_, Option<i64>>(35)?
                        .unwrap_or(DEFAULT_COMPLETED_RETENTION_DAYS),
                ),
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
                 sticky_snap_enabled = ?, holiday_auto_update = ?, completed_retention_days = ?
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
                normalize_completed_retention_days(settings.completed_retention_days),
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
}
