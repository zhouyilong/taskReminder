#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod db;
mod errors;
mod maintenance;
mod models;
mod paths;
mod scheduler;
mod single_instance;
mod state;
mod sync;
mod tray;

use std::sync::{Arc, Mutex};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State, WindowEvent};

use crate::db::DbManager;
use crate::errors::AppError;
use crate::models::{AppSettings, NotificationPayload, RecurringTask, ReminderRecord, SyncStatus, Task};
use crate::scheduler::ReminderScheduler;
use crate::single_instance::InstanceLock;
use crate::state::AppState;
use crate::sync::CloudSyncService;

type ApiResult<T> = Result<T, String>;

fn into_api<T>(result: Result<T, AppError>) -> ApiResult<T> {
    result.map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskUpdatePayload {
    id: String,
    description: String,
    reminder_time: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRecurringPayload {
    description: String,
    interval_minutes: i64,
    start_time: Option<String>,
    end_time: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AckPayload {
    record_id: String,
    action: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnoozePayload {
    record_id: String,
    reminder_id: String,
    reminder_type: String,
    minutes: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DebugInfo {
    db_path: String,
    db_size: u64,
    db_last_modified: Option<String>,
    active_tasks: usize,
    completed_tasks: usize,
    recurring_tasks: usize,
    reminder_records: usize,
}

#[tauri::command]
fn list_active_tasks(state: State<AppState>) -> ApiResult<Vec<Task>> {
    into_api(state.db.list_active_tasks())
}

#[tauri::command]
fn list_completed_tasks(state: State<AppState>) -> ApiResult<Vec<Task>> {
    into_api(state.db.list_completed_tasks())
}

#[tauri::command]
fn list_recurring_tasks(state: State<AppState>) -> ApiResult<Vec<RecurringTask>> {
    into_api(state.db.list_recurring_tasks())
}

#[tauri::command]
fn list_reminder_records(state: State<AppState>) -> ApiResult<Vec<ReminderRecord>> {
    into_api(state.db.list_reminder_records())
}

#[tauri::command]
fn create_task(state: State<AppState>, description: String) -> ApiResult<Task> {
    let task = into_api(state.db.create_task(description.trim()))?;
    into_api(state.sync.notify_local_change())?;
    Ok(task)
}

#[tauri::command]
fn update_task(state: State<AppState>, task: TaskUpdatePayload) -> ApiResult<()> {
    into_api(state.db.update_task(&task.id, task.description.trim(), task.reminder_time.clone()))?;
    state.scheduler.cancel_task(&task.id);
    if let Some(reminder_time) = task.reminder_time {
        if scheduler::is_future(&reminder_time).unwrap_or(false) {
            if let Some(updated) = into_api(state.db.get_task(&task.id))? {
                into_api(state.scheduler.schedule_task(updated))?;
            }
        }
    }
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn complete_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.complete_task(&id))?;
    state.scheduler.cancel_task(&id);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn uncomplete_task(state: State<AppState>, id: String) -> ApiResult<()> {
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
fn delete_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.delete_task(&id))?;
    state.scheduler.cancel_task(&id);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn create_recurring_task(state: State<AppState>, payload: CreateRecurringPayload) -> ApiResult<RecurringTask> {
    let interval = payload.interval_minutes.max(1);
    let task = into_api(state.db.create_recurring_task(
        payload.description.trim(),
        interval,
        payload.start_time,
        payload.end_time,
    ))?;
    if !task.is_paused {
        into_api(state.scheduler.schedule_recurring(task.clone()))?;
    }
    into_api(state.sync.notify_local_change())?;
    Ok(task)
}

#[tauri::command]
fn update_recurring_task(state: State<AppState>, task: RecurringTask) -> ApiResult<()> {
    into_api(state.db.update_recurring_task(&task))?;
    if task.is_paused {
        state.scheduler.cancel_recurring(&task.id);
    } else {
        into_api(state.scheduler.schedule_recurring(task))?;
    }
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn pause_recurring_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.pause_recurring_task(&id))?;
    state.scheduler.cancel_recurring(&id);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn resume_recurring_task(state: State<AppState>, id: String) -> ApiResult<()> {
    let Some(mut task) = into_api(state.db.get_recurring_task(&id))? else {
        return Ok(());
    };
    task.is_paused = false;
    task.next_trigger = add_minutes(task.interval_minutes);
    into_api(state.db.update_recurring_task(&task))?;
    into_api(state.scheduler.schedule_recurring(task))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn delete_recurring_task(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.delete_recurring_task(&id))?;
    state.scheduler.cancel_recurring(&id);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn delete_reminder_record(state: State<AppState>, id: String) -> ApiResult<()> {
    into_api(state.db.delete_reminder_record(&id))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn delete_reminder_records(state: State<AppState>, ids: Vec<String>) -> ApiResult<()> {
    into_api(state.db.delete_reminder_records(&ids))?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> ApiResult<AppSettings> {
    let mut settings = into_api(state.db.load_settings())?;
    if let Ok(enabled) = autostart::is_autostart_enabled() {
        if settings.auto_start_enabled != enabled {
            settings.auto_start_enabled = enabled;
            let _ = state.db.save_settings(&settings);
        }
    }
    Ok(settings)
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: AppSettings) -> ApiResult<()> {
    into_api(state.db.save_settings(&settings))?;
    into_api(state.sync.update_settings())?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn test_webdav(settings: AppSettings) -> ApiResult<WebDavTestResult> {
    match sync::test_webdav(&settings) {
        Ok((ok, message)) => Ok(WebDavTestResult { ok, message }),
        Err(err) => Ok(WebDavTestResult {
            ok: false,
            message: err.to_string(),
        }),
    }
}

#[derive(serde::Serialize)]
struct WebDavTestResult {
    ok: bool,
    message: String,
}

#[tauri::command]
fn sync_now(state: State<AppState>, reason: String) -> ApiResult<()> {
    into_api(state.sync.request_sync(&reason))
}

#[tauri::command]
fn set_autostart(state: State<AppState>, enabled: bool) -> ApiResult<()> {
    if enabled {
        into_api(autostart::enable_autostart())?;
    } else {
        into_api(autostart::disable_autostart())?;
    }
    let mut settings = into_api(state.db.load_settings())?;
    settings.auto_start_enabled = enabled;
    into_api(state.db.save_settings(&settings))?;
    Ok(())
}

#[tauri::command]
fn ack_notification(state: State<AppState>, payload: AckPayload) -> ApiResult<()> {
    if let Some(record) = into_api(state.db.get_reminder_record(&payload.record_id))? {
        into_api(state.db.update_reminder_record_action(&payload.record_id, &payload.action))?;
        if payload.action == "COMPLETED" && record.reminder_type == "TASK" {
            into_api(state.db.complete_task(&record.reminder_id))?;
            state.scheduler.cancel_task(&record.reminder_id);
        }
        into_api(state.sync.notify_local_change())?;
    }
    *state.notification_snapshot.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
fn snooze_notification(state: State<AppState>, payload: SnoozePayload) -> ApiResult<()> {
    let minutes = payload.minutes.max(1);
    into_api(state.db.update_reminder_record_action(&payload.record_id, "SNOOZED"))?;
    match payload.reminder_type.as_str() {
        "TASK" => {
            if let Some(mut task) = into_api(state.db.get_task(&payload.reminder_id))? {
                let reminder_time = add_minutes(minutes);
                into_api(state.db.update_task(&task.id, &task.description, Some(reminder_time.clone())))?;
                task.reminder_time = Some(reminder_time);
                state.scheduler.cancel_task(&task.id);
                into_api(state.scheduler.schedule_task(task))?;
            }
        }
        "RECURRING" => {
            if let Some(mut task) = into_api(state.db.get_recurring_task(&payload.reminder_id))? {
                task.next_trigger = add_minutes(minutes);
                task.is_paused = false;
                into_api(state.db.update_recurring_task(&task))?;
                into_api(state.scheduler.schedule_recurring(task))?;
            }
        }
        _ => {}
    }
    into_api(state.sync.notify_local_change())?;
    *state.notification_snapshot.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
fn get_sync_status(state: State<AppState>) -> ApiResult<SyncStatus> {
    into_api(state.sync.get_status())
}

#[tauri::command]
fn get_notification_snapshot(state: State<AppState>) -> ApiResult<Option<NotificationPayload>> {
    Ok(state.notification_snapshot.lock().unwrap().clone())
}

#[cfg(target_os = "windows")]
fn read_winrt_theme() -> Option<String> {
    use windows::UI::ViewManagement::{UISettings, UIColorType};
    let settings = UISettings::new().ok()?;
    let color = settings.GetColorValue(UIColorType::Background).ok()?;
    let luminance = (0.299 * color.R as f32 + 0.587 * color.G as f32 + 0.114 * color.B as f32) / 255.0;
    Some(if luminance < 0.5 { "dark" } else { "light" }.to_string())
}

#[cfg(target_os = "windows")]
fn read_syscolor_theme() -> Option<String> {
    use windows::Win32::Graphics::Gdi::{GetSysColor, COLOR_WINDOW};
    let color = unsafe { GetSysColor(COLOR_WINDOW) } as u32;
    let r = (color & 0x0000_00FF) as f32;
    let g = ((color & 0x0000_FF00) >> 8) as f32;
    let b = ((color & 0x00FF_0000) >> 16) as f32;
    let luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
    Some(if luminance < 0.5 { "dark" } else { "light" }.to_string())
}

#[tauri::command]
fn is_dev_mode() -> bool {
    paths::is_dev_mode()
}

#[tauri::command]
fn get_current_theme(window: tauri::Window) -> ApiResult<String> {
    #[cfg(target_os = "windows")]
    {
        if let Some(theme) = read_winrt_theme() {
            return Ok(theme);
        }

        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        {
            if let Ok(value) = key.get_value::<u32, _>("SystemUsesLightTheme") {
                return Ok(if value == 0 { "dark" } else { "light" }.to_string());
            }
            if let Ok(value) = key.get_value::<u32, _>("AppsUseLightTheme") {
                return Ok(if value == 0 { "dark" } else { "light" }.to_string());
            }
        }

        if let Some(theme) = read_syscolor_theme() {
            return Ok(theme);
        }
    }

    let theme = window.theme().map_err(|e| e.to_string())?;
    Ok(theme.to_string())
}

#[tauri::command]
fn get_debug_info(state: State<AppState>) -> ApiResult<DebugInfo> {
    let db_path = state.db.db_path();
    let metadata = std::fs::metadata(&db_path).ok();
    let db_size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
    let db_last_modified = metadata
        .and_then(|m| m.modified().ok())
        .map(|time| chrono::DateTime::<Local>::from(time).format("%Y-%m-%d %H:%M:%S").to_string());

    let active_tasks = into_api(state.db.list_active_tasks())?.len();
    let completed_tasks = into_api(state.db.list_completed_tasks())?.len();
    let recurring_tasks = into_api(state.db.list_recurring_tasks())?.len();
    let reminder_records = into_api(state.db.list_reminder_records())?.len();

    Ok(DebugInfo {
        db_path: db_path.to_string_lossy().to_string(),
        db_size,
        db_last_modified,
        active_tasks,
        completed_tasks,
        recurring_tasks,
        reminder_records,
    })
}

fn add_minutes(minutes: i64) -> String {
    let dt = Local::now().naive_local() + chrono::Duration::minutes(minutes);
    dt.format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn main() {
    tauri::Builder::default()
        .system_tray(tray::build_tray())
        .on_system_tray_event(tray::handle_event)
        .on_window_event(|event| {
            if event.window().label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event.event() {
                    api.prevent_close();
                    let _ = event.window().hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_active_tasks,
            list_completed_tasks,
            list_recurring_tasks,
            list_reminder_records,
            create_task,
            update_task,
            complete_task,
            uncomplete_task,
            delete_task,
            create_recurring_task,
            update_recurring_task,
            pause_recurring_task,
            resume_recurring_task,
            delete_recurring_task,
            delete_reminder_record,
            delete_reminder_records,
            get_settings,
            save_settings,
            test_webdav,
            sync_now,
            set_autostart,
            ack_notification,
            snooze_notification,
            get_sync_status,
            get_notification_snapshot,
            get_current_theme,
            get_debug_info,
            is_dev_mode
        ])
        .setup(|app| {
            let result: Result<(), AppError> = (|| {
                let app_handle = app.handle();
                let data_dir = paths::resolve_data_dir(&app_handle)?;
                let lock_path = paths::lock_path(&data_dir);
                let dev_mode = paths::is_dev_mode();
                let dialog_title = if dev_mode { "任务提醒 [开发]" } else { "任务提醒" };
                match InstanceLock::try_lock(&lock_path)? {
                    Some(lock) => {
                        app.manage(lock);
                    }
                    None => {
                        tauri::api::dialog::message::<tauri::Wry>(None, dialog_title, "应用已经在运行");
                        app_handle.exit(0);
                        return Ok(());
                    }
                }

                // 开发模式：窗口标题加 [开发] 标识
                if dev_mode {
                    if let Some(window) = app.get_window("main") {
                        let _ = window.set_title("任务提醒应用 [开发]");
                    }
                }

                let db = DbManager::new(paths::db_path(&data_dir))?;
                let snapshot = Arc::new(Mutex::new(None));
                let sync = CloudSyncService::new(app_handle.clone(), db.clone());
                let scheduler = ReminderScheduler::new(app_handle.clone(), db.clone(), sync.clone(), snapshot.clone());
                scheduler.schedule_existing()?;
                sync.start()?;
                maintenance::start_maintenance(db.clone());

                let settings = db.load_settings()?;
                if settings.auto_start_enabled {
                    let _ = autostart::enable_autostart();
                }

                let state = AppState {
                    db,
                    scheduler,
                    sync,
                    notification_snapshot: snapshot,
                    app_handle,
                };
                app.manage(state);
                Ok(())
            })();
            result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}
