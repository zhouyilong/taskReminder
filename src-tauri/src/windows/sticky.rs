//! 便签窗口管理：窗口标签编码、创建与显示、层级（置顶/桌面层）、UI 状态注入与窗口事件。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::{
    Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

use crate::db::DbManager;
use crate::errors::AppError;
use crate::models::{StickyNote, UiStatePayload};
use crate::state::AppState;
use crate::windows::placement::{self, Rect};

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
    if !encoded.len().is_multiple_of(2) {
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

/// 待办被删除或完成后收起它的便签窗口（`sticky_is_open` 由数据库在同一次更新中清掉），
/// 并通知便签管理列表刷新。
pub fn hide_sticky_note_window(app: &tauri::AppHandle, task_id: &str) {
    if let Some(window) = app.get_webview_window(&sticky_note_item_label(task_id)) {
        let _ = window.hide();
    }
    let _ = app.emit("sticky-note-changed", task_id.to_string());
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StickyNoteColorPayload {
    task_id: String,
    color: String,
}

/// 通知便签窗口改用新颜色（在主窗口的便签列表中修改时）。
pub fn emit_sticky_note_color(app: &tauri::AppHandle, task_id: &str, color: &str) {
    if let Some(window) = app.get_webview_window(&sticky_note_item_label(task_id)) {
        let _ = window.emit(
            "sticky-note-color-updated",
            StickyNoteColorPayload {
                task_id: task_id.to_string(),
                color: color.to_string(),
            },
        );
    }
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

/// 为越界校正而移动窗口时记下校正后的位置：随后的 `Moved` 事件落在这里时不保存，
/// 避免把本机的校正同步回原设备。用户之后拖到别处才会保存。
fn corrected_positions() -> &'static Mutex<HashMap<String, (f64, f64)>> {
    static POSITIONS: OnceLock<Mutex<HashMap<String, (f64, f64)>>> = OnceLock::new();
    POSITIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 窗口停在校正位置（1 px 以内）时返回 `true`；移到别处后清除记录。
fn is_at_corrected_position(note_id: &str, x: f64, y: f64) -> bool {
    let Ok(mut positions) = corrected_positions().lock() else {
        return false;
    };
    match positions.get(note_id) {
        Some(&(cx, cy)) if (cx - x).abs() <= 1.0 && (cy - y).abs() <= 1.0 => true,
        Some(_) => {
            positions.remove(note_id);
            false
        }
        None => false,
    }
}

/// 显示器工作区（按窗口当前缩放换算为逻辑像素，与 `set_position` 的换算一致）。
fn monitor_work_areas(window: &tauri::WebviewWindow) -> Vec<Rect> {
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|monitor| {
            let area = monitor.work_area();
            let position = area.position.to_logical::<f64>(scale_factor);
            let size = area.size.to_logical::<f64>(scale_factor);
            Rect::new(position.x, position.y, size.width, size.height)
        })
        .collect()
}

/// 便签应显示的位置：保存的位置在本机屏幕外时移回屏幕内（只移动窗口，不写库）。
fn visible_position(
    window: &tauri::WebviewWindow,
    note: &StickyNote,
    width: f64,
    height: f64,
) -> (f64, f64) {
    let saved = Rect::new(note.pos_x, note.pos_y, width, height);
    let corrected = placement::clamp_to_work_areas(saved, &monitor_work_areas(window));
    if let Ok(mut positions) = corrected_positions().lock() {
        match corrected {
            Some(position) => {
                positions.insert(note.task_id.clone(), position);
            }
            None => {
                positions.remove(&note.task_id);
            }
        }
    }
    corrected.unwrap_or((note.pos_x, note.pos_y))
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
    let (x, y) = visible_position(&window, note, width, height);
    let _ = window.set_position(LogicalPosition::new(x, y));
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

/// 是否有正在显示的便签窗口。
pub fn any_sticky_window_visible(app: &tauri::AppHandle) -> bool {
    app.webview_windows().iter().any(|(label, window)| {
        label.starts_with(STICKY_NOTE_ITEM_PREFIX) && window.is_visible().unwrap_or(false)
    })
}

/// 隐藏全部便签窗口。只隐藏窗口，不改 `sticky_is_open`，避免产生同步改动；
/// 重启应用或“显示全部便签”后按原状态恢复。
pub fn hide_all_sticky_windows(app: &tauri::AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(STICKY_NOTE_ITEM_PREFIX) {
            let _ = window.hide();
        }
    }
    // 便签管理列表据此刷新“显示中 / 已隐藏”。
    let _ = app.emit("sticky-note-changed", "");
}

/// 重新显示所有处于打开状态的便签（在主线程创建或显示窗口）。
pub fn show_all_sticky_windows(app: &tauri::AppHandle) {
    let handle = app.clone();
    let result = app.run_on_main_thread(move || {
        if let Some(state) = handle.try_state::<AppState>() {
            if let Err(err) = restore_open_sticky_note_items(&handle, &state.db) {
                eprintln!("[sticky-note] 显示全部便签失败: {}", err);
            }
        }
        let _ = handle.emit("sticky-note-changed", "");
    });
    if let Err(err) = result {
        eprintln!("[sticky-note] 主线程调度显示全部便签失败: {}", err);
    }
}

/// 有便签显示时全部隐藏，否则全部显示（全局快捷键使用）。
pub fn toggle_all_sticky_windows(app: &tauri::AppHandle) {
    if any_sticky_window_visible(app) {
        hide_all_sticky_windows(app);
    } else {
        show_all_sticky_windows(app);
    }
}

/// 便签窗口事件：关闭时隐藏并记为已关闭，移动与缩放时保存位置和尺寸
/// （越界校正造成的移动不保存）。
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
                if is_at_corrected_position(&note_id, logical.x, logical.y) {
                    return;
                }
                let _ = state.db.move_sticky_note(&note_id, logical.x, logical.y);
                #[cfg(target_os = "windows")]
                snap::on_moved(window, &note_id);
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

/// 贴边吸附（仅 Windows）。
///
/// 拖动标题栏时系统进入模态移动循环，Tauri 只给出一连串 `Moved`，没有“拖动结束”事件，
/// 所以按鼠标键状态判断：移动时有鼠标键按下才算用户拖动；松开鼠标且 150 ms 内没有新的
/// 移动后吸附。打开便签、越界校正、吸附本身造成的移动发生时鼠标没有按下，不会触发；
/// 拖动途中停顿也不会被“吸走”。松开时按住 Alt 不吸附。
#[cfg(target_os = "windows")]
mod snap {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use tauri::{LogicalPosition, Manager};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VIRTUAL_KEY, VK_LBUTTON, VK_MENU, VK_RBUTTON,
    };

    use super::{monitor_work_areas, STICKY_NOTE_ITEM_PREFIX};
    use crate::state::AppState;
    use crate::windows::placement::{self, Rect};

    const SETTLE: Duration = Duration::from_millis(150);
    const POLL: Duration = Duration::from_millis(50);
    /// 防止异常情况下（如按键状态读不到松开）监视线程一直不退出。
    const MAX_WAIT: Duration = Duration::from_secs(120);

    fn key_down(key: VIRTUAL_KEY) -> bool {
        // 最高位为 1 表示按下。
        unsafe { GetAsyncKeyState(i32::from(key.0)) < 0 }
    }

    /// 左右键都算：交换了主次键的用户按的是物理右键。
    fn mouse_down() -> bool {
        key_down(VK_LBUTTON) || key_down(VK_RBUTTON)
    }

    /// 正在拖动的便签 → 最近一次移动的时间。有记录说明监视线程在运行。
    fn drags() -> &'static Mutex<HashMap<String, Instant>> {
        static DRAGS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
        DRAGS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn on_moved(window: &tauri::Window, note_id: &str) {
        let Ok(mut drags) = drags().lock() else {
            return;
        };
        if let Some(last_moved) = drags.get_mut(note_id) {
            *last_moved = Instant::now();
            return;
        }
        if !mouse_down() {
            return;
        }
        drags.insert(note_id.to_string(), Instant::now());
        // 从左边或上边缩放也会移动窗口；结束时尺寸变了就不吸附，避免把缩放后的便签整体挪动。
        let start_size = window.outer_size().ok();
        let app = window.app_handle().clone();
        let label = window.label().to_string();
        let note_id = note_id.to_string();
        std::thread::spawn(move || watch_drag(app, label, note_id, start_size));
    }

    fn watch_drag(
        app: tauri::AppHandle,
        label: String,
        note_id: String,
        start_size: Option<tauri::PhysicalSize<u32>>,
    ) {
        let started = Instant::now();
        loop {
            std::thread::sleep(POLL);
            let Ok(mut drags) = drags().lock() else {
                return;
            };
            let last_moved = drags.get(&note_id).copied().unwrap_or(started);
            let settled = last_moved.elapsed() >= SETTLE && !mouse_down();
            if settled || started.elapsed() >= MAX_WAIT {
                drags.remove(&note_id);
                break;
            }
        }
        if key_down(VK_MENU) {
            return;
        }
        let enabled = app
            .try_state::<AppState>()
            .and_then(|state| state.db.sticky_snap_enabled().ok())
            .unwrap_or(false);
        if !enabled {
            return;
        }
        let Some(window) = app.get_webview_window(&label) else {
            return;
        };
        if start_size.is_none() || window.outer_size().ok() != start_size {
            return;
        }
        snap_window(&app, &window);
    }

    fn logical_rect(window: &tauri::WebviewWindow, scale_factor: f64) -> Option<Rect> {
        let position = window
            .outer_position()
            .ok()?
            .to_logical::<f64>(scale_factor);
        let size = window.outer_size().ok()?.to_logical::<f64>(scale_factor);
        Some(Rect::new(position.x, position.y, size.width, size.height))
    }

    /// 按当前位置计算吸附位置并移动窗口；随后的 `Moved` 事件会照常保存新位置。
    fn snap_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
        // 所有窗口统一按本窗口的缩放换算，与工作区、`set_position` 的换算一致。
        let scale_factor = window.scale_factor().unwrap_or(1.0);
        let Some(note) = logical_rect(window, scale_factor) else {
            return;
        };
        let peers: Vec<Rect> = app
            .webview_windows()
            .into_iter()
            .filter(|(label, peer)| {
                label.starts_with(STICKY_NOTE_ITEM_PREFIX)
                    && label.as_str() != window.label()
                    && peer.is_visible().unwrap_or(false)
            })
            .filter_map(|(_, peer)| logical_rect(&peer, scale_factor))
            .collect();
        let (x, y) = placement::snap_position(note, &monitor_work_areas(window), &peers);
        if (x - note.x).abs() >= 0.5 || (y - note.y).abs() >= 0.5 {
            let _ = window.set_position(LogicalPosition::new(x, y));
        }
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
            color: String::new(),
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
