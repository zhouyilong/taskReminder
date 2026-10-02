//! 便签命令：管理列表、打开、新建、编辑内容/标题/提醒、移动、关闭与置顶。

use serde::{Deserialize, Serialize};
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

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStickyNotePayload {
    pub title: Option<String>,
    pub content: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub default_x: Option<f64>,
    pub default_y: Option<f64>,
}

/// 便签管理列表的一行：便签本身加上窗口此刻是否可见。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StickyNoteSummary {
    #[serde(flatten)]
    note: StickyNote,
    visible: bool,
}

/// 管理列表只列出真正在用的便签：已打开，或写过内容。
fn is_listed_sticky_note(note: &StickyNote) -> bool {
    note.is_open || !note.content.trim().is_empty()
}

#[tauri::command]
pub fn list_sticky_note_summaries(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> ApiResult<Vec<StickyNoteSummary>> {
    let notes = into_api(state.db.list_sticky_notes())?;
    Ok(notes
        .into_iter()
        .filter(is_listed_sticky_note)
        .map(|note| {
            let visible = note.is_open
                && app
                    .get_webview_window(&sticky_note_item_label(&note.task_id))
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false);
            StickyNoteSummary { note, visible }
        })
        .collect())
}

/// 显示已打开（可能被“隐藏全部便签”隐藏）的便签并置前。只操作窗口，不写库，
/// 避免在管理列表里点“置前”产生同步改动；未打开的便签走 `open_sticky_note`。
#[tauri::command]
pub fn show_sticky_note(
    app: tauri::AppHandle,
    state: State<AppState>,
    task_id: String,
) -> ApiResult<()> {
    let Some(note) = into_api(state.db.get_sticky_note(&task_id))? else {
        return Err("找不到对应便签".to_string());
    };
    // 与 `open_sticky_note` 相同：窗口可能需要新建，不能在同步命令里直接创建（Windows 上会卡死），
    // 先换到后台线程，再调度到主线程。
    std::thread::spawn(move || {
        let app_in_main = app.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            if let Err(err) = show_sticky_note_item_window(&app_in_main, &note) {
                eprintln!("[sticky-note] 显示便签窗口失败: {}", err);
                return;
            }
            let label = sticky_note_item_label(&note.task_id);
            if let Some(window) = app_in_main.get_webview_window(&label) {
                let _ = window.set_focus();
            }
            let _ = app_in_main.emit("sticky-note-changed", note.task_id.clone());
        }) {
            eprintln!("[sticky-note] 主线程调度显示便签失败: {}", err);
        }
    });
    Ok(())
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
        &payload,
    ))
}

pub(crate) fn create_custom_sticky_note_via_app(
    app: &tauri::AppHandle,
    state: &AppState,
    payload: &CreateStickyNotePayload,
) -> Result<StickyNote, AppError> {
    let note = state.db.create_custom_sticky_note(
        payload.title.as_deref().unwrap_or(""),
        payload.content.as_deref(),
        payload.default_x,
        payload.default_y,
        payload.width,
        payload.height,
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

/// 设置便签颜色（空字符串为默认颜色）；不认识的颜色拒绝写入。
#[tauri::command]
pub fn set_sticky_note_color(
    app: tauri::AppHandle,
    state: State<AppState>,
    task_id: String,
    color: String,
) -> ApiResult<()> {
    let Some(color) = crate::models::normalize_sticky_color(&color) else {
        return Err("不支持的便签颜色".to_string());
    };
    if into_api(state.db.set_sticky_note_color(&task_id, &color))? {
        crate::windows::sticky::emit_sticky_note_color(&app, &task_id, &color);
        let _ = app.emit("sticky-note-changed", task_id.clone());
        into_api(state.sync.notify_local_change())?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn note(is_open: bool, content: &str) -> StickyNote {
        StickyNote {
            task_id: "t".to_string(),
            title: "标题".to_string(),
            note_type: "TASK".to_string(),
            content: content.to_string(),
            pos_x: 0.0,
            pos_y: 0.0,
            width: 284.0,
            height: 280.0,
            is_open,
            is_pinned: false,
            created_at: String::new(),
            updated_at: String::new(),
            reminder_time: None,
            color: String::new(),
        }
    }

    #[test]
    fn lists_open_or_written_notes_only() {
        assert!(is_listed_sticky_note(&note(true, "")));
        assert!(is_listed_sticky_note(&note(false, "内容")));
        assert!(!is_listed_sticky_note(&note(false, "  \n ")));
    }

    #[test]
    fn summary_flattens_note_fields() {
        let summary = StickyNoteSummary {
            note: note(true, "内容"),
            visible: true,
        };
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["taskId"], "t");
        assert_eq!(json["isOpen"], true);
        assert_eq!(json["visible"], true);
    }
}
