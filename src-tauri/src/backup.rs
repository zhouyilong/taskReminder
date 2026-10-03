//! 导入导出与本地自动备份。
//!
//! - 导出：JSON 全量备份（可再导入）、Markdown 待办清单、ICS 日历（导入系统日历）。
//! - 导入：按 id 合并 JSON 备份，与云同步一致采用“较新的版本胜出”，不会覆盖本地更新的修改。
//! - 自动备份：每天用 `VACUUM INTO` 在数据目录的 `backups/` 下保存一份数据库快照，保留最近 7 份；
//!   可以把某份快照按同样的规则合并回当前数据库。

use std::path::{Path, PathBuf};

use chrono::{Duration, NaiveDateTime};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::DbManager;
use crate::errors::AppError;
use crate::kinds::{RepeatMode, TaskStatus};
use crate::models::{RecurringTask, ReminderRecord, Task, PRIORITY_MAX};
use crate::recurrence::{self, weekday_mask};
use crate::time::{self, format_datetime, parse_datetime_any};

pub const BACKUP_APP_ID: &str = "TaskReminder";
pub const BACKUP_FORMAT_VERSION: u32 = 1;
pub const AUTO_BACKUP_KEEP: usize = 7;
const AUTO_BACKUP_PREFIX: &str = "taskreminder-";
const AUTO_BACKUP_DIR: &str = "backups";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub app: String,
    pub format_version: u32,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub exported_at: String,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub recurring_tasks: Vec<RecurringTask>,
    #[serde(default)]
    pub reminder_records: Vec<ReminderRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Markdown,
    Ics,
}

impl ExportFormat {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value.trim().to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "markdown" | "md" => Ok(Self::Markdown),
            "ics" | "ical" => Ok(Self::Ics),
            other => Err(AppError::Invalid(format!("不支持的导出格式：{}", other))),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
            Self::Ics => "ics",
        }
    }

    pub fn filter_name(self) -> &'static str {
        match self {
            Self::Json => "JSON 备份",
            Self::Markdown => "Markdown",
            Self::Ics => "iCalendar 日历",
        }
    }

    pub fn default_file_name(self, now: &NaiveDateTime) -> String {
        let stem = match self {
            Self::Json => "任务提醒备份",
            Self::Markdown => "任务提醒待办",
            Self::Ics => "任务提醒日历",
        };
        format!(
            "{}-{}.{}",
            stem,
            time::format_compact_date(now),
            self.extension()
        )
    }
}

/// 读取当前数据（不含已删除的行）组成备份。
pub fn build_backup(db: &DbManager, app_version: &str) -> Result<BackupFile, AppError> {
    let mut tasks = db.list_active_tasks()?;
    tasks.extend(db.list_completed_tasks()?);
    Ok(BackupFile {
        app: BACKUP_APP_ID.to_string(),
        format_version: BACKUP_FORMAT_VERSION,
        app_version: app_version.to_string(),
        exported_at: format_datetime(&time::now()),
        tasks,
        recurring_tasks: db.list_recurring_tasks()?,
        reminder_records: db.list_reminder_records()?,
    })
}

pub fn render_json(backup: &BackupFile) -> Result<String, AppError> {
    serde_json::to_string_pretty(backup).map_err(|e| AppError::System(e.to_string()))
}

pub fn parse_backup(content: &str) -> Result<BackupFile, AppError> {
    let backup: BackupFile = serde_json::from_str(content.trim_start_matches('\u{feff}'))
        .map_err(|e| AppError::Invalid(format!("备份文件格式无效：{}", e)))?;
    if backup.app != BACKUP_APP_ID {
        return Err(AppError::Invalid("不是任务提醒导出的备份文件".to_string()));
    }
    if backup.format_version > BACKUP_FORMAT_VERSION {
        return Err(AppError::Invalid(
            "备份文件来自更新的版本，请先升级应用".to_string(),
        ));
    }
    Ok(backup)
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub inserted: usize,
    pub updated: usize,
    /// 本地已有相同或更新的版本、或数据无效而跳过的条数。
    pub skipped: usize,
}

/// 按 id 合并备份，返回统计。循环提醒的下次触发时间若已过去，按当前时间重新计算。
pub fn import_backup(db: &DbManager, backup: &BackupFile) -> Result<ImportSummary, AppError> {
    let now = time::now();
    let recurring: Vec<RecurringTask> = backup
        .recurring_tasks
        .iter()
        .filter_map(|task| {
            let mut task = task.clone();
            recurrence::sanitize_recurring_task(&mut task).ok()?;
            let stale = parse_datetime_any(&task.next_trigger).is_none_or(|next| next <= now);
            if stale {
                task.next_trigger = recurrence::compute_next_trigger(&task, Some(now)).ok()?;
            }
            Some(task)
        })
        .collect();
    let invalid = backup.recurring_tasks.len() - recurring.len();
    let mut summary = db.import_rows(&backup.tasks, &recurring, &backup.reminder_records)?;
    summary.skipped += invalid;
    Ok(summary)
}

fn priority_text(priority: i64) -> &'static str {
    match priority.clamp(0, PRIORITY_MAX) {
        3 => "高",
        2 => "中",
        1 => "低",
        _ => "",
    }
}

const WEEKDAY_NAMES: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

fn weekday_text(mask: i64) -> String {
    match mask & recurrence::WEEKDAY_MASK_ALL {
        0b111_1111 => "每天".to_string(),
        0b001_1111 => "周一至周五".to_string(),
        0b110_0000 => "周末".to_string(),
        mask => {
            let days: Vec<&str> = (0..7)
                .filter(|index| mask & (1 << index) != 0)
                .map(|index| WEEKDAY_NAMES[index])
                .collect();
            format!("每周{}", days.join("、"))
        }
    }
}

/// 循环规则的中文描述，与前端 `formatRecurringRule` 一致。
pub fn describe_rule(task: &RecurringTask) -> String {
    let time = task.schedule_time.as_deref().unwrap_or("-");
    match &task.repeat_mode {
        RepeatMode::Daily => format!("每天 {}", time),
        RepeatMode::Workday => format!("法定工作日 {}", time),
        RepeatMode::MonthlyLastDay => format!("每月最后一天 {}", time),
        RepeatMode::MonthlyLastWorkday => format!("每月最后一个工作日 {}", time),
        RepeatMode::Weekly => {
            format!("{} {}", weekday_text(weekday_mask(task).unwrap_or(0)), time)
        }
        RepeatMode::Monthly => format!(
            "每月 {} 日 {}",
            task.schedule_day
                .map_or_else(|| "-".to_string(), |d| d.to_string()),
            time
        ),
        RepeatMode::Cron => {
            format!("Cron {}", task.cron_expression.as_deref().unwrap_or("-"))
        }
        RepeatMode::Unknown(mode) => format!("不支持的循环模式（{}）", mode),
        RepeatMode::IntervalRange => format!(
            "每 {} 分钟（{} - {}）",
            task.interval_minutes,
            task.start_time.as_deref().unwrap_or("00:00"),
            task.end_time.as_deref().unwrap_or("23:59")
        ),
    }
}

fn short_time(value: &str) -> String {
    value.replace('T', " ").chars().take(16).collect()
}

fn markdown_note(content: Option<&str>) -> String {
    let Some(content) = content.map(str::trim).filter(|value| !value.is_empty()) else {
        return String::new();
    };
    content
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("  {}", line.trim_end())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn markdown_task_line(task: &Task, show_project: bool) -> String {
    let done = task.status == TaskStatus::Completed;
    let mut parts = vec![format!(
        "- [{}] {}",
        if done { "x" } else { " " },
        task.description.trim()
    )];
    if show_project && !task.project.is_empty() {
        parts.push(format!("@{}", task.project));
    }
    if let Some(due) = task.due_at.as_deref() {
        parts.push(format!("📅 截止 {}", short_time(due)));
    }
    if let Some(reminder) = task.reminder_time.as_deref() {
        parts.push(format!("⏰ {}", short_time(reminder)));
    }
    if done {
        if let Some(completed) = task.completed_at.as_deref() {
            parts.push(format!("✓ {}", short_time(completed)));
        }
    }
    let priority = priority_text(task.priority);
    if !priority.is_empty() {
        parts.push(format!("优先级：{}", priority));
    }
    if !task.tags.is_empty() {
        parts.push(
            task.tags
                .iter()
                .map(|tag| format!("#{}", tag))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    format!(
        "{}\n{}",
        parts.join(" · "),
        markdown_note(task.sticky_content.as_deref())
    )
}

pub fn render_markdown(backup: &BackupFile) -> String {
    let mut pending: Vec<&Task> = backup
        .tasks
        .iter()
        .filter(|task| task.status != TaskStatus::Completed)
        .collect();
    pending.sort_by(|a, b| {
        b.priority.cmp(&a.priority).then_with(|| {
            let left = a.reminder_time.as_deref().unwrap_or("9999");
            let right = b.reminder_time.as_deref().unwrap_or("9999");
            left.cmp(right)
        })
    });
    let mut completed: Vec<&Task> = backup
        .tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Completed)
        .collect();
    completed.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));

    let mut out = String::new();
    out.push_str("# 任务提醒\n\n");
    out.push_str(&format!(
        "> 导出时间：{}\n\n",
        short_time(&backup.exported_at)
    ));

    out.push_str(&format!("## 待办（{}）\n\n", pending.len()));
    if pending.is_empty() {
        out.push_str("暂无待办。\n");
    }
    // 有项目（v2.2）时按项目分组：项目按名称排序，未分组的排在最后。
    if pending.iter().any(|task| !task.project.is_empty()) {
        let mut projects: Vec<&str> = pending.iter().map(|task| task.project.as_str()).collect();
        projects.sort_by(|a, b| a.is_empty().cmp(&b.is_empty()).then_with(|| a.cmp(b)));
        projects.dedup();
        for project in projects {
            let group: Vec<&&Task> = pending
                .iter()
                .filter(|task| task.project == project)
                .collect();
            let name = if project.is_empty() {
                "未分组"
            } else {
                project
            };
            out.push_str(&format!("### {}（{}）\n\n", name, group.len()));
            for task in group {
                out.push_str(&markdown_task_line(task, false));
            }
            out.push('\n');
        }
    } else {
        for task in &pending {
            out.push_str(&markdown_task_line(task, false));
        }
    }

    out.push_str(&format!(
        "\n## 循环提醒（{}）\n\n",
        backup.recurring_tasks.len()
    ));
    if backup.recurring_tasks.is_empty() {
        out.push_str("暂无循环提醒。\n");
    }
    for task in &backup.recurring_tasks {
        let end = recurrence::describe_end(task)
            .map(|text| format!(" · {}", text))
            .unwrap_or_default();
        let state = if !task.is_paused {
            ""
        } else if recurrence::has_ended(task) {
            "（已结束）"
        } else {
            "（已暂停）"
        };
        out.push_str(&format!(
            "- {} — {}{}{}\n",
            task.description.trim(),
            describe_rule(task),
            end,
            state
        ));
    }

    out.push_str(&format!("\n## 已完成（{}）\n\n", completed.len()));
    if completed.is_empty() {
        out.push_str("暂无已完成的待办。\n");
    }
    for task in &completed {
        out.push_str(&markdown_task_line(task, true));
    }
    out
}

/// iCalendar 文本转义（RFC 5545 3.3.11）。
fn ics_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out
}

/// 按 75 字节折行（不拆分 UTF-8 字符），续行以空格开头，行尾 CRLF。
fn ics_fold(line: &str) -> String {
    let mut out = String::new();
    let mut current = 0usize;
    for ch in line.chars() {
        let len = ch.len_utf8();
        if current + len > 75 {
            out.push_str("\r\n ");
            current = 1;
        }
        out.push(ch);
        current += len;
    }
    out.push_str("\r\n");
    out
}

fn ics_datetime(value: &NaiveDateTime) -> String {
    time::format_ics_local(value)
}

const ICS_WEEKDAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

fn ics_rrule(task: &RecurringTask) -> Option<String> {
    match &task.repeat_mode {
        RepeatMode::Daily => Some("FREQ=DAILY".to_string()),
        // 日历不认识调休，按周一至周五近似。
        RepeatMode::Workday => Some("FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR".to_string()),
        RepeatMode::MonthlyLastDay => Some("FREQ=MONTHLY;BYMONTHDAY=-1".to_string()),
        // 同样不认识调休：取每月最后一个周一至周五。
        RepeatMode::MonthlyLastWorkday => {
            Some("FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1".to_string())
        }
        RepeatMode::Weekly => {
            let mask = weekday_mask(task)?;
            let days: Vec<&str> = (0..7)
                .filter(|index| mask & (1 << index) != 0)
                .map(|index| ICS_WEEKDAYS[index])
                .collect();
            Some(format!("FREQ=WEEKLY;BYDAY={}", days.join(",")))
        }
        RepeatMode::Monthly => {
            let day = task.schedule_day?.clamp(1, 31);
            if day <= 28 {
                Some(format!("FREQ=MONTHLY;BYMONTHDAY={}", day))
            } else {
                // 与应用一致：当月没有这一天时在月末提醒。
                let days: Vec<String> = (28..=day).map(|d| d.to_string()).collect();
                Some(format!(
                    "FREQ=MONTHLY;BYMONTHDAY={};BYSETPOS=-1",
                    days.join(",")
                ))
            }
        }
        _ => None,
    }
    .map(|rule| match ics_end_clause(task) {
        Some(end) => format!("{};{}", rule, end),
        None => rule,
    })
}

/// 结束条件（v2.2）对应的 `UNTIL` / `COUNT`（两者不能同时出现）：同时设置时按先到的一个。
fn ics_end_clause(task: &RecurringTask) -> Option<String> {
    let until = task
        .ends_on
        .as_deref()
        .and_then(time::parse_date)
        .and_then(|date| date.and_hms_opt(23, 59, 59));
    let count = task.remaining_count.map(|count| count.max(0) as usize);
    let until_clause = |value: NaiveDateTime| format!("UNTIL={}", ics_datetime(&value));
    match (until, count) {
        (None, None) => None,
        (Some(until), None) => Some(until_clause(until)),
        (None, Some(count)) => Some(format!("COUNT={}", count)),
        (Some(until), Some(count)) => {
            let before_until = recurrence::upcoming_triggers(task, until, count)
                .map(|times| times.len())
                .unwrap_or(count);
            Some(if before_until < count {
                until_clause(until)
            } else {
                format!("COUNT={}", count)
            })
        }
    }
}

struct IcsEvent<'a> {
    uid: String,
    start: NaiveDateTime,
    summary: &'a str,
    description: String,
    rrule: Option<String>,
    categories: Vec<String>,
    priority: i64,
    /// 提醒相对开始时间提前的分钟数；None 表示不提醒（只有截止时间）。
    alarm_minutes_before: Option<i64>,
}

fn push_event(out: &mut String, event: &IcsEvent, dtstamp: &str) {
    let mut lines = vec![
        "BEGIN:VEVENT".to_string(),
        format!("UID:{}", event.uid),
        format!("DTSTAMP:{}", dtstamp),
        format!("DTSTART:{}", ics_datetime(&event.start)),
        format!(
            "DTEND:{}",
            ics_datetime(&(event.start + Duration::minutes(15)))
        ),
        format!("SUMMARY:{}", ics_escape(event.summary.trim())),
    ];
    if let Some(rrule) = &event.rrule {
        lines.push(format!("RRULE:{}", rrule));
    }
    if !event.description.trim().is_empty() {
        lines.push(format!(
            "DESCRIPTION:{}",
            ics_escape(event.description.trim())
        ));
    }
    if !event.categories.is_empty() {
        let categories: Vec<String> = event.categories.iter().map(|c| ics_escape(c)).collect();
        lines.push(format!("CATEGORIES:{}", categories.join(",")));
    }
    // iCalendar 优先级：1 最高，9 最低。
    match event.priority.clamp(0, PRIORITY_MAX) {
        3 => lines.push("PRIORITY:1".to_string()),
        2 => lines.push("PRIORITY:5".to_string()),
        1 => lines.push("PRIORITY:9".to_string()),
        _ => {}
    }
    if let Some(minutes) = event.alarm_minutes_before {
        lines.extend([
            "BEGIN:VALARM".to_string(),
            "ACTION:DISPLAY".to_string(),
            format!("DESCRIPTION:{}", ics_escape(event.summary.trim())),
            if minutes > 0 {
                format!("TRIGGER:-PT{}M", minutes)
            } else {
                "TRIGGER:PT0S".to_string()
            },
            "END:VALARM".to_string(),
        ]);
    }
    lines.push("END:VEVENT".to_string());
    for line in lines {
        out.push_str(&ics_fold(&line));
    }
}

/// 导出日历：未完成且设了提醒的待办，以及可用 RRULE 表达的循环提醒（每天、工作日、每周、每月）。
/// 返回 ICS 文本与未能导出的循环提醒数量（区间间隔、Cron、已暂停）。
pub fn render_ics(backup: &BackupFile, dtstamp: &str) -> (String, usize) {
    let mut out = String::new();
    for line in [
        "BEGIN:VCALENDAR",
        "VERSION:2.0",
        "PRODID:-//TaskReminder//任务提醒//ZH",
        "CALSCALE:GREGORIAN",
        "METHOD:PUBLISH",
        "X-WR-CALNAME:任务提醒",
    ] {
        out.push_str(&ics_fold(line));
    }

    for task in &backup.tasks {
        if task.status == TaskStatus::Completed {
            continue;
        }
        // 有截止时间（v2.1）时事件放在截止时间，提醒按提前量设置；否则放在提醒时间。
        let reminder = task.reminder_time.as_deref().and_then(parse_datetime_any);
        let due = task.due_at.as_deref().and_then(parse_datetime_any);
        let Some(start) = due.or(reminder) else {
            continue;
        };
        let alarm_minutes_before = reminder.map(|at| (start - at).num_minutes().max(0));
        push_event(
            &mut out,
            &IcsEvent {
                uid: format!("{}@taskreminder", task.id),
                start,
                summary: &task.description,
                description: task.sticky_content.clone().unwrap_or_default(),
                rrule: None,
                // 项目（v2.2）作为第一个分类，其后为标签。
                categories: std::iter::once(task.project.clone())
                    .filter(|project| !project.is_empty())
                    .chain(task.tags.iter().cloned())
                    .collect(),
                priority: task.priority,
                alarm_minutes_before,
            },
            dtstamp,
        );
    }

    let mut skipped = 0;
    for task in &backup.recurring_tasks {
        let rrule = if task.is_paused {
            None
        } else {
            ics_rrule(task)
        };
        let start = parse_datetime_any(&task.next_trigger);
        let (Some(rrule), Some(start)) = (rrule, start) else {
            skipped += 1;
            continue;
        };
        push_event(
            &mut out,
            &IcsEvent {
                uid: format!("{}@taskreminder", task.id),
                start,
                summary: &task.description,
                description: describe_rule(task),
                rrule: Some(rrule),
                categories: task.tags.clone(),
                priority: 0,
                alarm_minutes_before: Some(0),
            },
            dtstamp,
        );
    }
    out.push_str(&ics_fold("END:VCALENDAR"));
    (out, skipped)
}

// ---------- 本地自动备份 ----------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub name: String,
    pub size: u64,
    /// 由文件名解析出的备份时间（本地时间）。
    pub created_at: String,
}

pub fn backup_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join(AUTO_BACKUP_DIR)
}

fn backup_file_name(now: &NaiveDateTime) -> String {
    format!("{}{}.db", AUTO_BACKUP_PREFIX, time::format_stamp(now))
}

/// 解析备份文件名中的时间；不是本应用生成的备份返回 None（也用来防止路径穿越）。
fn parse_backup_name(name: &str) -> Option<NaiveDateTime> {
    let stamp = name.strip_prefix(AUTO_BACKUP_PREFIX)?.strip_suffix(".db")?;
    if stamp.len() != 15 || !stamp.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return None;
    }
    time::parse_stamp(stamp)
}

pub fn list_backups(dir: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut backups: Vec<BackupInfo> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let created = parse_backup_name(&name)?;
            let size = entry.metadata().ok()?.len();
            Some(BackupInfo {
                name,
                size,
                created_at: format_datetime(&created),
            })
        })
        .collect();
    backups.sort_by(|a, b| b.name.cmp(&a.name));
    backups
}

/// 需要删除的旧备份：按文件名（即时间）倒序保留最近 `keep` 份。
fn backups_to_prune(mut names: Vec<String>, keep: usize) -> Vec<String> {
    names.retain(|name| parse_backup_name(name).is_some());
    names.sort_by(|a, b| b.cmp(a));
    names.into_iter().skip(keep).collect()
}

pub fn create_backup(db_path: &Path, now: &NaiveDateTime) -> Result<PathBuf, AppError> {
    let dir = backup_dir(db_path);
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(backup_file_name(now));
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    let conn = Connection::open(db_path)?;
    let escaped = target.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}'", escaped))?;
    prune_backups(&dir, AUTO_BACKUP_KEEP)?;
    Ok(target)
}

pub fn prune_backups(dir: &Path, keep: usize) -> Result<(), AppError> {
    let names = list_backups(dir)
        .into_iter()
        .map(|info| info.name)
        .collect();
    for name in backups_to_prune(names, keep) {
        let _ = std::fs::remove_file(dir.join(name));
    }
    Ok(())
}

/// 今天还没有备份时创建一份。
pub fn ensure_daily_backup(
    db_path: &Path,
    now: &NaiveDateTime,
) -> Result<Option<PathBuf>, AppError> {
    let today = now.date();
    let has_today = list_backups(&backup_dir(db_path))
        .iter()
        .filter_map(|info| parse_backup_name(&info.name))
        .any(|created| created.date() == today);
    if has_today {
        return Ok(None);
    }
    create_backup(db_path, now).map(Some)
}

/// 找到备份文件的完整路径；名字不合法或文件不存在时报错。
pub fn resolve_backup(db_path: &Path, name: &str) -> Result<PathBuf, AppError> {
    if parse_backup_name(name).is_none() {
        return Err(AppError::Invalid("备份文件名无效".to_string()));
    }
    let path = backup_dir(db_path).join(name);
    if !path.is_file() {
        return Err(AppError::Invalid("备份文件不存在".to_string()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::TaskMeta;
    use crate::kinds::{ReminderKind, TaskType};

    fn temp_db() -> (DbManager, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("taskreminder-backup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = DbManager::new(dir.join("test.db")).unwrap();
        (db, dir)
    }

    fn sample_recurring(mode: &str) -> RecurringTask {
        RecurringTask {
            id: String::new(),
            description: "周报".to_string(),
            task_type: TaskType::Recurring,
            status: TaskStatus::Pending,
            created_at: String::new(),
            completed_at: None,
            reminder_time: None,
            updated_at: None,
            deleted_at: None,
            interval_minutes: 60,
            last_triggered: None,
            next_trigger: "2026-10-02T17:30:00".to_string(),
            is_paused: false,
            start_time: None,
            end_time: None,
            repeat_mode: RepeatMode::parse(mode),
            schedule_time: Some("17:30".to_string()),
            schedule_weekday: Some(5),
            schedule_weekdays: Some(0b001_0101),
            schedule_day: Some(31),
            cron_expression: None,
            tags: Vec::new(),
            ends_on: None,
            remaining_count: None,
        }
    }

    #[test]
    fn json_backup_roundtrips_and_import_merges_by_updated_at() {
        let (source, source_dir) = temp_db();
        let meta = TaskMeta {
            tags: vec!["工作".to_string()],
            priority: 2,
            project: String::new(),
        };
        let task = source
            .create_task_with_meta("交周报", Some("- 本周进展"), &meta)
            .unwrap();
        source
            .set_task_reminder_time(&task.id, Some("2099-01-01T09:00:00"))
            .unwrap();
        let mut recurring = sample_recurring("WEEKLY");
        recurring.next_trigger = "2000-01-07T17:30:00".to_string();
        source.create_recurring_task(&recurring).unwrap();
        source
            .create_reminder_record(&task.id, "交周报", ReminderKind::Task)
            .unwrap();

        let backup = build_backup(&source, "2.0.0").unwrap();
        let json = render_json(&backup).unwrap();
        let parsed = parse_backup(&json).unwrap();
        assert_eq!(parsed.tasks.len(), 1);
        assert_eq!(parsed.tasks[0].tags, vec!["工作"]);

        let (target, target_dir) = temp_db();
        let summary = import_backup(&target, &parsed).unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                inserted: 3,
                updated: 0,
                skipped: 0
            }
        );
        let imported = target.get_task(&task.id).unwrap().unwrap();
        assert_eq!(imported.description, "交周报");
        assert_eq!(imported.priority, 2);
        assert_eq!(
            imported.reminder_time.as_deref(),
            Some("2099-01-01T09:00:00")
        );
        // 已过去的下次触发时间被重新计算。
        let imported_recurring = &target.list_recurring_tasks().unwrap()[0];
        assert!(imported_recurring.next_trigger.as_str() > "2026-01-01");

        // 再导入一次：没有更新的版本，全部跳过。
        let again = import_backup(&target, &parsed).unwrap();
        assert_eq!(again.inserted + again.updated, 0);
        assert_eq!(again.skipped, 3);

        // 本地更新后导入旧备份不会覆盖本地修改；备份更新时覆盖。
        target
            .update_task(&task.id, "本地修改", None, None, None)
            .unwrap();
        import_backup(&target, &parsed).unwrap();
        assert_eq!(
            target.get_task(&task.id).unwrap().unwrap().description,
            "本地修改"
        );
        let mut newer = parsed.clone();
        newer.tasks[0].description = "备份更新".to_string();
        newer.tasks[0].updated_at = Some("2999-01-01T00:00:00".to_string());
        let summary = import_backup(&target, &newer).unwrap();
        assert_eq!(summary.updated, 1);
        assert_eq!(
            target.get_task(&task.id).unwrap().unwrap().description,
            "备份更新"
        );

        let _ = std::fs::remove_dir_all(source_dir);
        let _ = std::fs::remove_dir_all(target_dir);
    }

    #[test]
    fn parse_backup_rejects_foreign_or_newer_files() {
        assert!(parse_backup("not json").is_err());
        assert!(parse_backup(r#"{"app":"Other","formatVersion":1}"#).is_err());
        assert!(parse_backup(r#"{"app":"TaskReminder","formatVersion":99}"#).is_err());
        let ok = parse_backup("\u{feff}{\"app\":\"TaskReminder\",\"formatVersion\":1}").unwrap();
        assert!(ok.tasks.is_empty());
    }

    #[test]
    fn markdown_lists_pending_recurring_and_completed() {
        let (db, dir) = temp_db();
        let meta = TaskMeta {
            tags: vec!["工作".to_string()],
            priority: 3,
            project: String::new(),
        };
        let task = db
            .create_task_with_meta("交周报", Some("第一行\n\n第二行"), &meta)
            .unwrap();
        db.set_task_reminder_time(&task.id, Some("2026-09-28T15:00:00"))
            .unwrap();
        let done = db.create_task("买菜", None).unwrap();
        db.complete_task(&done.id).unwrap();
        db.create_recurring_task(&sample_recurring("WEEKLY"))
            .unwrap();

        let markdown = render_markdown(&build_backup(&db, "2.0.0").unwrap());
        assert!(markdown.contains("## 待办（1）"));
        assert!(markdown.contains(
            "- [ ] 交周报 · ⏰ 2026-09-28 15:00 · 优先级：高 · #工作\n  第一行\n\n  第二行\n"
        ));
        assert!(markdown.contains("- 周报 — 每周一、三、五 17:30\n"));
        assert!(markdown.contains("## 已完成（1）"));
        assert!(markdown.contains("- [x] 买菜 · ✓ "));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ics_exports_tasks_and_supported_rules() {
        let mut weekly = sample_recurring("WEEKLY");
        weekly.id = "w".to_string();
        let mut monthly = sample_recurring("MONTHLY");
        monthly.id = "m".to_string();
        let mut interval = sample_recurring("INTERVAL_RANGE");
        interval.id = "i".to_string();
        let mut paused = sample_recurring("DAILY");
        paused.id = "p".to_string();
        paused.is_paused = true;
        let backup = BackupFile {
            app: BACKUP_APP_ID.to_string(),
            format_version: 1,
            app_version: String::new(),
            exported_at: String::new(),
            tasks: vec![Task {
                id: "t1".to_string(),
                description: "开会; 带上电脑, 还有\\资料".to_string(),
                sticky_content: Some("议程：\n1. 周报".to_string()),
                task_type: TaskType::OneTime,
                status: TaskStatus::Pending,
                created_at: "2026-09-27T09:00:00".to_string(),
                completed_at: None,
                reminder_time: Some("2026-09-28T15:00:00".to_string()),
                updated_at: None,
                deleted_at: None,
                tags: vec!["工作".to_string()],
                priority: 3,
                due_at: None,
                sticky_color: String::new(),
                sort_order: None,
                project: String::new(),
            }],
            recurring_tasks: vec![weekly, monthly, interval, paused],
            reminder_records: Vec::new(),
        };
        let (ics, skipped) = render_ics(&backup, "20260927T020000Z");
        assert_eq!(skipped, 2);
        assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(ics.ends_with("END:VCALENDAR\r\n"));
        assert!(ics.contains("UID:t1@taskreminder\r\n"));
        assert!(ics.contains("DTSTART:20260928T150000\r\n"));
        assert!(ics.contains("DTEND:20260928T151500\r\n"));
        assert!(ics.contains("SUMMARY:开会\\; 带上电脑\\, 还有\\\\资料\r\n"));
        assert!(ics.contains("DESCRIPTION:议程：\\n1. 周报\r\n"));
        assert!(ics.contains("CATEGORIES:工作\r\n"));
        assert!(ics.contains("PRIORITY:1\r\n"));
        assert!(ics.contains("RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR\r\n"));
        assert!(ics.contains("RRULE:FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1\r\n"));
        assert!(!ics.contains("UID:i@taskreminder"));
        assert!(!ics.contains("UID:p@taskreminder"));
        for line in ics.split("\r\n") {
            assert!(line.len() <= 75, "line too long: {}", line);
        }
    }

    #[test]
    fn ics_uses_due_time_and_lead_for_alarm() {
        let task = |id: &str, due: Option<&str>, reminder: Option<&str>| Task {
            id: id.to_string(),
            description: id.to_string(),
            sticky_content: None,
            task_type: TaskType::OneTime,
            status: TaskStatus::Pending,
            created_at: "2026-09-27T09:00:00".to_string(),
            completed_at: None,
            reminder_time: reminder.map(str::to_string),
            updated_at: None,
            deleted_at: None,
            tags: Vec::new(),
            priority: 0,
            due_at: due.map(str::to_string),
            sticky_color: String::new(),
            sort_order: None,
            project: String::new(),
        };
        let backup = BackupFile {
            app: BACKUP_APP_ID.to_string(),
            format_version: 1,
            app_version: String::new(),
            exported_at: String::new(),
            tasks: vec![
                task(
                    "lead",
                    Some("2026-10-09T18:00:00"),
                    Some("2026-10-09T17:00:00"),
                ),
                task("silent", Some("2026-10-10T09:00:00"), None),
            ],
            recurring_tasks: Vec::new(),
            reminder_records: Vec::new(),
        };
        let (ics, _) = render_ics(&backup, "20260927T020000Z");
        let event = |uid: &str| {
            let start = ics.find(&format!("UID:{}@taskreminder", uid)).unwrap();
            let end = ics[start..].find("END:VEVENT").unwrap();
            ics[start..start + end].to_string()
        };
        let lead = event("lead");
        assert!(lead.contains("DTSTART:20261009T180000"));
        assert!(lead.contains("TRIGGER:-PT60M"));
        let silent = event("silent");
        assert!(silent.contains("DTSTART:20261010T090000"));
        assert!(!silent.contains("BEGIN:VALARM"));
    }

    #[test]
    fn ics_fold_keeps_utf8_characters_intact() {
        let long = format!("SUMMARY:{}", "提醒".repeat(40));
        let folded = ics_fold(&long);
        let unfolded = folded.trim_end_matches("\r\n").replace("\r\n ", "");
        assert_eq!(unfolded, long);
        assert!(folded.split("\r\n").all(|line| line.len() <= 75));
    }

    #[test]
    fn prune_keeps_latest_backups_only() {
        let names = vec![
            "taskreminder-20260920-080000.db".to_string(),
            "taskreminder-20260927-080000.db".to_string(),
            "taskreminder-20260925-080000.db".to_string(),
            "other.db".to_string(),
            "taskreminder-../evil.db".to_string(),
        ];
        assert_eq!(
            backups_to_prune(names, 2),
            vec!["taskreminder-20260920-080000.db".to_string()]
        );
    }

    #[test]
    fn daily_backup_is_created_once_per_day_and_pruned() {
        let (db, dir) = temp_db();
        db.create_task("备份我", None).unwrap();
        let db_path = db.db_path();
        let day = |d: u32| {
            chrono::NaiveDate::from_ymd_opt(2026, 9, d)
                .unwrap()
                .and_hms_opt(8, 0, 0)
                .unwrap()
        };
        assert!(ensure_daily_backup(&db_path, &day(1)).unwrap().is_some());
        assert!(
            ensure_daily_backup(&db_path, &(day(1) + Duration::hours(3)))
                .unwrap()
                .is_none()
        );
        for d in 2..=10 {
            ensure_daily_backup(&db_path, &day(d)).unwrap();
        }
        let backups = list_backups(&backup_dir(&db_path));
        assert_eq!(backups.len(), AUTO_BACKUP_KEEP);
        assert_eq!(backups[0].name, "taskreminder-20260910-080000.db");
        assert_eq!(backups[0].created_at, "2026-09-10T08:00:00");

        // 备份是完整的数据库，可以打开读取。
        let path = resolve_backup(&db_path, &backups[0].name).unwrap();
        let conn = Connection::open(path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert!(resolve_backup(&db_path, "../test.db").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ics_rrule_carries_end_condition() {
        let mut task = sample_recurring("DAILY");
        assert_eq!(ics_rrule(&task).as_deref(), Some("FREQ=DAILY"));
        task.remaining_count = Some(5);
        assert_eq!(ics_rrule(&task).as_deref(), Some("FREQ=DAILY;COUNT=5"));
        // 两者都有时按先到的：10-02 起到 10-04 只有 3 次，早于 10 次。
        task.ends_on = Some("2026-10-04".to_string());
        task.remaining_count = Some(10);
        assert_eq!(
            ics_rrule(&task).as_deref(),
            Some("FREQ=DAILY;UNTIL=20261004T235959")
        );
        task.ends_on = Some("2026-12-31".to_string());
        task.remaining_count = Some(2);
        assert_eq!(ics_rrule(&task).as_deref(), Some("FREQ=DAILY;COUNT=2"));
        task.remaining_count = None;
        assert_eq!(
            ics_rrule(&task).as_deref(),
            Some("FREQ=DAILY;UNTIL=20261231T235959")
        );
    }

    #[test]
    fn markdown_groups_pending_by_project_and_marks_ended_rules() {
        let (db, dir) = temp_db();
        let meta = |project: &str| TaskMeta {
            project: project.to_string(),
            ..TaskMeta::default()
        };
        db.create_task_with_meta("买瓷砖", None, &meta("装修"))
            .unwrap();
        db.create_task_with_meta("交周报", None, &meta("")).unwrap();
        let done = db
            .create_task_with_meta("量尺寸", None, &meta("装修"))
            .unwrap();
        db.complete_task(&done.id).unwrap();
        let mut ended = sample_recurring("DAILY");
        ended.remaining_count = Some(0);
        ended.is_paused = true;
        let ended = db.create_recurring_task(&ended).unwrap();
        db.update_recurring_task(&RecurringTask {
            is_paused: true,
            ..ended
        })
        .unwrap();

        let markdown = render_markdown(&build_backup(&db, "2.2.0").unwrap());
        let decor = markdown.find("### 装修（1）").unwrap();
        let none = markdown.find("### 未分组（1）").unwrap();
        assert!(decor < none);
        assert!(markdown.contains("### 装修（1）\n\n- [ ] 买瓷砖\n"));
        assert!(markdown.contains("- [x] 量尺寸 · @装修 · ✓ "));
        assert!(markdown.contains("- 周报 — 每天 17:30 · 还剩 0 次（已结束）\n"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
