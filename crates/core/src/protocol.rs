//! 客户端与服务端 REST & SSE 协议 DTO 定义。
//!
//! 3 张业务同步行结构（`SyncTaskRow`、`SyncRecurringTaskRow`、`SyncReminderRecordRow`）与
//! SQLite `SYNC_TABLES` 以及 MySQL 业务表字段 1:1 对齐。

use serde::{Deserialize, Serialize};

use crate::kinds::{ReminderAction, ReminderKind, RepeatMode, TaskStatus, TaskType, UserRole};
use crate::lww::effective_compare_time;

/// 当前协议与库结构版本。
pub const SCHEMA_VERSION: &str = "3.0.0";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub schema_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTimeResponse {
    pub unix_millis: i64,
    pub iso_utc: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub role: UserRole,
    pub enabled: bool,
    pub must_change_password: bool,
    pub last_login_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub device_id: String,
    #[serde(default)]
    pub device_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in_seconds: i64,
    pub user: UserInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoutRequest {
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub nickname: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserSummary {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub role: UserRole,
    pub enabled: bool,
    pub must_change_password: bool,
    pub last_login_at: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub task_count: i64,
    #[serde(default)]
    pub recurring_count: i64,
    #[serde(default)]
    pub record_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCreateUserRequest {
    pub username: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub role: UserRole,
    pub password: String,
    #[serde(default)]
    pub must_change_password: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUpdateUserRequest {
    pub nickname: Option<String>,
    pub role: Option<UserRole>,
    pub enabled: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminResetPasswordRequest {
    pub new_password: String,
    #[serde(default = "default_true")]
    pub must_change_password: bool,
}

fn default_true() -> bool {
    true
}

/// 参与同步的 `tasks` 完整行（22 个同步列，不含本机专用的 `sticky_is_pinned`）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncTaskRow {
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
    #[serde(default)]
    pub sticky_content: String,
    #[serde(default = "default_sticky_pos_x")]
    pub sticky_pos_x: f64,
    #[serde(default = "default_sticky_pos_y")]
    pub sticky_pos_y: f64,
    #[serde(default)]
    pub sticky_is_open: bool,
    #[serde(default = "default_sticky_width")]
    pub sticky_width: f64,
    #[serde(default = "default_sticky_height")]
    pub sticky_height: f64,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub sticky_color: String,
    #[serde(default)]
    pub sort_order: Option<f64>,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub urgent: bool,
}

fn default_sticky_pos_x() -> f64 {
    48.0
}

fn default_sticky_pos_y() -> f64 {
    76.0
}

fn default_sticky_width() -> f64 {
    420.0
}

fn default_sticky_height() -> f64 {
    300.0
}

impl SyncTaskRow {
    pub fn compare_time(&self) -> Option<chrono::NaiveDateTime> {
        effective_compare_time(
            self.updated_at.as_deref(),
            self.deleted_at.as_deref(),
            Some(self.created_at.as_str()),
        )
    }
}

/// 参与同步的 `recurring_tasks` 完整行（25 个同步列）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRecurringTaskRow {
    pub id: String,
    pub description: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub interval_minutes: i64,
    pub last_triggered: Option<String>,
    pub next_trigger: String,
    #[serde(default)]
    pub is_paused: bool,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
    #[serde(default = "default_repeat_mode")]
    pub repeat_mode: RepeatMode,
    pub schedule_time: Option<String>,
    pub schedule_weekday: Option<i64>,
    pub schedule_day: Option<i64>,
    pub cron_expression: Option<String>,
    #[serde(default)]
    pub schedule_weekdays: Option<i64>,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub ends_on: Option<String>,
    #[serde(default)]
    pub remaining_count: Option<i64>,
    #[serde(default)]
    pub temp_trigger: Option<String>,
    #[serde(default)]
    pub temp_replaces: Option<String>,
}

fn default_repeat_mode() -> RepeatMode {
    RepeatMode::IntervalRange
}

impl SyncRecurringTaskRow {
    pub fn compare_time(&self) -> Option<chrono::NaiveDateTime> {
        effective_compare_time(
            self.updated_at.as_deref(),
            self.deleted_at.as_deref(),
            Some(self.created_at.as_str()),
        )
    }
}

/// 参与同步的 `reminder_records` 完整行（9 个同步列）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReminderRecordRow {
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

impl SyncReminderRecordRow {
    pub fn compare_time(&self) -> Option<chrono::NaiveDateTime> {
        effective_compare_time(
            self.updated_at.as_deref(),
            self.deleted_at.as_deref(),
            Some(self.trigger_time.as_str()),
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncChangeSet {
    #[serde(default)]
    pub tasks: Vec<SyncTaskRow>,
    #[serde(default)]
    pub recurring_tasks: Vec<SyncRecurringTaskRow>,
    #[serde(default)]
    pub reminder_records: Vec<SyncReminderRecordRow>,
}

impl SyncChangeSet {
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty() && self.recurring_tasks.is_empty() && self.reminder_records.is_empty()
    }

    pub fn total_rows(&self) -> usize {
        self.tasks.len() + self.recurring_tasks.len() + self.reminder_records.len()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPullResponse {
    pub server_seq: i64,
    pub has_more: bool,
    pub changes: SyncChangeSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPushRequest {
    pub device_id: String,
    pub changes: SyncChangeSet,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPushResponse {
    pub server_seq: i64,
    pub accepted: usize,
    pub rejected: usize,
    /// 客户端提交的行落后于服务端时，返回服务端胜出的最新版本供客户端覆盖。
    pub server_wins: SyncChangeSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMergeRequest {
    pub device_id: String,
    pub changes: SyncChangeSet,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMergeSummary {
    pub uploaded: usize,
    pub updated_on_server: usize,
    pub server_wins: usize,
    pub server_only: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMergeResponse {
    pub server_seq: i64,
    pub summary: SyncMergeSummary,
    /// 合并后需要下发到客户端的行（服务端胜出 + 仅服务端存在）。
    pub remote_changes: SyncChangeSet,
}

/// SSE 推送的轻量级变更通知载荷。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEventPayload {
    pub server_seq: i64,
    pub device_id: String,
}

/// 统一的 API 错误体。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}
