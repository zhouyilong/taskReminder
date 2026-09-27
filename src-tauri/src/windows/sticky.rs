//! 便签窗口管理：窗口标签编码、创建与显示、层级（置顶/桌面层）、UI 状态注入与窗口事件。

use serde::Serialize;
use tauri::{
    Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

use crate::db::DbManager;
use crate::errors::AppError;
use crate::models::{StickyNote, UiStatePayload};
use crate::state::AppState;

pub const STICKY_NOTE_ITEM_PREFIX: &str = "sticky-note-item-";
const STICKY_NOTE_ITEM_WIDTH: f64 = 284.0;
const STICKY_NOTE_ITEM_HEIGHT: f64 = 280.0;
const STICKY_NOTE_ITEM_MIN_WIDTH: f64 = 220.0;
const STICKY_NOTE_ITEM_MIN_HEIGHT: f64 = 180.0;

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

pub fn sticky_note_item_label(note_id: &str) -> String {
    format!("{}{}", STICKY_NOTE_ITEM_PREFIX, encode_note_id(note_id))
}

fn sticky_note_item_refresh_event(note_id: &str) -> String {
    format!("sticky-note-item-refresh-{}", encode_note_id(note_id))
}

fn sticky_note_item_url() -> WebviewUrl {
    // Use App URL on every platform. In `tauri dev` Tauri rewrites this to
    // `build.devUrl`, which keeps the sticky-note window on the same origin as
    // the main window so Linux WebKitGTK still grants IPC. An External
    // localhost URL is treated as remote on Linux and then invoke/save fail.
    WebviewUrl::App("sticky-note-item.html".into())
}

fn sticky_note_bootstrap_script(note: &StickyNote) -> Option<String> {
    let payload_json = serde_json::to_string(note).ok()?;
    Some(format!(
        r#"(function () {{
  try {{
    const payload = {payload_json};
    window.__TASKREMINDER_STICKY_NOTE = payload;
    window.dispatchEvent(new CustomEvent("taskreminder-sticky-note", {{ detail: payload }}));
  }} catch (error) {{
    console.error("[taskreminder] apply sticky note failed", error);
  }}
}})();"#
    ))
}

fn apply_sticky_note_via_eval(window: &tauri::WebviewWindow, note: &StickyNote) {
    if let Some(script) = sticky_note_bootstrap_script(note) {
        let _ = window.eval(script);
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StickyNoteReminderPayload {
    task_id: String,
    reminder_time: Option<String>,
}

pub fn emit_sticky_note_reminder(
    app: &tauri::AppHandle,
    task_id: &str,
    reminder_time: Option<String>,
) {
    let Some(window) = app.get_webview_window(&sticky_note_item_label(task_id)) else {
        return;
    };
    let _ = window.emit(
        "sticky-note-reminder-updated",
        StickyNoteReminderPayload {
            task_id: task_id.to_string(),
            reminder_time,
        },
    );
}

pub fn note_id_from_item_label(label: &str) -> Option<String> {
    label
        .strip_prefix(STICKY_NOTE_ITEM_PREFIX)
        .and_then(decode_note_id)
}

fn is_sticky_item_pinned(state: &AppState, note_id: &str) -> bool {
    state.db.get_sticky_note_pinned(note_id).unwrap_or(false)
}

pub fn apply_sticky_item_layer(window: &tauri::WebviewWindow, is_pinned: bool) {
    if is_pinned {
        // Preserve pinned notes as top-most instead of forcing them back to the desktop layer.
        let _ = window.set_always_on_bottom(false);
        let _ = window.set_always_on_top(true);
    } else {
        let _ = window.set_always_on_top(false);
        let _ = window.set_always_on_bottom(false);
        let _ = window.set_always_on_bottom(true);
    }
    let _ = window.set_skip_taskbar(true);
}

fn enforce_sticky_item_layer(window: &tauri::WebviewWindow, is_pinned: bool) {
    apply_sticky_item_layer(window, is_pinned);
}

fn reorder_sticky_item_layer(window: &tauri::WebviewWindow, is_pinned: bool) {
    // Re-apply the correct layer so unpinned notes stay on the desktop layer
    // while pinned notes keep their top-most state.
    apply_sticky_item_layer(window, is_pinned);
}

pub fn promote_sticky_item_over_peers(app: &tauri::AppHandle, target_label: &str) {
    // Keep all sticky item windows on desktop layer, but re-order peers first,
    // then place the target last so it appears above previous notes.
    let pinned_state = app.try_state::<AppState>();
    for (label, window) in app.webview_windows() {
        if label.starts_with(STICKY_NOTE_ITEM_PREFIX) && label != target_label {
            let is_pinned = pinned_state
                .as_ref()
                .and_then(|state| {
                    note_id_from_item_label(&label)
                        .map(|note_id| is_sticky_item_pinned(state, &note_id))
                })
                .unwrap_or_else(|| window.is_always_on_top().unwrap_or(false));
            reorder_sticky_item_layer(&window, is_pinned);
        }
    }
    if let Some(target) = app.get_webview_window(target_label) {
        let is_pinned = pinned_state
            .as_ref()
            .and_then(|state| {
                note_id_from_item_label(target_label)
                    .map(|note_id| is_sticky_item_pinned(state, &note_id))
            })
            .unwrap_or_else(|| target.is_always_on_top().unwrap_or(false));
        reorder_sticky_item_layer(&target, is_pinned);
    }
}

fn apply_ui_state_via_eval(window: &tauri::WebviewWindow, payload: &UiStatePayload) {
    let Ok(payload_json) = serde_json::to_string(payload) else {
        return;
    };
    let script = format!(
        r#"(function () {{
  try {{
    const payload = {payload_json};
    window.__TASKREMINDER_UI_STATE = payload;
    try {{
      window.localStorage.setItem("appTheme", payload.theme);
      window.localStorage.setItem("uiScale", String(payload.uiScale));
      window.localStorage.setItem("windowOpacity", String(payload.windowOpacity));
    }} catch (_storageError) {{}}
    window.dispatchEvent(new CustomEvent("taskreminder-ui-state", {{ detail: payload }}));
  }} catch (error) {{
    console.error("[taskreminder] apply ui state failed", error);
  }}
}})();"#,
    );
    let _ = window.eval(script);
}

pub fn emit_ui_state_to_sticky_windows(app: &tauri::AppHandle, payload: &UiStatePayload) {
    // Use app-level broadcast so all webview windows (including external dev-mode URLs)
    // receive the event via their `listen("ui-state-changed")` handlers — same mechanism
    // used by the working `settings-updated` broadcast.
    let _ = app.emit("ui-state-changed", payload.clone());
    for (label, window) in app.webview_windows() {
        if label.starts_with(STICKY_NOTE_ITEM_PREFIX) {
            let _ = window.emit("ui-state-changed", payload.clone());
            apply_ui_state_via_eval(&window, payload);
        }
    }
}

fn emit_cached_ui_state_to_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Ok(snapshot) = state.ui_state.lock() else {
        return;
    };
    if let Some(payload) = snapshot.clone() {
        let _ = window.emit("ui-state-changed", payload.clone());
        apply_ui_state_via_eval(window, &payload);
    }
}

pub fn show_sticky_note_item_window(
    app: &tauri::AppHandle,
    note: &StickyNote,
) -> Result<(), AppError> {
    let label = sticky_note_item_label(&note.task_id);
    let refresh_event = sticky_note_item_refresh_event(&note.task_id);
    let is_pinned = note.is_pinned;
    let window = if let Some(existing) = app.get_webview_window(&label) {
        existing
    } else {
        // Linux WebKitGTK does not reliably repaint transparent windows after the
        // first frame, so the "载入便签..." placeholder can stay on screen even
        // after the note payload arrives. Keep Windows (and other platforms)
        // transparent so the floating paper look is unchanged there.
        let mut builder = WebviewWindowBuilder::new(app, &label, sticky_note_item_url())
            .title("便签")
            .inner_size(STICKY_NOTE_ITEM_WIDTH, STICKY_NOTE_ITEM_HEIGHT)
            .min_inner_size(STICKY_NOTE_ITEM_MIN_WIDTH, STICKY_NOTE_ITEM_MIN_HEIGHT)
            .resizable(true)
            .focused(false)
            .decorations(false)
            .transparent(!cfg!(target_os = "linux"))
            .shadow(false)
            .always_on_bottom(true)
            .skip_taskbar(true)
            .visible(false);
        if let Some(script) = sticky_note_bootstrap_script(note) {
            builder = builder.initialization_script(script);
        }
        builder
            .build()
            .map_err(|e| AppError::System(e.to_string()))?
    };

    let width = note.width.max(STICKY_NOTE_ITEM_MIN_WIDTH);
    let height = note.height.max(STICKY_NOTE_ITEM_MIN_HEIGHT);
    let _ = window.set_size(LogicalSize::new(width, height));
    let _ = window.set_position(LogicalPosition::new(note.pos_x, note.pos_y));
    let _ = window.set_shadow(false);
    enforce_sticky_item_layer(&window, is_pinned);
    let _ = window.emit(&refresh_event, note.clone());
    apply_sticky_note_via_eval(&window, note);
    emit_cached_ui_state_to_window(app, &window);
    window.show().map_err(|e| AppError::System(e.to_string()))?;
    promote_sticky_item_over_peers(app, &label);
    Ok(())
}

pub fn restore_open_sticky_note_items(
    app: &tauri::AppHandle,
    db: &DbManager,
) -> Result<(), AppError> {
    let mut notes = db.list_sticky_notes()?;
    notes.retain(|note| note.is_open);
    notes.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
    for note in notes {
        if let Err(err) = show_sticky_note_item_window(app, &note) {
            eprintln!(
                "[sticky-note] 启动恢复便签窗口失败 task_id={} err={}",
                note.task_id, err
            );
        }
    }
    Ok(())
}

/// 便签窗口事件：关闭时隐藏并记为已关闭，移动与缩放时保存位置和尺寸。
pub fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
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
        WindowEvent::Resized(size) => {
            if let (Some(state), Some(note_id)) =
                (window.app_handle().try_state::<AppState>(), note_id)
            {
                let scale_factor = window.scale_factor().unwrap_or(1.0);
                let logical = size.to_logical::<f64>(scale_factor);
                let _ = state
                    .db
                    .resize_sticky_note(&note_id, logical.width, logical.height);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_note() -> StickyNote {
        StickyNote {
            task_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            title: "便签".to_string(),
            note_type: "CUSTOM".to_string(),
            content: "hello".to_string(),
            pos_x: 48.0,
            pos_y: 76.0,
            width: 284.0,
            height: 280.0,
            is_open: true,
            is_pinned: false,
            created_at: "2026-01-01T00:00:00".to_string(),
            updated_at: "2026-01-01T00:00:00".to_string(),
            reminder_time: Some("2026-09-22T16:30:00".to_string()),
        }
    }

    #[test]
    fn encode_decode_note_id_roundtrip() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(decode_note_id(&encode_note_id(id)).as_deref(), Some(id));
        assert_eq!(
            note_id_from_item_label(&sticky_note_item_label(id)).as_deref(),
            Some(id)
        );
    }

    #[test]
    fn sticky_note_item_url_uses_app_protocol() {
        match sticky_note_item_url() {
            WebviewUrl::App(path) => {
                assert_eq!(path.to_string_lossy(), "sticky-note-item.html");
            }
            _ => panic!("sticky note windows must use WebviewUrl::App"),
        }
    }

    #[test]
    fn sticky_note_bootstrap_script_includes_payload() {
        let note = sample_note();
        let script = sticky_note_bootstrap_script(&note).expect("script");
        assert!(script.contains(&note.task_id));
        assert!(script.contains("__TASKREMINDER_STICKY_NOTE"));
        assert!(script.contains("taskreminder-sticky-note"));
        assert!(script.contains("2026-09-22T16:30:00"));
    }
}
