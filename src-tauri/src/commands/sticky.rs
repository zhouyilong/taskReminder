//! 便签命令：打开、新建、编辑内容/标题/提醒、移动、关闭与置顶。

use serde::Deserialize;
use tauri::{Emitter, Manager, State};

use crate::commands::tasks::normalize_reminder_time;
use crate::commands::{into_api, ApiResult};
use crate::errors::AppError;
use crate::models::StickyNote;
use crate::scheduler;
use crate::state::AppState;
use crate::windows::sticky::{
    apply_sticky_item_layer, emit_sticky_note_reminder, note_id_from_item_label,
    promote_sticky_item_over_peers, show_sticky_note_item_window, sticky_note_item_label,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenStickyNotePayload {
    task_id: String,
    title: Option<String>,
    default_x: Option<f64>,
    default_y: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveStickyNoteContentPayload {
    task_id: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStickyNoteTitlePayload {
    task_id: String,
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStickyNoteReminderPayload {
    task_id: String,
    reminder_time: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStickyNotePayload {
    task_id: String,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStickyNotePayload {
    title: Option<String>,
    content: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
    default_x: Option<f64>,
    default_y: Option<f64>,
}

#[tauri::command]
pub fn get_sticky_note_by_window_label(
    state: State<AppState>,
    label: String,
) -> ApiResult<Option<StickyNote>> {
    let Some(note_id) = note_id_from_item_label(&label) else {
        return Ok(None);
    };
    into_api(state.db.get_sticky_note(&note_id))
}

#[tauri::command]
pub fn get_sticky_note(state: State<AppState>, note_id: String) -> ApiResult<Option<StickyNote>> {
    into_api(state.db.get_sticky_note(&note_id))
}

#[tauri::command]
pub fn open_sticky_note(
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
pub fn create_sticky_note(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: CreateStickyNotePayload,
) -> ApiResult<StickyNote> {
    into_api(create_custom_sticky_note_via_app(
        &app,
        state.inner(),
        payload.title.as_deref().unwrap_or(""),
        payload.content.as_deref(),
        payload.default_x,
        payload.default_y,
        payload.width,
        payload.height,
    ))
}

pub(crate) fn create_custom_sticky_note_via_app(
    app: &tauri::AppHandle,
    state: &AppState,
    title: &str,
    content: Option<&str>,
    default_x: Option<f64>,
    default_y: Option<f64>,
    default_width: Option<f64>,
    default_height: Option<f64>,
) -> Result<StickyNote, AppError> {
    let note = state.db.create_custom_sticky_note(
        title,
        content,
        default_x,
        default_y,
        default_width,
        default_height,
    )?;
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
pub fn save_sticky_note_content(
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
pub fn update_sticky_note_title(
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
pub fn update_sticky_note_reminder(
    app: tauri::AppHandle,
    state: State<AppState>,
    payload: UpdateStickyNoteReminderPayload,
) -> ApiResult<()> {
    let reminder_time = normalize_reminder_time(payload.reminder_time);
    if let Some(value) = reminder_time.as_deref() {
        into_api(scheduler::is_future(value))?;
    }
    into_api(
        state
            .db
            .set_task_reminder_time(&payload.task_id, reminder_time.as_deref()),
    )?;
    state.scheduler.cancel_task(&payload.task_id);
    if let Some(value) = reminder_time.as_deref() {
        if scheduler::is_future(value).unwrap_or(false) {
            if let Some(task) = into_api(state.db.get_task(&payload.task_id))? {
                into_api(state.scheduler.schedule_task(task))?;
            }
        }
    }
    emit_sticky_note_reminder(&app, &payload.task_id, reminder_time);
    let _ = app.emit("sticky-note-changed", payload.task_id);
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn move_sticky_note(state: State<AppState>, payload: MoveStickyNotePayload) -> ApiResult<()> {
    into_api(
        state
            .db
            .move_sticky_note(&payload.task_id, payload.x, payload.y),
    )?;
    into_api(state.sync.notify_local_change())?;
    Ok(())
}

#[tauri::command]
pub fn close_sticky_note(
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
pub fn close_sticky_note_by_window_label(
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
pub fn set_sticky_note_pinned_by_window_label(
    app: tauri::AppHandle,
    state: State<AppState>,
    label: String,
    pinned: bool,
) -> ApiResult<bool> {
    let Some(note_id) = note_id_from_item_label(&label) else {
        return Ok(false);
    };
    into_api(state.db.set_sticky_note_pinned(&note_id, pinned))?;
    if let Some(window) = app.get_webview_window(&label) {
        apply_sticky_item_layer(&window, pinned);
    }
    promote_sticky_item_over_peers(&app, &label);
    Ok(pinned)
}

#[tauri::command]
pub fn get_sticky_note_pinned_by_window_label(
    state: State<AppState>,
    label: String,
) -> ApiResult<bool> {
    let Some(note_id) = note_id_from_item_label(&label) else {
        return Ok(false);
    };
    into_api(state.db.get_sticky_note_pinned(&note_id))
}
