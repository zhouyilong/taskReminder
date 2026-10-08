use serde::{Deserialize, Serialize};

use crate::kinds::{ReminderAction, ReminderKind, RepeatMode, TaskStatus, TaskType};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub description: String,
    pub sticky_content: Option<String>,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub reminder_time: Option<String>,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
    /// 标签（已去重、去掉前导 `#`），数据库中以逗号分隔存储。
    #[serde(default)]
    pub tags: Vec<String>,
    /// 优先级：0 无、1 低、2 中、3 高。
    #[serde(default)]
    pub priority: i64,
    /// 截止时间（v2.1）；提醒时间仍是 `reminder_time`（= 截止时间 - 提前量）。
    #[serde(default)]
    pub due_at: Option<String>,
    /// 便签颜色（v2.1），空字符串为默认颜色。
    #[serde(default)]
    pub sticky_color: String,
    /// 手动排序位置（v2.1），越小越靠前；空表示未手动排序。
    #[serde(default)]
    pub sort_order: Option<f64>,
    /// 项目（v2.2），空字符串为未分组；读写经 `normalize_project`。
    #[serde(default)]
    pub project: String,
}

pub const MAX_PROJECT_CHARS: usize = 32;

/// 规范化项目名：去掉首尾空白、前导 `@`、控制字符，截断过长的名字。前端 `normalizeProject` 与之一致。
pub fn normalize_project(value: &str) -> String {
    value
        .trim()
        .trim_start_matches(['@', '＠'])
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_PROJECT_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

pub const PRIORITY_MAX: i64 = 3;

/// 便签可选颜色（空字符串为默认颜色）。不认识的值原样保存、按默认颜色显示。
pub const STICKY_COLORS: &[&str] = &["yellow", "blue", "green", "pink", "purple", "gray"];

/// 规范化便签颜色：认识的颜色或空字符串，其余返回 None（命令中拒绝写入）。
pub fn normalize_sticky_color(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    (value.is_empty() || STICKY_COLORS.contains(&value.as_str())).then_some(value)
}
pub const MAX_TAGS_PER_TASK: usize = 10;
pub const MAX_TAG_CHARS: usize = 24;

/// 规范化标签：去掉首尾空白与前导 `#`，去掉逗号（存储分隔符），按不区分大小写去重，
/// 截断过长的标签并限制数量。
pub fn normalize_tags<I, S>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut result: Vec<String> = Vec::new();
    for raw in tags {
        let cleaned: String = raw
            .as_ref()
            .trim()
            .trim_start_matches(['#', '＃'])
            .chars()
            .filter(|c| *c != ',' && *c != '，' && !c.is_control())
            .take(MAX_TAG_CHARS)
            .collect::<String>()
            .trim()
            .to_string();
        if cleaned.is_empty() {
            continue;
        }
        if result
            .iter()
            .any(|existing| existing.to_lowercase() == cleaned.to_lowercase())
        {
            continue;
        }
        result.push(cleaned);
        if result.len() >= MAX_TAGS_PER_TASK {
            break;
        }
    }
    result
}

pub fn tags_to_db(tags: &[String]) -> String {
    normalize_tags(tags).join(",")
}

pub fn tags_from_db(value: Option<String>) -> Vec<String> {
    normalize_tags(value.unwrap_or_default().split(','))
}

pub fn normalize_priority(value: i64) -> i64 {
    value.clamp(0, PRIORITY_MAX)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringTask {
    pub id: String,
    pub description: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub reminder_time: Option<String>,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
    pub interval_minutes: i64,
    pub last_triggered: Option<String>,
    pub next_trigger: String,
    pub is_paused: bool,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub repeat_mode: RepeatMode,
    pub schedule_time: Option<String>,
    pub schedule_weekday: Option<i64>,
    /// 每周多天位掩码（周一 = bit0 … 周日 = bit6）；`schedule_weekday` 保留为其中最早的一天以兼容旧版本。
    #[serde(default)]
    pub schedule_weekdays: Option<i64>,
    pub schedule_day: Option<i64>,
    pub cron_expression: Option<String>,
    /// 标签（v2.1），与待办相同的规范化。
    #[serde(default)]
    pub tags: Vec<String>,
    /// 结束日期（v2.2，`YYYY-MM-DD`，含当天）；空为不限。
    #[serde(default)]
    pub ends_on: Option<String>,
    /// 剩余提醒次数（v2.2），每次触发减一，到 0 时结束；空为不限。
    #[serde(default)]
    pub remaining_count: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderRecord {
    pub id: String,
    pub reminder_id: String,
    pub description: String,
    #[serde(rename = "type")]
    pub reminder_type: ReminderKind,
    pub trigger_time: String,
    pub close_time: Option<String>,
    pub action: ReminderAction,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickyNote {
    pub task_id: String,
    pub title: String,
    pub note_type: String,
    pub content: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub width: f64,
    pub height: f64,
    pub is_open: bool,
    pub is_pinned: bool,
    pub created_at: String,
    pub updated_at: String,
    pub reminder_time: Option<String>,
    /// 便签颜色（v2.1），空字符串为默认颜色。
    #[serde(default)]
    pub color: String,
}

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
    pub webdav_enabled: bool,
    pub webdav_url: String,
    pub webdav_username: String,
    pub webdav_password: String,
    pub webdav_root_path: String,
    pub webdav_sync_interval_minutes: i64,
    pub webdav_last_sync_time: Option<String>,
    pub webdav_last_local_change_time: Option<String>,
    pub webdav_last_sync_status: Option<String>,
    pub webdav_last_sync_error: Option<String>,
    pub webdav_device_id: String,
    pub notification_theme: String,
    #[serde(default = "default_true")]
    pub quick_add_enabled: bool,
    #[serde(default = "default_quick_add_shortcut")]
    pub quick_add_shortcut: String,
    /// 云同步端到端加密：上传前用同步密码加密快照。
    #[serde(default)]
    pub sync_encryption_enabled: bool,
    /// 同步密码，只保存在本机；上传的快照中会被清空。
    #[serde(default)]
    pub sync_passphrase: String,
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
    /// 密码存放位置（`secrets::STORAGE_*`），只读：保存时由后端决定。
    #[serde(default)]
    pub secret_storage: String,
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub status: String,
    pub error: Option<String>,
    pub time: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiStatePayload {
    pub ui_scale: f64,
    pub theme: String,
    pub window_opacity: f64,
}
