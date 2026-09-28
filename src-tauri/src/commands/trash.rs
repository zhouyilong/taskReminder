//! 回收站：列出、恢复与永久删除墓碑。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{into_api, ApiResult};
use crate::db;
use crate::errors::AppError;
use crate::models::{RecurringTask, Task};
use crate::recurrence;
use crate::scheduler;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashPayload {
    tasks: Vec<Task>,
    recurring_tasks: Vec<RecurringTask>,
    /// 墓碑保留天数：超过后自动永久删除。
    retention_days: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeTrashPayload {
    #[serde(default)]
    task_ids: Vec<String>,
    #[serde(default)]
    recurring_ids: Vec<String>,
}

fn current_retention_days(state: &AppState) -> Result<i64, AppError> {
    let settings = state.db.load_settings()?;
    Ok(db::tombstone_retention_days(settings.webdav_enabled))
}

#[tauri::command]
pub fn list_trash(state: State<AppState>) -> ApiResult<TrashPayload> {
    let retention_days = into_api(current_retention_days(&state))?;
    Ok(TrashPayload {
        tasks: into_api(state.db.list_deleted_tasks(retention_days))?,
        recurring_tasks: into_api(state.db.list_deleted_recurring_tasks(retention_days))?,
        retention_days,
    })
}

#[tauri::command]
pub fn restore_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.restore_task(&id))?;
    if let Some(task) = into_api(state.db.get_task(&id))? {
        if task.status != "COMPLETED" {
            if let Some(reminder_time) = &task.reminder_time {
                if scheduler::is_future(reminder_time).unwrap_or(false) {
                    into_api(state.scheduler.schedule_task(task))?;
                }
            }
        }
    }
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn restore_recurring_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.restore_recurring_task(&id))?;
    let Some(mut task) = into_api(state.db.get_recurring_task(&id))? else {
        return Ok(());
    };
    // 删除期间错过的触发不补发，从现在起重新计算下次触发时间。
    into_api(recurrence::sanitize_recurring_task(&mut task))?;
    task.next_trigger = into_api(recurrence::compute_next_trigger(&task, None))?;
    into_api(state.db.update_recurring_task(&task))?;
    if !task.is_paused {
        into_api(state.scheduler.schedule_recurring(task))?;
    }
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

/// 永久删除回收站中的条目。未开启同步时立即物理删除；
/// 开启同步时由下一次同步在本地与远端一起清理，避免远端墓碑合并回来。
#[tauri::command]
pub fn purge_trash(state: State<AppState>, payload: PurgeTrashPayload) -> ApiResult<()> {
    into_api(
        state
            .db
            .expire_tombstones(db::TrashTable::Tasks, &payload.task_ids),
    )?;
    into_api(
        state
            .db
            .expire_tombstones(db::TrashTable::RecurringTasks, &payload.recurring_ids),
    )?;
    let settings = into_api(state.db.load_settings())?;
    if !settings.webdav_enabled {
        into_api(
            state
                .db
                .purge_expired_tombstones(db::tombstone_retention_days(false)),
        )?;
    }
    into_api(state.sync.notify_local_change())?;
    Ok(())
}
