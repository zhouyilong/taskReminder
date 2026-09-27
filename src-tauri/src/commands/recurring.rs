//! 循环提醒相关命令：新建、编辑、暂停/恢复、删除与触发预估。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{into_api, ApiResult};
use crate::models::RecurringTask;
use crate::state::AppState;
use crate::{holidays, recurrence, time};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRecurringPayload {
    description: String,
    interval_minutes: i64,
    start_time: Option<String>,
    end_time: Option<String>,
    repeat_mode: Option<String>,
    schedule_time: Option<String>,
    schedule_weekday: Option<i64>,
    #[serde(default)]
    schedule_weekdays: Option<i64>,
    schedule_day: Option<i64>,
    cron_expression: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringPreview {
    task_id: String,
    times: Vec<String>,
}

#[tauri::command]
pub fn list_recurring_tasks(state: State<AppState>) -> ApiResult<Vec<RecurringTask>> {
    into_api(state.db.list_recurring_tasks())
}

#[tauri::command]
pub fn create_recurring_task(
    state: State<AppState>,
    payload: CreateRecurringPayload,
) -> ApiResult<RecurringTask> {
    let mut draft = RecurringTask {
        id: String::new(),
        description: payload.description.trim().to_string(),
        task_type: "RECURRING".to_string(),
        status: "PENDING".to_string(),
        created_at: String::new(),
        completed_at: None,
        reminder_time: None,
        updated_at: None,
        deleted_at: None,
        interval_minutes: payload.interval_minutes.max(1),
        last_triggered: None,
        next_trigger: String::new(),
        is_paused: false,
        start_time: payload.start_time,
        end_time: payload.end_time,
        repeat_mode: payload
            .repeat_mode
            .unwrap_or_else(|| recurrence::REPEAT_MODE_INTERVAL_RANGE.to_string()),
        schedule_time: payload.schedule_time,
        schedule_weekday: payload.schedule_weekday,
        schedule_weekdays: payload.schedule_weekdays,
        schedule_day: payload.schedule_day,
        cron_expression: payload.cron_expression,
    };
    into_api(recurrence::sanitize_recurring_task(&mut draft))?;
    draft.next_trigger = into_api(recurrence::compute_next_trigger(&draft, None))?;
    let task = into_api(state.db.create_recurring_task(&draft))?;
    if !task.is_paused {
        into_api(state.scheduler.schedule_recurring(task.clone()))?;
    }
    into_api(state.sync.notify_local_change())?;
    Ok(task)
}

#[tauri::command]
pub fn update_recurring_task(state: State<AppState>, task: RecurringTask) -> ApiResult<()> {
    let mut task = task;
    into_api(recurrence::sanitize_recurring_task(&mut task))?;
    task.next_trigger = into_api(recurrence::compute_next_trigger(&task, None))?;
    into_api(state.db.update_recurring_task(&task))?;
    if task.is_paused {
        state.scheduler.cancel_recurring(&task.id);
    } else {
        into_api(state.scheduler.schedule_recurring(task.clone()))?;
    }
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn pause_recurring_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.pause_recurring_task(&id))?;
    state.scheduler.cancel_recurring(&id);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn resume_recurring_task(state: State<AppState>, id: String) -> ApiResult<()> {
    let Some(mut task) = into_api(state.db.get_recurring_task(&id))? else {
        return Ok(());
    };
    task.is_paused = false;
    into_api(recurrence::sanitize_recurring_task(&mut task))?;
    task.next_trigger = into_api(recurrence::compute_next_trigger(&task, None))?;
    into_api(state.db.update_recurring_task(&task))?;
    into_api(state.scheduler.schedule_recurring(task))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn delete_recurring_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.delete_recurring_task(&id))?;
    state.scheduler.cancel_recurring(&id);
    into_api(state.scheduler.withdraw_notifications(&id, "DISMISSED"))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

/// 预估每个循环提醒在 `until`（含）之前的触发时间，每个任务最多 `limit` 个。
#[tauri::command]
pub fn preview_recurring_triggers(
    state: State<AppState>,
    until: String,
    limit: Option<usize>,
) -> ApiResult<Vec<RecurringPreview>> {
    let until = time::parse_datetime_any(&until).ok_or_else(|| "截止时间格式无效".to_string())?;
    let limit = limit.unwrap_or(48).min(500);
    let mut result = Vec::new();
    for task in into_api(state.db.list_recurring_tasks())? {
        // 单个任务规则异常不影响其他任务的预览。
        let times = recurrence::upcoming_triggers(&task, until, limit).unwrap_or_default();
        if !times.is_empty() {
            result.push(RecurringPreview {
                task_id: task.id,
                times,
            });
        }
    }
    Ok(result)
}

/// 已有法定节假日数据的年份，供设置界面提示覆盖范围。
#[tauri::command]
pub fn get_holiday_years() -> Vec<i32> {
    holidays::covered_years()
}
