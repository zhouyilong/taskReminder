//! 本机设备级设置的读写（始终存于主库 `taskreminder.db`，不受云端账号业务库切换影响）。

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
    /// 待办清单便签的窗口状态（本机设置）。
    pub fn todo_sticky_state(&self) -> Result<TodoStickyState, AppError> {
        let conn = self.get_settings_conn()?;
        let value: Option<String> = conn
            .query_row(
                "SELECT todo_sticky_state FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(TodoStickyState::parse(value.as_deref().unwrap_or("")))
    }

    /// 读取、修改并保存待办清单便签的窗口状态，返回修改后的状态。
    pub fn update_todo_sticky_state(
        &self,
        change: impl FnOnce(&mut TodoStickyState),
    ) -> Result<TodoStickyState, AppError> {
        let mut state = self.todo_sticky_state()?;
        change(&mut state);
        let json = serde_json::to_string(&state)
            .map_err(|e| AppError::System(format!("保存待办清单便签状态失败: {}", e)))?;
        let conn = self.get_settings_conn()?;
        conn.execute(
            "UPDATE settings SET todo_sticky_state = ? WHERE id = 1",
            [json],
        )?;
        Ok(state)
    }

    /// 是否开启便签贴边吸附。
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn sticky_snap_enabled(&self) -> Result<bool, AppError> {
        let conn = self.get_settings_conn()?;
        let value: Option<i64> = conn
            .query_row(
                "SELECT sticky_snap_enabled FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.unwrap_or(1) == 1)
    }

    /// 是否开启节假日数据在线更新。
    pub fn holiday_auto_update_enabled(&self) -> Result<bool, AppError> {
        let conn = self.get_settings_conn()?;
        let value: Option<i64> = conn
            .query_row(
                "SELECT holiday_auto_update FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.unwrap_or(1) == 1)
    }

    /// 已完成待办的保留天数（0 为永久）。
    pub fn completed_retention_days(&self) -> Result<i64, AppError> {
        let conn = self.get_settings_conn()?;
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

    /// 每日自动备份保留的份数。
    pub fn auto_backup_keep(&self) -> Result<i64, AppError> {
        let conn = self.get_settings_conn()?;
        let value: Option<i64> = conn
            .query_row(
                "SELECT auto_backup_keep FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(normalize_auto_backup_keep(
            value.unwrap_or(DEFAULT_AUTO_BACKUP_KEEP),
        ))
    }

    /// 提醒弹出与再次提醒用到的设置。
    pub fn notification_prefs(&self) -> Result<NotificationPrefs, AppError> {
        let conn = self.get_settings_conn()?;
        let prefs = conn
            .query_row(
                "SELECT sound_enabled, quiet_hours_enabled, quiet_hours_start, quiet_hours_end,
                        notification_theme, attention_effect, attention_until_handled,
                        attention_repeat_minutes, notification_screen, attention_intensity,
                        attention_color
                 FROM settings WHERE id = 1",
                [],
                |row| {
                    Ok(NotificationPrefs {
                        sound_enabled: row.get::<_, Option<i64>>(0)?.unwrap_or(1) == 1,
                        quiet_hours_enabled: row.get::<_, Option<i64>>(1)?.unwrap_or(0) == 1,
                        quiet_hours_start: quiet_hours::normalize_clock(
                            &row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                            quiet_hours::DEFAULT_START,
                        ),
                        quiet_hours_end: quiet_hours::normalize_clock(
                            &row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                            quiet_hours::DEFAULT_END,
                        ),
                        notification_theme: row
                            .get::<_, Option<String>>(4)?
                            .unwrap_or_else(|| "app".to_string()),
                        attention_effect: attention::normalize_effect(
                            &row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                        ),
                        attention_until_handled: row.get::<_, Option<i64>>(6)?.unwrap_or(0) == 1,
                        attention_repeat_minutes: normalize_attention_repeat_minutes(
                            row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                        ),
                        notification_screen: attention::normalize_screen(
                            &row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                        ),
                        attention_intensity: attention::normalize_intensity(
                            row.get::<_, Option<i64>>(9)?
                                .unwrap_or(attention::INTENSITY_DEFAULT),
                        ),
                        attention_color: attention::normalize_color(
                            &row.get::<_, Option<String>>(10)?.unwrap_or_default(),
                        ),
                    })
                },
            )
            .optional()?;
        Ok(prefs.unwrap_or_default())
    }

    pub fn load_settings(&self) -> Result<AppSettings, AppError> {
        let conn = self.get_settings_conn()?;
        let sql = "SELECT auto_start_enabled, sound_enabled, snooze_minutes,
                   sticky_note_enabled, sticky_note_content, sticky_note_width, sticky_note_height,
                   sticky_note_x, sticky_note_y, sticky_note_opacity, window_opacity,
                   webdav_device_id, notification_theme, quick_add_enabled, quick_add_shortcut,
                   quiet_hours_enabled, quiet_hours_start, quiet_hours_end,
                   native_notification_enabled, sticky_toggle_shortcut, secret_storage,
                   sticky_snap_enabled, holiday_auto_update, completed_retention_days,
                   attention_effect, attention_until_handled, attention_repeat_minutes,
                   notification_screen, attention_intensity, attention_color,
                   auto_backup_keep, app_mode, cloud_server_url, cloud_user_id,
                   cloud_username, cloud_nickname, cloud_user_role, cloud_must_change_password,
                   last_merged_account_id, last_merged_username
                   FROM settings WHERE id = 1";
        let mut stmt = conn.prepare(sql)?;
        let settings = stmt.query_row([], |row| {
            let sticky_note_width = row.get::<_, Option<i64>>(5)?.unwrap_or(360).max(260);
            let sticky_note_height = row.get::<_, Option<i64>>(6)?.unwrap_or(520).max(320);
            let sticky_note_opacity = normalize_sticky_note_opacity(row.get(9)?);
            let window_opacity = normalize_window_opacity(row.get(10)?);
            let device_id: String = row
                .get::<_, Option<String>>(11)?
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let notification_theme: String = row
                .get::<_, Option<String>>(12)?
                .unwrap_or_else(|| "app".to_string());
            let quick_add_shortcut: String = row
                .get::<_, Option<String>>(14)?
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(default_quick_add_shortcut);
            let app_mode = AppMode::parse(&row.get::<_, Option<String>>(31)?.unwrap_or_default());
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
                device_id,
                notification_theme,
                quick_add_enabled: row.get::<_, Option<i64>>(13)?.unwrap_or(1) == 1,
                quick_add_shortcut,
                quiet_hours_enabled: row.get::<_, Option<i64>>(15)?.unwrap_or(0) == 1,
                quiet_hours_start: quiet_hours::normalize_clock(
                    &row.get::<_, Option<String>>(16)?.unwrap_or_default(),
                    quiet_hours::DEFAULT_START,
                ),
                quiet_hours_end: quiet_hours::normalize_clock(
                    &row.get::<_, Option<String>>(17)?.unwrap_or_default(),
                    quiet_hours::DEFAULT_END,
                ),
                native_notification_enabled: row.get::<_, Option<i64>>(18)?.unwrap_or(0) == 1,
                sticky_toggle_shortcut: row
                    .get::<_, Option<String>>(19)?
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                secret_storage: row
                    .get::<_, Option<String>>(20)?
                    .unwrap_or_else(|| secrets::STORAGE_DB.to_string()),
                sticky_snap_enabled: row.get::<_, Option<i64>>(21)?.unwrap_or(1) == 1,
                holiday_auto_update: row.get::<_, Option<i64>>(22)?.unwrap_or(1) == 1,
                completed_retention_days: normalize_completed_retention_days(
                    row.get::<_, Option<i64>>(23)?
                        .unwrap_or(DEFAULT_COMPLETED_RETENTION_DAYS),
                ),
                attention_effect: attention::normalize_effect(
                    &row.get::<_, Option<String>>(24)?.unwrap_or_default(),
                ),
                attention_until_handled: row.get::<_, Option<i64>>(25)?.unwrap_or(0) == 1,
                attention_repeat_minutes: normalize_attention_repeat_minutes(
                    row.get::<_, Option<i64>>(26)?.unwrap_or(0),
                ),
                notification_screen: attention::normalize_screen(
                    &row.get::<_, Option<String>>(27)?.unwrap_or_default(),
                ),
                attention_intensity: attention::normalize_intensity(
                    row.get::<_, Option<i64>>(28)?
                        .unwrap_or(attention::INTENSITY_DEFAULT),
                ),
                attention_color: attention::normalize_color(
                    &row.get::<_, Option<String>>(29)?.unwrap_or_default(),
                ),
                auto_backup_keep: normalize_auto_backup_keep(
                    row.get::<_, Option<i64>>(30)?
                        .unwrap_or(DEFAULT_AUTO_BACKUP_KEEP),
                ),
                app_mode,
                cloud_server_url: row.get::<_, Option<String>>(32)?.unwrap_or_default(),
                cloud_user_id: row.get::<_, Option<String>>(33)?.unwrap_or_default(),
                cloud_username: row.get::<_, Option<String>>(34)?.unwrap_or_default(),
                cloud_nickname: row.get::<_, Option<String>>(35)?.unwrap_or_default(),
                cloud_user_role: row.get::<_, Option<String>>(36)?.unwrap_or_default(),
                cloud_must_change_password: row.get::<_, Option<i64>>(37)?.unwrap_or(0) == 1,
                last_merged_account_id: row.get::<_, Option<String>>(38)?.unwrap_or_default(),
                last_merged_username: row.get::<_, Option<String>>(39)?.unwrap_or_default(),
            })
        })?;
        Ok(settings)
    }

    /// 保存常规应用设置（不覆盖由云端引擎管理的账号/模式/合并元数据字段）。
    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), AppError> {
        let conn = self.get_settings_conn()?;
        conn.execute(
            "UPDATE settings
             SET auto_start_enabled = ?, sound_enabled = ?, snooze_minutes = ?,
                 sticky_note_enabled = ?, sticky_note_content = ?, sticky_note_width = ?, sticky_note_height = ?,
                 sticky_note_x = ?, sticky_note_y = ?, sticky_note_opacity = ?, window_opacity = ?,
                 notification_theme = ?,
                 quick_add_enabled = ?, quick_add_shortcut = ?,
                 quiet_hours_enabled = ?, quiet_hours_start = ?, quiet_hours_end = ?,
                 native_notification_enabled = ?, sticky_toggle_shortcut = ?,
                 sticky_snap_enabled = ?, holiday_auto_update = ?, completed_retention_days = ?,
                 attention_effect = ?, attention_until_handled = ?, attention_repeat_minutes = ?,
                 notification_screen = ?, attention_intensity = ?, attention_color = ?,
                 auto_backup_keep = ?
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
                settings.notification_theme,
                if settings.quick_add_enabled { 1 } else { 0 },
                settings.quick_add_shortcut.trim(),
                if settings.quiet_hours_enabled { 1 } else { 0 },
                quiet_hours::normalize_clock(&settings.quiet_hours_start, quiet_hours::DEFAULT_START),
                quiet_hours::normalize_clock(&settings.quiet_hours_end, quiet_hours::DEFAULT_END),
                if settings.native_notification_enabled { 1 } else { 0 },
                settings.sticky_toggle_shortcut.trim(),
                if settings.sticky_snap_enabled { 1 } else { 0 },
                if settings.holiday_auto_update { 1 } else { 0 },
                normalize_completed_retention_days(settings.completed_retention_days),
                attention::normalize_effect(&settings.attention_effect),
                if settings.attention_until_handled { 1 } else { 0 },
                normalize_attention_repeat_minutes(settings.attention_repeat_minutes),
                attention::normalize_screen(&settings.notification_screen),
                attention::normalize_intensity(settings.attention_intensity),
                attention::normalize_color(&settings.attention_color),
                normalize_auto_backup_keep(settings.auto_backup_keep),
            ],
        )?;
        Ok(())
    }

    pub fn set_app_mode(&self, mode: AppMode) -> Result<(), AppError> {
        let conn = self.get_settings_conn()?;
        conn.execute(
            "UPDATE settings SET app_mode = ? WHERE id = 1",
            [mode.as_str()],
        )?;
        Ok(())
    }

    pub fn set_cloud_server_url(&self, url: &str) -> Result<(), AppError> {
        let conn = self.get_settings_conn()?;
        conn.execute(
            "UPDATE settings SET cloud_server_url = ? WHERE id = 1",
            [url.trim()],
        )?;
        Ok(())
    }

    pub fn save_cloud_account(
        &self,
        user_id: &str,
        username: &str,
        nickname: &str,
        role: &str,
        must_change_password: bool,
    ) -> Result<(), AppError> {
        let conn = self.get_settings_conn()?;
        conn.execute(
            "UPDATE settings
             SET cloud_user_id = ?, cloud_username = ?, cloud_nickname = ?,
                 cloud_user_role = ?, cloud_must_change_password = ?
             WHERE id = 1",
            params![
                user_id,
                username,
                nickname,
                role,
                if must_change_password { 1 } else { 0 }
            ],
        )?;
        Ok(())
    }

    pub fn clear_cloud_account(&self) -> Result<(), AppError> {
        let conn = self.get_settings_conn()?;
        conn.execute(
            "UPDATE settings
             SET cloud_user_id = '', cloud_username = '', cloud_nickname = '',
                 cloud_user_role = '', cloud_must_change_password = 0
             WHERE id = 1",
            [],
        )?;
        Ok(())
    }

    pub fn record_last_merged_account(
        &self,
        account_id: &str,
        username: &str,
    ) -> Result<(), AppError> {
        let conn = self.get_settings_conn()?;
        conn.execute(
            "UPDATE settings SET last_merged_account_id = ?, last_merged_username = ? WHERE id = 1",
            params![account_id, username],
        )?;
        Ok(())
    }
}
