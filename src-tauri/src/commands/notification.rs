//! 提醒弹窗与提醒记录：知道了、完成、稍后提醒、队列读取与记录删除。

use chrono::Local;
use serde::Deserialize;
use tauri::{Emitter, Manager, State};

use crate::commands::{into_api, ApiResult};
use crate::models::{NotificationPayload, ReminderRecord};
use crate::state::AppState;
use crate::windows::sticky::{emit_sticky_note_reminder, sticky_note_item_label};
use crate::{scheduler, time};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AckPayload {
    record_id: String,
    action: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnoozePayload {
    record_id: String,
    reminder_id: String,
    reminder_type: String,
    minutes: i64,
    /// 推迟到指定时间（如“明早 9 点”），优先于 `minutes`。
    #[serde(default)]
    until: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteNotificationPayload {
    record_id: String,
    reminder_id: String,
}

#[tauri::command]
pub fn list_reminder_records(state: State<AppState>) -> ApiResult<Vec<ReminderRecord>> {
    into_api(state.db.list_reminder_records())
}

#[tauri::command]
pub fn delete_reminder_record(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.delete_reminder_record(&id))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn delete_reminder_records(state: State<AppState>, ids: Vec<String>) -> ApiResult<()> {
    into_api(state.db.delete_reminder_records(&ids))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

/// 处理完弹窗中的一条提醒后，从队列移除并广播剩余队列；返回剩余队列供弹窗直接使用。
fn finish_notification(
    app: &tauri::AppHandle,
    state: &AppState,
    record_id: &str,
) -> Vec<NotificationPayload> {
    let remaining = state.scheduler.queue().remove_record(record_id);
    scheduler::publish_queue(app, &remaining);
    remaining
}

#[tauri::command]
pub fn ack_notification(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: AckPayload,
) -> ApiResult<Vec<NotificationPayload>> {
    if into_api(state.db.get_reminder_record(&payload.record_id))?.is_some() {
        into_api(
            state
                .db
                .update_reminder_record_action(&payload.record_id, &payload.action),
        )?;
        into_api(state.sync.notify_local_change())?;
    }
    Ok(finish_notification(&app, &state, &payload.record_id))
}

/// 一次性关闭队列中的全部提醒（均记为“已关闭”）。
#[tauri::command]
pub fn ack_all_notifications(app: tauri::AppHandle, state: State<AppState>) -> ApiResult<()> {
    let drained = state.scheduler.queue().drain();
    for item in &drained {
        into_api(
            state
                .db
                .update_reminder_record_action(&item.record_id, "DISMISSED"),
        )?;
    }
    if !drained.is_empty() {
        into_api(state.sync.notify_local_change())?;
    }
    scheduler::publish_queue(&app, &[]);
    Ok(())
}

/// 在提醒弹窗中直接完成一次性待办：记录标记为“已完成”，任务完成并关闭其便签。
#[tauri::command]
pub fn complete_notification(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: CompleteNotificationPayload,
) -> ApiResult<Vec<NotificationPayload>> {
    into_api(
        state
            .db
            .update_reminder_record_action(&payload.record_id, "COMPLETED"),
    )?;
    if let Some(task) = into_api(state.db.get_task(&payload.reminder_id))? {
        if task.status != "COMPLETED" && task.deleted_at.is_none() {
            into_api(state.db.complete_task(&task.id))?;
        }
        state.scheduler.cancel_task(&task.id);
        if let Some(window) = app.get_webview_window(&sticky_note_item_label(&task.id)) {
            let _ = window.hide();
            into_api(state.db.close_sticky_note(&task.id))?;
            let _ = app.emit("sticky-note-changed", task.id.clone());
        }
    }
    // 同一任务可能还有其他排队中的提醒，一并撤下。
    let _ = state
        .scheduler
        .queue()
        .remove_reminder(&payload.reminder_id);
    into_api(state.sync.notify_local_change())?;
    let remaining = finish_notification(&app, &state, &payload.record_id);
    let _ = app.emit("data-updated", ());
    Ok(remaining)
}

#[tauri::command]
pub fn snooze_notification(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: SnoozePayload,
) -> ApiResult<Vec<NotificationPayload>> {
    let minutes = payload.minutes.max(1);
    let snooze_until = payload
        .until
        .as_deref()
        .and_then(time::parse_datetime_any)
        .filter(|value| *value > Local::now().naive_local())
        .map(|value| time::format_datetime(&value))
        .unwrap_or_else(|| add_minutes(minutes));
    into_api(
        state
            .db
            .update_reminder_record_action(&payload.record_id, "SNOOZED"),
    )?;
    match payload.reminder_type.as_str() {
        "TASK" => {
            if let Some(mut task) = into_api(state.db.get_task(&payload.reminder_id))? {
                let reminder_time = snooze_until.clone();
                into_api(state.db.update_task(
                    &task.id,
                    &task.description,
                    task.sticky_content.clone(),
                    Some(reminder_time.clone()),
                    None,
                ))?;
                task.reminder_time = Some(reminder_time);
                state.scheduler.cancel_task(&task.id);
                emit_sticky_note_reminder(&app, &task.id, task.reminder_time.clone());
                into_api(state.scheduler.schedule_task(task))?;
            }
        }
        "RECURRING" => {
            if let Some(mut task) = into_api(state.db.get_recurring_task(&payload.reminder_id))? {
                task.next_trigger = snooze_until.clone();
                task.is_paused = false;
                into_api(state.db.update_recurring_task(&task))?;
                into_api(state.scheduler.schedule_recurring(task))?;
            }
        }
        _ => {}
    }
    into_api(state.sync.notify_local_change())?;
    Ok(finish_notification(&app, &state, &payload.record_id))
}

#[tauri::command]
pub fn get_notification_queue(state: State<AppState>) -> ApiResult<Vec<NotificationPayload>> {
    Ok(state.scheduler.queue().snapshot())
}

fn add_minutes(minutes: i64) -> String {
    let dt = Local::now().naive_local() + chrono::Duration::minutes(minutes);
    dt.format("%Y-%m-%dT%H:%M:%S").to_string()
}
