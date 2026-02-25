#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod db;
mod errors;
mod maintenance;
mod models;
mod paths;
mod recurrence;
mod scheduler;
mod single_instance;
mod state;
mod sync;
mod tray;

use std::sync::{Arc, Mutex};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{
    Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};

use crate::db::DbManager;
use crate::errors::AppError;
use crate::models::{
    AppSettings, NotificationPayload, RecurringTask, ReminderRecord, StickyNote, SyncStatus, Task,
};
use crate::scheduler::ReminderScheduler;
use crate::single_instance::InstanceLock;
use crate::state::AppState;
use crate::sync::CloudSyncService;

type ApiResult<T> = Result<T, String>;
const STICKY_NOTE_LABEL: &str = "sticky-note";
const STICKY_NOTE_ITEM_PREFIX: &str = "sticky-note-item-";
const STICKY_NOTE_MIN_WIDTH: i64 = 280;
const STICKY_NOTE_MIN_HEIGHT: i64 = 320;
const STICKY_NOTE_ITEM_WIDTH: f64 = 284.0;
const STICKY_NOTE_ITEM_HEIGHT: f64 = 280.0;
const STICKY_NOTE_MIN_OPACITY: f64 = 0.35;
const STICKY_NOTE_MAX_OPACITY: f64 = 1.0;
const STICKY_NOTE_DEFAULT_OPACITY: f64 = 0.95;

fn into_api<T>(result: Result<T, AppError>) -> ApiResult<T> {
    result.map_err(|e| e.to_string())
}

fn encode_note_id(note_id: &str) -> String {
    note_id
        .as_bytes()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

fn decode_note_id(encoded: &str) -> Option<String> {
    if encoded.len() % 2 != 0 {
        return None;
    }
    let bytes = (0..encoded.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

fn sticky_note_item_label(note_id: &str) -> String {
    format!("{}{}", STICKY_NOTE_ITEM_PREFIX, encode_note_id(note_id))
}

fn sticky_note_item_refresh_event(note_id: &str) -> String {
    format!("sticky-note-item-refresh-{}", encode_note_id(note_id))
}

fn note_id_from_item_label(label: &str) -> Option<String> {
    label
        .strip_prefix(STICKY_NOTE_ITEM_PREFIX)
        .and_then(decode_note_id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskUpdatePayload {
    id: String,
    description: String,
    sticky_content: Option<String>,
    reminder_time: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskPayload {
    description: String,
    sticky_content: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRecurringPayload {
    description: String,
    interval_minutes: i64,
    start_time: Option<String>,
    end_time: Option<String>,
    repeat_mode: Option<String>,
    schedule_time: Option<String>,
    schedule_weekday: Option<i64>,
    schedule_day: Option<i64>,
    cron_expression: Option<String>,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenStickyNotePayload {
    task_id: String,
    title: Option<String>,
    default_x: Option<f64>,
    default_y: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveStickyNoteContentPayload {
    task_id: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStickyNoteTitlePayload {
    task_id: String,
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveStickyNotePayload {
    task_id: String,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateStickyNotePayload {
    title: Option<String>,
    default_x: Option<f64>,
    default_y: Option<f64>,
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
fn create_task(state: State<AppState>, payload: CreateTaskPayload) -> ApiResult<Task> {
    let task = into_api(state.db.create_task(
        payload.description.trim(),
        payload.sticky_content.as_deref(),
    ))?;
    into_api(state.sync.notify_local_change())?;
    Ok(task)
}

#[tauri::command]
fn update_task(state: State<AppState>, task: TaskUpdatePayload) -> ApiResult<()> {
    into_api(state.db.update_task(
        &task.id,
        task.description.trim(),
        task.sticky_content.clone(),
        task.reminder_time.clone(),
    ))?;
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
fn create_recurring_task(
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
fn update_recurring_task(state: State<AppState>, task: RecurringTask) -> ApiResult<()> {
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
    into_api(recurrence::sanitize_recurring_task(&mut task))?;
    task.next_trigger = into_api(recurrence::compute_next_trigger(&task, None))?;
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
fn save_settings(
    app: tauri::AppHandle,
    state: State<AppState>,
    settings: AppSettings,
) -> ApiResult<()> {
    into_api(state.db.save_settings(&settings))?;
    into_api(sync_sticky_note_window(&app, &settings))?;
    into_api(state.sync.update_settings())?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn list_sticky_notes(state: State<AppState>) -> ApiResult<Vec<StickyNote>> {
    into_api(state.db.list_sticky_notes())
}

#[tauri::command]
fn get_sticky_note_by_window_label(
    state: State<AppState>,
    label: String,
) -> ApiResult<Option<StickyNote>> {
    let Some(note_id) = note_id_from_item_label(&label) else {
        return Ok(None);
    };
    into_api(state.db.get_sticky_note(&note_id))
}

#[tauri::command]
fn open_sticky_note(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: OpenStickyNotePayload,
) -> ApiResult<StickyNote> {
    let note = into_api(state.db.open_sticky_note(
        &payload.task_id,
        payload.title,
        payload.default_x,
        payload.default_y,
    ))?;
    {
        let app_for_show = app.clone();
        let note_for_show = note.clone();
        std::thread::spawn(move || {
            let app_in_main = app_for_show.clone();
            let note_in_main = note_for_show.clone();
            if let Err(err) = app_for_show.run_on_main_thread(move || {
                if let Err(show_err) = show_sticky_note_item_window(&app_in_main, &note_in_main) {
                    eprintln!("[sticky-note] 打开便签项窗口失败: {}", show_err);
                }
            }) {
                eprintln!("[sticky-note] 主线程调度打开便签项窗口失败: {}", err);
            }
        });
    }
    let _ = app.emit("sticky-note-changed", note.task_id.clone());
    let sync = state.sync.clone();
    tauri::async_runtime::spawn(async move {
        let _ = sync.notify_local_change();
    });
    Ok(note)
}

#[tauri::command]
fn create_sticky_note(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: CreateStickyNotePayload,
) -> ApiResult<StickyNote> {
    let note = into_api(state.db.create_custom_sticky_note(
        payload.title.as_deref().unwrap_or(""),
        payload.default_x,
        payload.default_y,
    ))?;
    {
        let app_for_show = app.clone();
        let note_for_show = note.clone();
        std::thread::spawn(move || {
            let app_in_main = app_for_show.clone();
            let note_in_main = note_for_show.clone();
            if let Err(err) = app_for_show.run_on_main_thread(move || {
                if let Err(show_err) = show_sticky_note_item_window(&app_in_main, &note_in_main) {
                    eprintln!("[sticky-note] 新增便签窗口显示失败: {}", show_err);
                }
            }) {
                eprintln!("[sticky-note] 主线程调度新增便签窗口失败: {}", err);
            }
        });
    }
    let _ = app.emit("sticky-note-changed", note.task_id.clone());
    let sync = state.sync.clone();
    tauri::async_runtime::spawn(async move {
        let _ = sync.notify_local_change();
    });
    Ok(note)
}

#[tauri::command]
fn save_sticky_note_content(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: SaveStickyNoteContentPayload,
) -> ApiResult<()> {
    into_api(
        state
            .db
            .save_sticky_note_content(&payload.task_id, &payload.content),
    )?;
    let _ = app.emit("sticky-note-changed", payload.task_id.clone());
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn update_sticky_note_title(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: UpdateStickyNoteTitlePayload,
) -> ApiResult<()> {
    into_api(
        state
            .db
            .update_sticky_note_title(&payload.task_id, &payload.title),
    )?;
    let _ = app.emit("sticky-note-changed", payload.task_id.clone());
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn move_sticky_note(state: State<AppState>, payload: MoveStickyNotePayload) -> ApiResult<()> {
    into_api(
        state
            .db
            .move_sticky_note(&payload.task_id, payload.x, payload.y),
    )?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn close_sticky_note(
    app: tauri::AppHandle,
    state: State<AppState>,
    task_id: String,
) -> ApiResult<()> {
    if let Some(window) = app.get_webview_window(&sticky_note_item_label(&task_id)) {
        let _ = window.hide();
    }
    into_api(state.db.close_sticky_note(&task_id))?;
    let _ = app.emit("sticky-note-changed", task_id);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn close_sticky_note_by_window_label(
    app: tauri::AppHandle,
    state: State<AppState>,
    label: String,
) -> ApiResult<()> {
    let Some(note_id) = note_id_from_item_label(&label) else {
        return Ok(());
    };
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.hide();
    }
    into_api(state.db.close_sticky_note(&note_id))?;
    let _ = app.emit("sticky-note-changed", note_id);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
fn set_sticky_note_opacity(
    app: tauri::AppHandle,
    state: State<AppState>,
    opacity: f64,
) -> ApiResult<f64> {
    let normalized = into_api(state.db.update_sticky_note_opacity(opacity))?;
    if let Ok(settings) = state.db.load_settings() {
        let _ = app.emit("sticky-note-settings-updated", settings);
    }
    into_api(state.sync.notify_local_change())?;
    Ok(normalized)
}

#[tauri::command]
fn is_sticky_note_window_visible(app: tauri::AppHandle) -> ApiResult<bool> {
    if let Some(window) = app.get_webview_window(STICKY_NOTE_LABEL) {
        return window.is_visible().map_err(|err| err.to_string());
    }
    Ok(false)
}

#[tauri::command]
fn set_sticky_note_window_visible(
    app: tauri::AppHandle,
    state: State<AppState>,
    visible: bool,
) -> ApiResult<bool> {
    if visible {
        let app_for_show = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = show_sticky_note_window_fallback(&app_for_show) {
                eprintln!("[sticky-note] 开窗失败: {}", err);
                let _ = app_for_show.emit("sticky-note-visibility", false);
            }
        });
    } else {
        if let Some(window) = app.get_webview_window(STICKY_NOTE_LABEL) {
            let _ = window.hide();
        }
        let _ = app.emit("sticky-note-visibility", false);
    }

    let db = state.db.clone();
    std::thread::spawn(move || {
        let _ = db.update_sticky_note_enabled(visible);
    });

    let _ = app.emit("sticky-note-changed", "all");
    Ok(visible)
}

#[tauri::command]
fn force_show_sticky_note_window(app: tauri::AppHandle) -> ApiResult<bool> {
    show_sticky_note_window_fallback(&app).map_err(|e| e.to_string())?;
    Ok(true)
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
    if into_api(state.db.get_reminder_record(&payload.record_id))?.is_some() {
        into_api(
            state
                .db
                .update_reminder_record_action(&payload.record_id, &payload.action),
        )?;
        into_api(state.sync.notify_local_change())?;
    }
    *state.notification_snapshot.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
fn snooze_notification(state: State<AppState>, payload: SnoozePayload) -> ApiResult<()> {
    let minutes = payload.minutes.max(1);
    into_api(
        state
            .db
            .update_reminder_record_action(&payload.record_id, "SNOOZED"),
    )?;
    match payload.reminder_type.as_str() {
        "TASK" => {
            if let Some(mut task) = into_api(state.db.get_task(&payload.reminder_id))? {
                let reminder_time = add_minutes(minutes);
                into_api(state.db.update_task(
                    &task.id,
                    &task.description,
                    task.sticky_content.clone(),
                    Some(reminder_time.clone()),
                ))?;
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
    use windows::UI::ViewManagement::{UIColorType, UISettings};
    let settings = UISettings::new().ok()?;
    let color = settings.GetColorValue(UIColorType::Background).ok()?;
    let luminance =
        (0.299 * color.R as f32 + 0.587 * color.G as f32 + 0.114 * color.B as f32) / 255.0;
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
    let db_last_modified = metadata.and_then(|m| m.modified().ok()).map(|time| {
        chrono::DateTime::<Local>::from(time)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    });

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

fn normalize_sticky_note_size(width: i64, height: i64) -> (f64, f64) {
    (
        width.max(STICKY_NOTE_MIN_WIDTH) as f64,
        height.max(STICKY_NOTE_MIN_HEIGHT) as f64,
    )
}

fn normalize_sticky_note_opacity(value: f64) -> f64 {
    if !value.is_finite() {
        return STICKY_NOTE_DEFAULT_OPACITY;
    }
    value.clamp(STICKY_NOTE_MIN_OPACITY, STICKY_NOTE_MAX_OPACITY)
}

fn enforce_sticky_item_layer(window: &tauri::WebviewWindow) {
    let _ = window.set_always_on_top(false);
    let _ = window.set_always_on_bottom(true);
    let _ = window.set_skip_taskbar(true);
}

fn reorder_sticky_item_layer(window: &tauri::WebviewWindow) {
    // Re-apply bottom-most flags to force z-order refresh within sticky-note windows.
    let _ = window.set_always_on_top(false);
    let _ = window.set_always_on_bottom(false);
    let _ = window.set_always_on_bottom(true);
    let _ = window.set_skip_taskbar(true);
}

fn promote_sticky_item_over_peers(app: &tauri::AppHandle, target_label: &str) {
    // Keep all sticky item windows on desktop layer, but re-order peers first,
    // then place the target last so it appears above previous notes.
    for (label, window) in app.webview_windows() {
        if label.starts_with(STICKY_NOTE_ITEM_PREFIX) && label != target_label {
            reorder_sticky_item_layer(&window);
        }
    }
    if let Some(target) = app.get_webview_window(target_label) {
        reorder_sticky_item_layer(&target);
    }
}

fn place_sticky_window_top_right(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    width: f64,
) {
    const MARGIN_X: f64 = 12.0;
    const MARGIN_Y: f64 = 68.0;
    let monitor = app
        .get_webview_window("main")
        .and_then(|main_window| {
            main_window
                .current_monitor()
                .ok()
                .flatten()
                .or_else(|| main_window.primary_monitor().ok().flatten())
        })
        .or_else(|| {
            window
                .current_monitor()
                .ok()
                .flatten()
                .or_else(|| window.primary_monitor().ok().flatten())
        });
    if let Some(monitor) = monitor {
        let scale_factor = monitor.scale_factor();
        let monitor_position = monitor.position().to_logical::<f64>(scale_factor);
        let monitor_size = monitor.size().to_logical::<f64>(scale_factor);
        let x = monitor_position.x + (monitor_size.width - width - MARGIN_X).max(0.0);
        let y = monitor_position.y + MARGIN_Y;
        let _ = window.set_position(LogicalPosition::new(x, y));
        return;
    }
    let _ = window.set_position(LogicalPosition::new(120.0, 120.0));
}

fn show_sticky_note_window_fallback(app: &tauri::AppHandle) -> Result<(), AppError> {
    let width = 360.0;
    let height = 520.0;
    let window = if let Some(existing) = app.get_webview_window(STICKY_NOTE_LABEL) {
        existing
    } else {
        WebviewWindowBuilder::new(
            app,
            STICKY_NOTE_LABEL,
            WebviewUrl::App("sticky-note.html".into()),
        )
        .title("桌面便签")
        .inner_size(width, height)
        .min_inner_size(STICKY_NOTE_MIN_WIDTH as f64, STICKY_NOTE_MIN_HEIGHT as f64)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(false)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| AppError::System(e.to_string()))?
    };
    let _ = window.set_size(LogicalSize::new(width, height));
    let _ = window.set_shadow(false);
    window.show().map_err(|e| AppError::System(e.to_string()))?;
    let _ = window.unminimize();
    let _ = window.set_always_on_top(false);
    let _ = window.set_always_on_bottom(false);
    let _ = window.set_skip_taskbar(true);
    let app_for_position = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(window) = app_for_position.get_webview_window(STICKY_NOTE_LABEL) {
            place_sticky_window_top_right(&app_for_position, &window, width);
        }
    });
    let _ = app.emit("sticky-note-visibility", true);
    Ok(())
}

fn show_sticky_note_item_window(app: &tauri::AppHandle, note: &StickyNote) -> Result<(), AppError> {
    let label = sticky_note_item_label(&note.task_id);
    let refresh_event = sticky_note_item_refresh_event(&note.task_id);
    let window = if let Some(existing) = app.get_webview_window(&label) {
        existing
    } else {
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App("sticky-note-item.html".into()))
            .title("便签")
            .inner_size(STICKY_NOTE_ITEM_WIDTH, STICKY_NOTE_ITEM_HEIGHT)
            .min_inner_size(220.0, 180.0)
            .resizable(true)
            .focused(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_bottom(true)
            .skip_taskbar(true)
            .visible(false)
            .build()
            .map_err(|e| AppError::System(e.to_string()))?
    };

    let _ = window.set_position(LogicalPosition::new(note.pos_x, note.pos_y));
    let _ = window.set_shadow(false);
    enforce_sticky_item_layer(&window);
    let _ = window.emit(&refresh_event, note.clone());
    window.show().map_err(|e| AppError::System(e.to_string()))?;
    promote_sticky_item_over_peers(app, &label);
    Ok(())
}

fn sync_sticky_note_window(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), AppError> {
    let mut sanitized_settings = settings.clone();
    sanitized_settings.sticky_note_opacity =
        normalize_sticky_note_opacity(sanitized_settings.sticky_note_opacity);

    if !settings.sticky_note_enabled {
        if let Some(window) = app.get_webview_window(STICKY_NOTE_LABEL) {
            let _ = window.hide();
        }
        let _ = app.emit("sticky-note-settings-updated", sanitized_settings);
        let _ = app.emit("sticky-note-visibility", false);
        return Ok(());
    }

    let (width, height) =
        normalize_sticky_note_size(settings.sticky_note_width, settings.sticky_note_height);
    let window = if let Some(existing) = app.get_webview_window(STICKY_NOTE_LABEL) {
        existing
    } else {
        WebviewWindowBuilder::new(
            app,
            STICKY_NOTE_LABEL,
            WebviewUrl::App("sticky-note.html".into()),
        )
        .title("桌面便签")
        .inner_size(width, height)
        .min_inner_size(STICKY_NOTE_MIN_WIDTH as f64, STICKY_NOTE_MIN_HEIGHT as f64)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(false)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| AppError::System(e.to_string()))?
    };

    let _ = window.set_size(LogicalSize::new(width, height));
    let _ = window.set_resizable(true);
    let _ = window.set_shadow(false);
    window.show().map_err(|e| AppError::System(e.to_string()))?;
    let _ = window.unminimize();
    let _ = window.set_always_on_top(false);
    let _ = window.set_always_on_bottom(false);
    let _ = window.set_skip_taskbar(true);
    let app_for_position = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(window) = app_for_position.get_webview_window(STICKY_NOTE_LABEL) {
            place_sticky_window_top_right(&app_for_position, &window, width);
        }
    });
    let _ = app.emit("sticky-note-settings-updated", sanitized_settings);
    let _ = app.emit("sticky-note-visibility", true);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            if window.label() == STICKY_NOTE_LABEL {
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window.hide();
                        if let Some(state) = window.app_handle().try_state::<AppState>() {
                            let _ = state.db.update_sticky_note_enabled(false);
                            let _ = state.sync.notify_local_change();
                        }
                        let _ = window.app_handle().emit("sticky-note-visibility", false);
                    }
                    WindowEvent::Resized(size) => {
                        if let Some(state) = window.app_handle().try_state::<AppState>() {
                            let scale_factor = window.scale_factor().unwrap_or(1.0);
                            let logical = size.to_logical::<f64>(scale_factor);
                            let _ = state.db.update_sticky_note_size(
                                logical.width.round() as i64,
                                logical.height.round() as i64,
                            );
                        }
                    }
                    WindowEvent::Moved(position) => {
                        if let Some(state) = window.app_handle().try_state::<AppState>() {
                            let scale_factor = window.scale_factor().unwrap_or(1.0);
                            let logical = position.to_logical::<f64>(scale_factor);
                            let _ = state.db.update_sticky_note_position(logical.x, logical.y);
                        }
                    }
                    _ => {}
                }
            }
            if window.label().starts_with(STICKY_NOTE_ITEM_PREFIX) {
                let note_id = note_id_from_item_label(window.label());
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window.hide();
                        if let (Some(state), Some(note_id)) =
                            (window.app_handle().try_state::<AppState>(), note_id)
                        {
                            let _ = state.db.close_sticky_note(&note_id);
                            let _ = state.sync.notify_local_change();
                            let _ = window.app_handle().emit("sticky-note-changed", note_id);
                        }
                    }
                    WindowEvent::Moved(position) => {
                        if let (Some(state), Some(note_id)) =
                            (window.app_handle().try_state::<AppState>(), note_id)
                        {
                            let scale_factor = window.scale_factor().unwrap_or(1.0);
                            let logical = position.to_logical::<f64>(scale_factor);
                            let _ = state.db.move_sticky_note(&note_id, logical.x, logical.y);
                        }
                    }
                    _ => {}
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
            list_sticky_notes,
            get_sticky_note_by_window_label,
            open_sticky_note,
            create_sticky_note,
            save_sticky_note_content,
            update_sticky_note_title,
            move_sticky_note,
            close_sticky_note,
            close_sticky_note_by_window_label,
            set_sticky_note_opacity,
            is_sticky_note_window_visible,
            set_sticky_note_window_visible,
            force_show_sticky_note_window,
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
                let dialog_title = if dev_mode {
                    "任务提醒 [开发]"
                } else {
                    "任务提醒"
                };
                match InstanceLock::try_lock(&lock_path)? {
                    Some(lock) => {
                        app.manage(lock);
                    }
                    None => {
                        eprintln!("{}: 应用已经在运行", dialog_title);
                        app_handle.exit(0);
                        return Ok(());
                    }
                }

                tray::setup_tray(&app_handle).map_err(|e| AppError::System(e.to_string()))?;

                // 开发模式：窗口标题加 [开发] 标识
                if dev_mode {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.set_title("任务提醒应用 [开发]");
                    }
                }

                let db = DbManager::new(paths::db_path(&data_dir))?;
                let snapshot = Arc::new(Mutex::new(None));
                let sync = CloudSyncService::new(app_handle.clone(), db.clone());
                let scheduler = ReminderScheduler::new(
                    app_handle.clone(),
                    db.clone(),
                    sync.clone(),
                    snapshot.clone(),
                );
                scheduler.schedule_existing()?;
                sync.start()?;
                maintenance::start_maintenance(db.clone());

                let mut settings = db.load_settings()?;
                if settings.auto_start_enabled {
                    let _ = autostart::enable_autostart();
                }
                if settings.sticky_note_enabled {
                    settings.sticky_note_enabled = false;
                    let _ = db.update_sticky_note_enabled(false);
                    let _ = db.close_all_sticky_notes();
                }
                sync_sticky_note_window(&app_handle, &settings)?;

                let state = AppState {
                    db,
                    scheduler,
                    sync,
                    notification_snapshot: snapshot,
                };
                app.manage(state);
                Ok(())
            })();
            result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}
