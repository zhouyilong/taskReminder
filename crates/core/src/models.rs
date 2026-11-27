//! 共享核心业务模型与规范化函数。

use serde::{Deserialize, Serialize};

use crate::kinds::{ReminderAction, ReminderKind, RepeatMode, TaskStatus, TaskType};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// 是否加急（v2.2.5），待办清单便签与列表中醒目标识。
    #[serde(default)]
    pub urgent: bool,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// 临时提醒时间（v2.2.4）：到点提醒一次后清空；空为没有临时提醒。
    #[serde(default)]
    pub temp_trigger: Option<String>,
    /// 临时提醒替代的那一次常规提醒的原定时间（v2.2.4）；空为不替代，只多提醒一次。
    #[serde(default)]
    pub temp_replaces: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

/// 待办清单便签（v2.2.4）的窗口状态。本机设置（`settings.todo_sticky_state`，JSON），不参与同步。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TodoStickyState {
    /// 是否打开（重启应用后恢复）。
    pub open: bool,
    /// 窗口位置（逻辑像素）；空为还没有放过，按默认位置显示。
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: f64,
    pub height: f64,
    /// 是否锚定：置顶（v2.2.6 起不再锁定位置）。
    pub pinned: bool,
    /// 清单颜色（v2.2.5），取值同便签颜色（`STICKY_COLORS`），空字符串为默认颜色。
    pub color: String,
}

/// 便签窗口的默认尺寸（逻辑像素，v2.2.6 起为横向长方形，此前为 284 × 280）。
pub const STICKY_NOTE_WIDTH: f64 = 420.0;
pub const STICKY_NOTE_HEIGHT: f64 = 300.0;
/// 2.2.6 之前的默认尺寸，也是 `sticky_width` / `sticky_height` 列的默认值：
/// 关闭状态下仍是这个尺寸的便签（没调整过大小）重新打开时改用新的默认尺寸。
pub const LEGACY_STICKY_NOTE_WIDTH: f64 = 284.0;
pub const LEGACY_STICKY_NOTE_HEIGHT: f64 = 280.0;
pub const STICKY_NOTE_MIN_WIDTH: f64 = 220.0;
pub const STICKY_NOTE_MIN_HEIGHT: f64 = 180.0;

/// 是否为 2.2.6 之前的默认尺寸（没有调整过大小）。
pub fn is_legacy_sticky_size(width: f64, height: f64) -> bool {
    (width - LEGACY_STICKY_NOTE_WIDTH).abs() < 0.5
        && (height - LEGACY_STICKY_NOTE_HEIGHT).abs() < 0.5
}

pub const TODO_STICKY_WIDTH: f64 = 300.0;
pub const TODO_STICKY_HEIGHT: f64 = 420.0;
pub const TODO_STICKY_MIN_WIDTH: f64 = 240.0;
pub const TODO_STICKY_MIN_HEIGHT: f64 = 220.0;

impl Default for TodoStickyState {
    fn default() -> Self {
        Self {
            open: false,
            x: None,
            y: None,
            width: TODO_STICKY_WIDTH,
            height: TODO_STICKY_HEIGHT,
            pinned: false,
            color: String::new(),
        }
    }
}

impl TodoStickyState {
    /// 从设置列解析；空值、无法解析或数值异常的按默认处理。
    pub fn parse(value: &str) -> Self {
        let mut state: Self = serde_json::from_str(value).unwrap_or_default();
        let finite = |v: Option<f64>| v.filter(|v| v.is_finite());
        state.x = finite(state.x);
        state.y = finite(state.y);
        if state.x.is_none() || state.y.is_none() {
            state.x = None;
            state.y = None;
        }
        state.width = if state.width.is_finite() {
            state.width.max(TODO_STICKY_MIN_WIDTH)
        } else {
            TODO_STICKY_WIDTH
        };
        state.height = if state.height.is_finite() {
            state.height.max(TODO_STICKY_MIN_HEIGHT)
        } else {
            TODO_STICKY_HEIGHT
        };
        state.color = normalize_sticky_color(&state.color).unwrap_or_default();
        state
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
