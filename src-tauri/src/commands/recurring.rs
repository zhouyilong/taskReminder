//! 循环提醒相关命令：新建、编辑、暂停/恢复、跳过本次、临时提醒、删除与触发预估。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{into_api, ApiResult};
use crate::kinds::{ReminderAction, RepeatMode, TaskStatus, TaskType};
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
    #[serde(default)]
    tags: Vec<String>,
    /// 结束日期（v2.2，`YYYY-MM-DD`，含当天）。
    #[serde(default)]
    ends_on: Option<String>,
    /// 共提醒几次（v2.2），空为不限。
    #[serde(default)]
    remaining_count: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringPreview {
    task_id: String,
    times: Vec<String>,
    /// 其中的临时提醒（v2.2.4）；被它替代的那一次已从 `times` 中去掉。
    #[serde(skip_serializing_if = "Option::is_none")]
    temp_time: Option<String>,
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
        task_type: TaskType::Recurring,
        status: TaskStatus::Pending,
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
        // 界面提交不认识的模式时由 `sanitize_recurring_task` 报错，不会存库。
        repeat_mode: payload
            .repeat_mode
            .as_deref()
            .map(RepeatMode::parse)
            .unwrap_or(RepeatMode::IntervalRange),
        schedule_time: payload.schedule_time,
        schedule_weekday: payload.schedule_weekday,
        schedule_weekdays: payload.schedule_weekdays,
        schedule_day: payload.schedule_day,
        cron_expression: payload.cron_expression,
        tags: crate::models::normalize_tags(&payload.tags),
        ends_on: payload.ends_on,
        remaining_count: payload.remaining_count,
        temp_trigger: None,
        temp_replaces: None,
    };
    into_api(recurrence::sanitize_recurring_task(&mut draft))?;
    draft.next_trigger = into_api(recurrence::compute_next_trigger(&draft, None))?;
    if recurrence::has_ended(&draft) {
        return Err(if draft.remaining_count == Some(0) {
            "提醒次数需大于 0".to_string()
        } else {
            format!(
                "结束日期早于第一次提醒（{}）",
                draft.next_trigger.replace('T', " ")
            )
        });
    }
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
    // 编辑前已结束、改了结束条件后不再结束的，自动恢复运行；其他情况保留原来的暂停状态。
    let was_ended = into_api(state.db.get_recurring_task(&task.id))?
        .is_some_and(|previous| previous.is_paused && recurrence::has_ended(&previous));
    into_api(recurrence::sanitize_recurring_task(&mut task))?;
    task.next_trigger = into_api(recurrence::compute_next_trigger(&task, None))?;
    if was_ended && !recurrence::has_ended(&task) {
        task.is_paused = false;
    }
    recurrence::apply_end_condition(&mut task);
    into_api(state.db.update_recurring_task(&task))?;
    // 编辑不改临时提醒：按库中的版本计时（界面提交的可能是打开编辑框时的旧值）。
    let task = into_api(state.db.get_recurring_task(&task.id))?.unwrap_or(task);
    into_api(state.scheduler.schedule_recurring(task))?;
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
    if recurrence::has_ended(&task) {
        let reason = recurrence::describe_end(&task).unwrap_or_default();
        return Err(format!("已到结束条件（{}），请先编辑结束条件", reason));
    }
    into_api(state.db.update_recurring_task(&task))?;
    into_api(state.scheduler.schedule_recurring(task))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

/// 跳过本次：把下次触发改为即将到来的这一次之后的那一次，不写提醒记录。
#[tauri::command]
pub fn skip_recurring_occurrence(state: State<AppState>, id: String) -> ApiResult<RecurringTask> {
    let Some(mut task) = into_api(state.db.get_recurring_task(&id))? else {
        return Err("循环提醒不存在".to_string());
    };
    task.next_trigger = into_api(recurrence::skipped_trigger(&task, time::now()))?;
    // 跳过不扣减剩余次数；跳过后超出结束日期时结束。
    recurrence::apply_end_condition(&mut task);
    into_api(state.db.update_recurring_task(&task))?;
    into_api(state.scheduler.schedule_recurring(task.clone()))?;
    into_api(state.sync.notify_local_change())?;
    Ok(task)
}

/// 临时提醒（v2.2.4）：在 `at` 提醒一次（暂停时也提醒）。`replace_next` 为真时同时替代离它最近的
/// 那一次常规提醒（例如每天 9:00 的晨会今天改到 8:45）。`at` 为空即取消临时提醒。
#[tauri::command]
pub fn set_recurring_temp_reminder(
    state: State<AppState>,
    id: String,
    at: Option<String>,
    replace_next: bool,
) -> ApiResult<RecurringTask> {
    let Some(task) = into_api(state.db.get_recurring_task(&id))? else {
        return Err("循环提醒不存在".to_string());
    };
    if !task.repeat_mode.is_known() {
        return Err("该循环提醒来自更新版本，请升级后再设置".to_string());
    }
    let (trigger, replaces) = match at.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        None => (None, None),
        Some(value) => {
            let at = time::parse_datetime_any(value)
                .ok_or_else(|| "临时提醒时间格式无效".to_string())?;
            if at <= time::now() {
                return Err("临时提醒时间需晚于现在".to_string());
            }
            let replaces = if replace_next {
                Some(
                    into_api(recurrence::temp_replace_target(&task, at))?
                        .ok_or_else(|| "没有可以替代的常规提醒".to_string())?,
                )
            } else {
                None
            };
            (Some(time::format_datetime(&at)), replaces)
        }
    };
    into_api(
        state
            .db
            .set_recurring_temp(&id, trigger.as_deref(), replaces.as_deref()),
    )?;
    let Some(task) = into_api(state.db.get_recurring_task(&id))? else {
        return Err("循环提醒不存在".to_string());
    };
    into_api(state.scheduler.schedule_temp(&task))?;
    into_api(state.sync.notify_local_change())?;
    Ok(task)
}

/// 临时提醒在 `at` 时会替代哪一次常规提醒（设置对话框中展示）；没有可替代的返回空。
#[tauri::command]
pub fn preview_recurring_temp_target(
    state: State<AppState>,
    id: String,
    at: String,
) -> ApiResult<Option<String>> {
    let Some(task) = into_api(state.db.get_recurring_task(&id))? else {
        return Ok(None);
    };
    let Some(at) = time::parse_datetime_any(&at) else {
        return Ok(None);
    };
    into_api(recurrence::temp_replace_target(&task, at))
}

#[tauri::command]
pub fn delete_recurring_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.delete_recurring_task(&id))?;
    state.scheduler.cancel_recurring(&id);
    state.scheduler.cancel_temp(&id);
    into_api(
        state
            .scheduler
            .withdraw_notifications(&id, ReminderAction::Dismissed),
    )?;
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
        let (times, temp_time) = recurrence::with_temp_reminder(&task, times, until);
        if !times.is_empty() {
            result.push(RecurringPreview {
                task_id: task.id,
                times,
                temp_time,
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

/// 立即从项目仓库检查节假日数据更新（设置界面“立即检查”），网络请求放在后台线程。
#[tauri::command]
pub async fn check_holiday_updates(
    app: tauri::AppHandle,
) -> ApiResult<crate::holiday_update::HolidayCheckResult> {
    let data_dir = crate::paths::resolve_data_dir(&app).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || crate::holiday_update::check_now(&app, &data_dir))
        .await
        .map_err(|e| e.to_string())?
}
