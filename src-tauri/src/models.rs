//! 客户端数据模型：核心业务模型从 `task_reminder_core` 复导出，此处补充客户端本机设置与界面载荷。

use serde::{Deserialize, Serialize};

pub use task_reminder_core::models::{
    is_legacy_sticky_size, normalize_priority, normalize_project, normalize_sticky_color,
    normalize_tags, tags_from_db, tags_to_db, RecurringTask, ReminderRecord, StickyNote, Task,
    TodoStickyState, PRIORITY_MAX, STICKY_NOTE_HEIGHT, STICKY_NOTE_MIN_HEIGHT,
    STICKY_NOTE_MIN_WIDTH, STICKY_NOTE_WIDTH, TODO_STICKY_HEIGHT, TODO_STICKY_MIN_HEIGHT,
    TODO_STICKY_MIN_WIDTH, TODO_STICKY_WIDTH,
};

use crate::kinds::{AppMode, ReminderKind};

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub auto_start_enabled: bool,
    pub sound_enabled: bool,
    pub snooze_minutes: i64,
    pub sticky_note_enabled: bool,
    pub sticky_note_content: String,
    pub sticky_note_width: i64,
    pub sticky_note_height: i64,
    pub sticky_note_x: Option<f64>,
    pub sticky_note_y: Option<f64>,
    pub sticky_note_opacity: f64,
    pub window_opacity: f64,
    /// 本机稳定设备标识（沿用旧库 `webdav_device_id` 列，避免升级后设备 ID 变化）。
    #[serde(default)]
    pub device_id: String,
    pub notification_theme: String,
    #[serde(default = "default_true")]
    pub quick_add_enabled: bool,
    #[serde(default = "default_quick_add_shortcut")]
    pub quick_add_shortcut: String,
    /// 勿扰时段：期间提醒只记录不弹窗，结束后一次性弹出。
    #[serde(default)]
    pub quiet_hours_enabled: bool,
    #[serde(default = "default_quiet_hours_start")]
    pub quiet_hours_start: String,
    #[serde(default = "default_quiet_hours_end")]
    pub quiet_hours_end: String,
    /// 弹出提醒时同时发送系统原生通知（全屏程序中也能看到）。
    #[serde(default)]
    pub native_notification_enabled: bool,
    /// 显示/隐藏全部便签的全局快捷键，为空表示不启用。
    #[serde(default)]
    pub sticky_toggle_shortcut: String,
    /// 拖动便签后贴边吸附（屏幕边缘与其他便签）。
    #[serde(default = "default_true")]
    pub sticky_snap_enabled: bool,
    /// 每天从项目仓库检查节假日数据更新。
    #[serde(default = "default_true")]
    pub holiday_auto_update: bool,
    /// 已完成待办的保留天数（`COMPLETED_RETENTION_OPTIONS`），0 为永久保留。
    #[serde(default = "default_completed_retention_days")]
    pub completed_retention_days: i64,
    /// 每日自动备份保留的份数（`AUTO_BACKUP_KEEP_OPTIONS`）。
    #[serde(default = "default_auto_backup_keep")]
    pub auto_backup_keep: i64,
    /// 到点时的全屏注意效果（`attention::EFFECT_*`）。
    #[serde(default = "default_attention_effect")]
    pub attention_effect: String,
    /// 全屏注意效果持续到提醒处理完；否则约 6 秒后淡出。
    #[serde(default)]
    pub attention_until_handled: bool,
    /// 全屏效果的光晕强度（`attention::INTENSITY_*`），越强越向屏幕中间延伸。
    #[serde(default = "default_attention_intensity")]
    pub attention_intensity: i64,
    /// 全屏效果的光晕颜色（`#rrggbb`），空字符串为默认颜色；错过的提醒仍用警示色。
    #[serde(default)]
    pub attention_color: String,
    /// 提醒未处理时每隔几分钟再次提醒（`ATTENTION_REPEAT_OPTIONS`），0 为不重复。
    #[serde(default)]
    pub attention_repeat_minutes: i64,
    /// 提醒弹窗出现的屏幕（`attention::SCREEN_*`）。
    #[serde(default = "default_notification_screen")]
    pub notification_screen: String,
    /// 凭据存放位置（`secrets::STORAGE_*`），只读：保存时由后端决定。
    #[serde(default)]
    pub secret_storage: String,
    /// 当前运行模式：`local`（本地模式）或 `cloud`（云端模式）。
    #[serde(default)]
    pub app_mode: AppMode,
    /// 自定义云端服务地址；为空字符串时使用内置默认官方地址。
    #[serde(default)]
    pub cloud_server_url: String,
    /// 当前登录的云端账号元数据（切回本地模式默认保留登录态，退出登录时清空）。
    #[serde(default)]
    pub cloud_user_id: String,
    #[serde(default)]
    pub cloud_username: String,
    #[serde(default)]
    pub cloud_nickname: String,
    #[serde(default)]
    pub cloud_user_role: String,
    #[serde(default)]
    pub cloud_must_change_password: bool,
    /// 最近一次与本地库 `taskreminder.db` 合并过的账号（防跨账号串号保护）。
    #[serde(default)]
    pub last_merged_account_id: String,
    #[serde(default)]
    pub last_merged_username: String,
}

fn default_quiet_hours_start() -> String {
    crate::quiet_hours::DEFAULT_START.to_string()
}

fn default_quiet_hours_end() -> String {
    crate::quiet_hours::DEFAULT_END.to_string()
}

fn default_true() -> bool {
    true
}

/// 已完成待办保留天数的可选值：30（默认，另限最近 100 条）、90、365、0（永久保留）。
pub const COMPLETED_RETENTION_OPTIONS: &[i64] = &[30, 90, 365, 0];
pub const DEFAULT_COMPLETED_RETENTION_DAYS: i64 = 30;

fn default_completed_retention_days() -> i64 {
    DEFAULT_COMPLETED_RETENTION_DAYS
}

/// 不认识的保留天数按默认值处理。
pub fn normalize_completed_retention_days(value: i64) -> i64 {
    if COMPLETED_RETENTION_OPTIONS.contains(&value) {
        value
    } else {
        DEFAULT_COMPLETED_RETENTION_DAYS
    }
}

/// 每日自动备份保留份数的可选值：7（默认）、14、30。
pub const AUTO_BACKUP_KEEP_OPTIONS: &[i64] = &[7, 14, 30];
pub const DEFAULT_AUTO_BACKUP_KEEP: i64 = 7;

fn default_auto_backup_keep() -> i64 {
    DEFAULT_AUTO_BACKUP_KEEP
}

/// 不认识的保留份数按默认值处理。
pub fn normalize_auto_backup_keep(value: i64) -> i64 {
    if AUTO_BACKUP_KEEP_OPTIONS.contains(&value) {
        value
    } else {
        DEFAULT_AUTO_BACKUP_KEEP
    }
}

/// 提醒弹出与再次提醒用到的本机设置（`DbManager::notification_prefs` 只读这几列）。
#[derive(Clone, Debug, Default)]
pub struct NotificationPrefs {
    pub sound_enabled: bool,
    pub quiet_hours_enabled: bool,
    pub quiet_hours_start: String,
    pub quiet_hours_end: String,
    pub notification_theme: String,
    pub attention_effect: String,
    pub attention_until_handled: bool,
    pub attention_intensity: i64,
    pub attention_color: String,
    pub attention_repeat_minutes: i64,
    pub notification_screen: String,
}

impl From<&AppSettings> for NotificationPrefs {
    fn from(settings: &AppSettings) -> Self {
        Self {
            sound_enabled: settings.sound_enabled,
            quiet_hours_enabled: settings.quiet_hours_enabled,
            quiet_hours_start: settings.quiet_hours_start.clone(),
            quiet_hours_end: settings.quiet_hours_end.clone(),
            notification_theme: settings.notification_theme.clone(),
            attention_effect: settings.attention_effect.clone(),
            attention_until_handled: settings.attention_until_handled,
            attention_intensity: settings.attention_intensity,
            attention_color: settings.attention_color.clone(),
            attention_repeat_minutes: settings.attention_repeat_minutes,
            notification_screen: settings.notification_screen.clone(),
        }
    }
}

impl NotificationPrefs {
    pub fn is_quiet_now(&self) -> bool {
        crate::quiet_hours::is_quiet_window_now(
            self.quiet_hours_enabled,
            &self.quiet_hours_start,
            &self.quiet_hours_end,
        )
    }

    pub fn effect_options(&self) -> crate::attention::EffectOptions {
        crate::attention::EffectOptions {
            effect: self.attention_effect.clone(),
            until_handled: self.attention_until_handled,
            intensity: self.attention_intensity,
            color: self.attention_color.clone(),
            theme: self.notification_theme.clone(),
        }
    }
}

fn default_attention_intensity() -> i64 {
    crate::attention::INTENSITY_DEFAULT
}

fn default_attention_effect() -> String {
    crate::attention::EFFECT_BANNER.to_string()
}

fn default_notification_screen() -> String {
    crate::attention::SCREEN_CURSOR.to_string()
}

/// 未处理时再次提醒的间隔（分钟）可选值，0 为不重复。
pub const ATTENTION_REPEAT_OPTIONS: &[i64] = &[0, 2, 5, 10];

/// 不认识的间隔按“不重复”处理。
pub fn normalize_attention_repeat_minutes(value: i64) -> i64 {
    if ATTENTION_REPEAT_OPTIONS.contains(&value) {
        value
    } else {
        0
    }
}

pub fn default_quick_add_shortcut() -> String {
    "CommandOrControl+Alt+N".to_string()
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
    pub record_id: String,
    pub reminder_id: String,
    pub reminder_type: ReminderKind,
    pub description: String,
    pub snooze_minutes: i64,
    /// 本次提醒原定的触发时间；用于在弹窗中标识“错过的提醒”。
    #[serde(default)]
    pub scheduled_time: Option<String>,
    /// 弹出时是否播放提示音：入队时按“提示音”设置决定，勿扰期间入队的为 false。
    #[serde(default)]
    pub sound: bool,
}

/// 运行模式与云端同步状态（供标题栏、设置页、登录遮罩展示）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    pub mode: AppMode,
    /// `local` | `connected` | `syncing` | `offline` | `auth_required`
    pub state: String,
    pub logged_in: bool,
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub role: String,
    pub must_change_password: bool,
    pub server_url: String,
    pub default_server_url: String,
    pub pending_count: usize,
    pub last_sync_time: Option<String>,
    pub error: Option<String>,
}

/// 业务库有效数据统计（用于模式切换确认弹窗展示）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessDataCounts {
    pub active_tasks: usize,
    pub completed_tasks: usize,
    pub recurring_tasks: usize,
    pub reminder_records: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiStatePayload {
    pub ui_scale: f64,
    pub theme: String,
    pub window_opacity: f64,
}
