//! 待办相关命令：列表、新建（含快速添加）、编辑、完成与删除、批量操作。

use serde::Deserialize;
use tauri::{Emitter, State};

use crate::commands::{into_api, ApiResult};
use crate::db::{TaskBatchOp, TaskMeta};
use crate::errors::AppError;
use crate::kinds::{ReminderAction, TaskStatus};
use crate::models::Task;
use crate::scheduler;
use crate::state::AppState;
use crate::windows::sticky::{emit_sticky_note_reminder, hide_sticky_note_window};

pub fn normalize_reminder_time(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdatePayload {
    id: String,
    description: String,
    sticky_content: Option<String>,
    reminder_time: Option<String>,
    /// 为空时保留原有标签与优先级。
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    priority: Option<i64>,
    /// 截止时间：省略时保留原值，`null` 清除。
    #[serde(default, deserialize_with = "present")]
    due_at: Option<Option<String>>,
    /// 项目（v2.2）：省略时保留原值，空字符串移出项目。
    #[serde(default)]
    project: Option<String>,
}

/// 区分“字段省略”（外层 `None`，由 `default` 提供）与“显式为 null”（`Some(None)`）。
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// 校验截止时间格式（`YYYY-MM-DDTHH:MM[:SS]`）并规范化，空字符串视为清除。
fn normalize_due(value: Option<String>) -> Result<Option<String>, String> {
    match normalize_reminder_time(value) {
        None => Ok(None),
        Some(raw) => crate::time::parse_datetime_any(&raw)
            .map(|parsed| Some(crate::time::format_datetime(&parsed)))
            .ok_or_else(|| "截止时间格式无效".to_string()),
    }
}

impl TaskUpdatePayload {
    fn meta(&self) -> Option<TaskMeta> {
        if self.tags.is_none() && self.priority.is_none() {
            return None;
        }
        Some(TaskMeta {
            tags: self.tags.clone().unwrap_or_default(),
            priority: self.priority.unwrap_or(0),
            // 更新时项目单独处理（`set_task_project`），这里不使用。
            project: String::new(),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskPayload {
    description: String,
    sticky_content: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    priority: i64,
    /// 创建时一并设置提醒（自然语言输入识别出的时间）。
    #[serde(default)]
    reminder_time: Option<String>,
    /// 截止时间（v2.1）。
    #[serde(default)]
    due_at: Option<String>,
    /// 项目（v2.2）。
    #[serde(default)]
    project: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAddPayload {
    description: String,
    reminder_time: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    priority: i64,
    #[serde(default)]
    due_at: Option<String>,
    #[serde(default)]
    project: String,
}

#[tauri::command]
pub fn list_active_tasks(state: State<AppState>) -> ApiResult<Vec<Task>> {
    into_api(state.db.list_active_tasks())
}

#[tauri::command]
pub fn list_completed_tasks(state: State<AppState>) -> ApiResult<Vec<Task>> {
    into_api(state.db.list_completed_tasks())
}

#[tauri::command]
pub fn create_task(state: State<AppState>, payload: CreateTaskPayload) -> ApiResult<Task> {
    let meta = TaskMeta {
        tags: payload.tags,
        priority: payload.priority,
        project: payload.project,
    };
    create_task_with_reminder(
        &state,
        payload.description.trim(),
        payload.sticky_content.as_deref(),
        payload.reminder_time,
        payload.due_at,
        &meta,
    )
}

/// 快速添加窗口：一次完成创建待办与设置提醒，并通知主窗口刷新。
#[tauri::command]
pub fn quick_add_task(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: QuickAddPayload,
) -> ApiResult<Task> {
    let meta = TaskMeta {
        tags: payload.tags,
        priority: payload.priority,
        project: payload.project,
    };
    let task = create_task_with_reminder(
        &state,
        payload.description.trim(),
        None,
        payload.reminder_time,
        payload.due_at,
        &meta,
    )?;
    let _ = app.emit("data-updated", ());
    Ok(task)
}

fn create_task_with_reminder(
    state: &AppState,
    description: &str,
    sticky_content: Option<&str>,
    reminder_time: Option<String>,
    due_at: Option<String>,
    meta: &TaskMeta,
) -> ApiResult<Task> {
    if description.is_empty() {
        return Err("待办内容不能为空".to_string());
    }
    let due_at = normalize_due(due_at)?;
    let reminder_time = normalize_reminder_time(reminder_time);
    if let Some(value) = reminder_time.as_deref() {
        if !into_api(scheduler::is_future(value))? {
            return Err("提醒时间需晚于当前时间".to_string());
        }
    }
    let mut task = into_api(
        state
            .db
            .create_task_with_meta(description, sticky_content, meta),
    )?;
    if let Some(reminder_time) = reminder_time {
        into_api(
            state
                .db
                .set_task_reminder_time(&task.id, Some(reminder_time.as_str())),
        )?;
        task.reminder_time = Some(reminder_time);
        into_api(state.scheduler.schedule_task(task.clone()))?;
    }
    if let Some(due) = due_at {
        into_api(state.db.set_task_due(&task.id, Some(due.as_str())))?;
        task.due_at = Some(due);
    }
    into_api(state.sync.notify_local_change())?;
    Ok(task)
}

#[tauri::command]
pub fn update_task(
    app: tauri::AppHandle,
    state: State<AppState>,
    task: TaskUpdatePayload,
) -> ApiResult<()> {
    let reminder_time = task.reminder_time.clone();
    let meta = task.meta();
    let due_at = match task.due_at.clone() {
        Some(value) => Some(normalize_due(value)?),
        None => None,
    };
    into_api(state.db.update_task(
        &task.id,
        task.description.trim(),
        task.sticky_content.clone(),
        reminder_time.clone(),
        meta.as_ref(),
    ))?;
    if let Some(due) = due_at {
        into_api(state.db.set_task_due(&task.id, due.as_deref()))?;
    }
    if let Some(project) = task.project.as_deref() {
        into_api(state.db.set_task_project(&task.id, project))?;
    }
    state.scheduler.cancel_task(&task.id);
    if let Some(reminder_time) = reminder_time.clone() {
        if scheduler::is_future(&reminder_time).unwrap_or(false) {
            if let Some(updated) = into_api(state.db.get_task(&task.id))? {
                into_api(state.scheduler.schedule_task(updated))?;
            }
        }
    }
    emit_sticky_note_reminder(&app, &task.id, reminder_time);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn complete_task(app: tauri::AppHandle, state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(complete_task_by_id(&app, &state, &id))
}

/// 完成待办：取消计时器、撤下弹窗中该待办的提醒并收起它的便签。托盘菜单也会调用。
pub(crate) fn complete_task_by_id(
    app: &tauri::AppHandle,
    state: &AppState,
    id: &str,
) -> Result<(), AppError> {
    state.db.complete_task(id)?;
    hide_sticky_note_window(app, id);
    state.scheduler.cancel_task(id);
    state
        .scheduler
        .withdraw_notifications(id, ReminderAction::Completed)?;
    state.sync.notify_local_change()
}

/// 把待办的提醒改到 `until`，只改时间、保留标签与优先级。托盘菜单“推迟”使用。
pub(crate) fn reschedule_task_reminder(
    app: &tauri::AppHandle,
    state: &AppState,
    id: &str,
    until: &str,
) -> Result<(), AppError> {
    state.db.set_task_reminder_time(id, Some(until))?;
    state.scheduler.cancel_task(id);
    if let Some(task) = state.db.get_task(id)? {
        state.scheduler.schedule_task(task)?;
    }
    emit_sticky_note_reminder(app, id, Some(until.to_string()));
    state.sync.notify_local_change()
}

#[tauri::command]
pub fn uncomplete_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.uncomplete_task(&id))?;
    if let Some(task) = into_api(state.db.get_task(&id))? {
        if let Some(reminder_time) = &task.reminder_time {
            if scheduler::is_future(reminder_time).unwrap_or(false) {
                into_api(state.scheduler.schedule_task(task))?;
            }
        }
    }
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn delete_task(app: tauri::AppHandle, state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.delete_task(&id))?;
    hide_sticky_note_window(&app, &id);
    state.scheduler.cancel_task(&id);
    into_api(
        state
            .scheduler
            .withdraw_notifications(&id, ReminderAction::Dismissed),
    )?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "action"
)]
pub enum TaskBatchPayload {
    Complete,
    Delete,
    AddTags { tags: Vec<String> },
    SetPriority { priority: i64 },
    SetProject { project: String },
    SetReminder { reminder_time: Option<String> },
}

impl TaskBatchPayload {
    fn into_op(self) -> TaskBatchOp {
        match self {
            TaskBatchPayload::Complete => TaskBatchOp::Complete,
            TaskBatchPayload::Delete => TaskBatchOp::Delete,
            TaskBatchPayload::AddTags { tags } => TaskBatchOp::AddTags(tags),
            TaskBatchPayload::SetPriority { priority } => TaskBatchOp::SetPriority(priority),
            TaskBatchPayload::SetProject { project } => TaskBatchOp::SetProject(project),
            TaskBatchPayload::SetReminder { reminder_time } => {
                TaskBatchOp::SetReminder(normalize_reminder_time(reminder_time))
            }
        }
    }
}

/// 批量操作待办：数据库改动在一个事务中完成，随后逐条处理计时器、弹窗与便签窗口，
/// 只标记一次本地变更。返回实际改动的条数。
#[tauri::command]
pub fn batch_update_tasks(
    app: tauri::AppHandle,
    state: State<AppState>,
    ids: Vec<String>,
    payload: TaskBatchPayload,
) -> ApiResult<usize> {
    let op = payload.into_op();
    let changed = into_api(state.db.batch_update_tasks(&ids, &op))?;
    for id in &changed {
        match &op {
            TaskBatchOp::Complete | TaskBatchOp::Delete => {
                let action = if op == TaskBatchOp::Complete {
                    ReminderAction::Completed
                } else {
                    ReminderAction::Dismissed
                };
                hide_sticky_note_window(&app, id);
                state.scheduler.cancel_task(id);
                into_api(state.scheduler.withdraw_notifications(id, action))?;
            }
            TaskBatchOp::SetReminder(reminder) => {
                state.scheduler.cancel_task(id);
                if let Some(task) = into_api(state.db.get_task(id))? {
                    let future = reminder
                        .as_deref()
                        .is_some_and(|value| scheduler::is_future(value).unwrap_or(false));
                    if future && task.status == TaskStatus::Pending {
                        into_api(state.scheduler.schedule_task(task))?;
                    }
                }
                emit_sticky_note_reminder(&app, id, reminder.clone());
            }
            TaskBatchOp::AddTags(_) | TaskBatchOp::SetPriority(_) | TaskBatchOp::SetProject(_) => {}
        }
    }
    if !changed.is_empty() {
        into_api(state.sync.notify_local_change())?;
    }
    Ok(changed.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(json: &str) -> TaskBatchOp {
        serde_json::from_str::<TaskBatchPayload>(json)
            .unwrap()
            .into_op()
    }

    #[test]
    fn update_payload_distinguishes_missing_and_null_due() {
        let parse = |json: &str| {
            serde_json::from_str::<TaskUpdatePayload>(json)
                .unwrap()
                .due_at
        };
        let base = r#""id":"t","description":"d","stickyContent":null,"reminderTime":null"#;
        assert_eq!(parse(&format!("{{{}}}", base)), None);
        assert_eq!(parse(&format!(r#"{{{},"dueAt":null}}"#, base)), Some(None));
        assert_eq!(
            parse(&format!(r#"{{{},"dueAt":"2026-10-09T18:00"}}"#, base)),
            Some(Some("2026-10-09T18:00".to_string()))
        );
        assert_eq!(
            normalize_due(Some("2026-10-09T18:00".into()))
                .unwrap()
                .as_deref(),
            Some("2026-10-09T18:00:00")
        );
        assert!(normalize_due(Some("明天".into())).is_err());
        assert_eq!(normalize_due(Some("  ".into())).unwrap(), None);
    }

    #[test]
    fn update_payload_keeps_project_when_missing() {
        let parse = |json: &str| {
            serde_json::from_str::<TaskUpdatePayload>(json)
                .unwrap()
                .project
        };
        let base = r#""id":"t","description":"d","stickyContent":null,"reminderTime":null"#;
        assert_eq!(parse(&format!("{{{}}}", base)), None);
        assert_eq!(
            parse(&format!(r#"{{{},"project":""}}"#, base)),
            Some(String::new())
        );
        assert_eq!(
            parse(&format!(r#"{{{},"project":"装修"}}"#, base)),
            Some("装修".to_string())
        );
    }

    #[test]
    fn batch_payload_matches_frontend_json() {
        assert_eq!(op(r#"{"action":"complete"}"#), TaskBatchOp::Complete);
        assert_eq!(op(r#"{"action":"delete"}"#), TaskBatchOp::Delete);
        assert_eq!(
            op(r#"{"action":"addTags","tags":["周报"]}"#),
            TaskBatchOp::AddTags(vec!["周报".to_string()])
        );
        assert_eq!(
            op(r#"{"action":"setPriority","priority":2}"#),
            TaskBatchOp::SetPriority(2)
        );
        assert_eq!(
            op(r#"{"action":"setProject","project":"装修"}"#),
            TaskBatchOp::SetProject("装修".to_string())
        );
        assert_eq!(
            op(r#"{"action":"setReminder","reminderTime":"2026-10-03T15:00:00"}"#),
            TaskBatchOp::SetReminder(Some("2026-10-03T15:00:00".to_string()))
        );
        assert_eq!(
            op(r#"{"action":"setReminder","reminderTime":null}"#),
            TaskBatchOp::SetReminder(None)
        );
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOrderPayload {
    id: String,
    sort_order: f64,
}

/// 拖拽排序：写入一条或多条（重新编号时）待办的手动排序位置。
#[tauri::command]
pub fn set_task_order(state: State<AppState>, orders: Vec<TaskOrderPayload>) -> ApiResult<usize> {
    let orders: Vec<(String, f64)> = orders
        .into_iter()
        .map(|item| (item.id, item.sort_order))
        .collect();
    let changed = into_api(state.db.set_task_sort_orders(&orders))?;
    if changed > 0 {
        into_api(state.sync.notify_local_change())?;
    }
    Ok(changed)
}
