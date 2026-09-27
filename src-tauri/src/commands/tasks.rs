//! 待办相关命令：列表、新建（含快速添加）、编辑、完成与删除。

use serde::Deserialize;
use tauri::{Emitter, State};

use crate::commands::{into_api, ApiResult};
use crate::db::TaskMeta;
use crate::errors::AppError;
use crate::models::Task;
use crate::scheduler;
use crate::state::AppState;
use crate::windows::sticky::emit_sticky_note_reminder;

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
}

impl TaskUpdatePayload {
    fn meta(&self) -> Option<TaskMeta> {
        if self.tags.is_none() && self.priority.is_none() {
            return None;
        }
        Some(TaskMeta {
            tags: self.tags.clone().unwrap_or_default(),
            priority: self.priority.unwrap_or(0),
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
    };
    create_task_with_reminder(
        &state,
        payload.description.trim(),
        payload.sticky_content.as_deref(),
        payload.reminder_time,
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
    };
    let task = create_task_with_reminder(
        &state,
        payload.description.trim(),
        None,
        payload.reminder_time,
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
    meta: &TaskMeta,
) -> ApiResult<Task> {
    if description.is_empty() {
        return Err("待办内容不能为空".to_string());
    }
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
    into_api(state.db.update_task(
        &task.id,
        task.description.trim(),
        task.sticky_content.clone(),
        reminder_time.clone(),
        meta.as_ref(),
    ))?;
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
pub fn complete_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(complete_task_by_id(&state, &id))
}

/// 完成待办：取消计时器并撤下弹窗中该待办的提醒。托盘菜单也会调用。
pub(crate) fn complete_task_by_id(state: &AppState, id: &str) -> Result<(), AppError> {
    state.db.complete_task(id)?;
    state.scheduler.cancel_task(id);
    state.scheduler.withdraw_notifications(id, "COMPLETED")?;
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
pub fn delete_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.delete_task(&id))?;
    state.scheduler.cancel_task(&id);
    into_api(state.scheduler.withdraw_notifications(&id, "DISMISSED"))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}
