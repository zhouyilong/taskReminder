use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub description: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub reminder_time: Option<String>,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringTask {
    pub id: String,
    pub description: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub status: String,
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
    pub repeat_mode: String,
    pub schedule_time: Option<String>,
    pub schedule_weekday: Option<i64>,
    pub schedule_day: Option<i64>,
    pub cron_expression: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderRecord {
    pub id: String,
    pub reminder_id: String,
    pub description: String,
    #[serde(rename = "type")]
    pub reminder_type: String,
    pub trigger_time: String,
    pub close_time: Option<String>,
    pub action: String,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickyNote {
    pub task_id: String,
    pub content: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub is_open: bool,
    pub created_at: String,
    pub updated_at: String,
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
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
    pub record_id: String,
    pub reminder_id: String,
    pub reminder_type: String,
    pub description: String,
    pub snooze_minutes: i64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub status: String,
    pub error: Option<String>,
    pub time: Option<String>,
}
