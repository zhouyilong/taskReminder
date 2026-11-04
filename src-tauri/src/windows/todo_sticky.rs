//! 待办清单便签（v2.2.4）：在桌面上常驻、列出全部未完成待办的卡片窗口。
//!
//! 只有一个窗口（标签 `sticky-todo-list`）。打开、位置、尺寸与锚定都是本机设置
//! （`settings.todo_sticky_state`），不参与同步；层级与普通便签相同：未锚定时在桌面层，
//! 锚定时置顶并锁定位置。

use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::db::DbManager;
use crate::errors::AppError;
use crate::models::{
    TodoStickyState, TODO_STICKY_HEIGHT, TODO_STICKY_MIN_HEIGHT, TODO_STICKY_MIN_WIDTH,
    TODO_STICKY_WIDTH,
};
use crate::state::AppState;
use crate::windows::placement::{self, Rect};
use crate::windows::sticky::{apply_sticky_item_layer, monitor_work_areas};

pub const TODO_STICKY_LABEL: &str = "sticky-todo-list";
/// 打开、关闭、锚定变化时通知主窗口（便签管理列表、待办页按钮）刷新。
pub const TODO_STICKY_CHANGED_EVENT: &str = "todo-sticky-changed";
/// 没有保存过位置时，距离工作区右上角的距离。
const DEFAULT_MARGIN: f64 = 24.0;
const DEFAULT_TOP: f64 = 72.0;

fn todo_sticky_url() -> WebviewUrl {
    // 与其他窗口一样用 App URL（Linux WebKitGTK 把 External localhost 视为远程源，会拒绝 invoke）。
    WebviewUrl::App("sticky-todo-list.html".into())
}

/// 没有保存过位置时放在工作区的右上角。
fn default_position(areas: &[Rect], width: f64) -> (f64, f64) {
    match areas.first() {
        Some(area) => (
            area.x + (area.width - width - DEFAULT_MARGIN).max(0.0),
            area.y + DEFAULT_TOP,
        ),
        None => (DEFAULT_MARGIN, DEFAULT_TOP),
    }
}

/// 应显示的位置：保存的位置在本机屏幕外时移回屏幕内。
fn display_position(areas: &[Rect], state: &TodoStickyState) -> (f64, f64) {
    let (Some(x), Some(y)) = (state.x, state.y) else {
        return default_position(areas, state.width);
    };
    let saved = Rect::new(x, y, state.width, state.height);
    placement::clamp_to_work_areas(saved, areas).unwrap_or((x, y))
}

fn emit_changed(app: &tauri::AppHandle, state: &TodoStickyState) {
    let _ = app.emit(TODO_STICKY_CHANGED_EVENT, state.clone());
}

/// 创建（或显示已有的）清单窗口并按保存的状态摆放。必须在主线程调用。
pub fn show_todo_sticky_window(
    app: &tauri::AppHandle,
    state: &TodoStickyState,
) -> Result<(), AppError> {
    let window = if let Some(existing) = app.get_webview_window(TODO_STICKY_LABEL) {
        existing
    } else {
        let window = WebviewWindowBuilder::new(app, TODO_STICKY_LABEL, todo_sticky_url())
            .title("待办清单")
            .inner_size(TODO_STICKY_WIDTH, TODO_STICKY_HEIGHT)
            .min_inner_size(TODO_STICKY_MIN_WIDTH, TODO_STICKY_MIN_HEIGHT)
            .resizable(true)
            .maximizable(false)
            .focused(false)
            .decorations(false)
            // Linux 透明窗口不重绘，与普通便签相同只在 Linux 关闭透明。
            .transparent(!cfg!(target_os = "linux"))
            .shadow(false)
            .always_on_bottom(true)
            .skip_taskbar(true)
            .visible(false)
            .build()
            .map_err(|e| AppError::System(e.to_string()))?;
        #[cfg(target_os = "windows")]
        crate::windows::sticky::install_snap(&window);
        window
    };
    let width = state.width.max(TODO_STICKY_MIN_WIDTH);
    let height = state.height.max(TODO_STICKY_MIN_HEIGHT);
    let _ = window.set_size(LogicalSize::new(width, height));
    let (x, y) = display_position(&monitor_work_areas(&window), state);
    let _ = window.set_position(LogicalPosition::new(x, y));
    let _ = window.set_shadow(false);
    apply_sticky_item_layer(&window, state.pinned);
    window.show().map_err(|e| AppError::System(e.to_string()))?;
    Ok(())
}

/// 打开清单便签：记为打开并在主线程显示（不能在同步命令所在的线程直接建窗口，Windows 上会卡死）。
pub fn open_todo_sticky(
    app: &tauri::AppHandle,
    db: &DbManager,
) -> Result<TodoStickyState, AppError> {
    let state = db.update_todo_sticky_state(|state| state.open = true)?;
    let handle = app.clone();
    let shown = state.clone();
    std::thread::spawn(move || {
        let app_in_main = handle.clone();
        if let Err(err) = handle.run_on_main_thread(move || {
            if let Err(err) = show_todo_sticky_window(&app_in_main, &shown) {
                eprintln!("[todo-sticky] 显示待办清单便签失败: {}", err);
                return;
            }
            if let Some(window) = app_in_main.get_webview_window(TODO_STICKY_LABEL) {
                let _ = window.set_focus();
            }
        }) {
            eprintln!("[todo-sticky] 主线程调度显示待办清单便签失败: {}", err);
        }
    });
    emit_changed(app, &state);
    Ok(state)
}

/// 关闭清单便签：隐藏窗口并记为关闭。
pub fn close_todo_sticky(
    app: &tauri::AppHandle,
    db: &DbManager,
) -> Result<TodoStickyState, AppError> {
    if let Some(window) = app.get_webview_window(TODO_STICKY_LABEL) {
        let _ = window.hide();
    }
    let state = db.update_todo_sticky_state(|state| state.open = false)?;
    emit_changed(app, &state);
    Ok(state)
}

/// 锚定（置顶并锁定位置）或取消。
pub fn set_todo_sticky_pinned(
    app: &tauri::AppHandle,
    db: &DbManager,
    pinned: bool,
) -> Result<TodoStickyState, AppError> {
    let state = db.update_todo_sticky_state(|state| state.pinned = pinned)?;
    if let Some(window) = app.get_webview_window(TODO_STICKY_LABEL) {
        apply_sticky_item_layer(&window, pinned);
    }
    emit_changed(app, &state);
    Ok(state)
}

/// 设置清单颜色（已规范化，空字符串为默认颜色）。
pub fn set_todo_sticky_color(
    app: &tauri::AppHandle,
    db: &DbManager,
    color: String,
) -> Result<TodoStickyState, AppError> {
    let state = db.update_todo_sticky_state(|state| state.color = color)?;
    emit_changed(app, &state);
    Ok(state)
}

/// 启动时恢复：上次退出时打开着的清单便签重新显示。
pub fn restore_todo_sticky(app: &tauri::AppHandle, db: &DbManager) {
    match db.todo_sticky_state() {
        Ok(state) if state.open => {
            if let Err(err) = show_todo_sticky_window(app, &state) {
                eprintln!("[todo-sticky] 启动恢复待办清单便签失败: {}", err);
            }
        }
        Ok(_) => {}
        Err(err) => eprintln!("[todo-sticky] 读取待办清单便签状态失败: {}", err),
    }
}

/// “隐藏全部便签”：只隐藏窗口，不改打开状态。
pub fn hide_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(TODO_STICKY_LABEL) {
        let _ = window.hide();
    }
}

/// 窗口事件：关闭时隐藏并记为关闭，移动与缩放时保存位置和尺寸（锚定时位置不变）。
pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    let app = window.app_handle();
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
            if let Ok(saved) = state.db.update_todo_sticky_state(|s| s.open = false) {
                emit_changed(app, &saved);
            }
        }
        tauri::WindowEvent::Moved(position) => {
            // 隐藏、最小化时系统会把窗口挪到屏幕外，不保存这些位置。
            if !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false) {
                return;
            }
            let scale_factor = window.scale_factor().unwrap_or(1.0);
            let logical = position.to_logical::<f64>(scale_factor);
            let _ = state.db.update_todo_sticky_state(|s| {
                s.x = Some(logical.x);
                s.y = Some(logical.y);
            });
        }
        tauri::WindowEvent::Resized(size) => {
            if !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false) {
                return;
            }
            let scale_factor = window.scale_factor().unwrap_or(1.0);
            let logical = size.to_logical::<f64>(scale_factor);
            if logical.width < 1.0 || logical.height < 1.0 {
                return;
            }
            let _ = state.db.update_todo_sticky_state(|s| {
                s.width = logical.width;
                s.height = logical.height;
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_sticky_url_uses_app_protocol() {
        match todo_sticky_url() {
            WebviewUrl::App(path) => assert_eq!(path.to_string_lossy(), "sticky-todo-list.html"),
            _ => panic!("todo sticky window must use WebviewUrl::App"),
        }
    }

    #[test]
    fn default_position_is_the_top_right_of_the_first_work_area() {
        let areas = [Rect::new(0.0, 0.0, 1920.0, 1040.0)];
        assert_eq!(default_position(&areas, 300.0), (1596.0, 72.0));
        assert_eq!(default_position(&[], 300.0), (24.0, 72.0));
        let state = TodoStickyState::default();
        assert_eq!(display_position(&areas, &state), (1596.0, 72.0));
    }

    #[test]
    fn saved_position_is_kept_inside_the_screen() {
        let areas = [Rect::new(0.0, 0.0, 1920.0, 1040.0)];
        let mut state = TodoStickyState {
            x: Some(100.0),
            y: Some(200.0),
            ..TodoStickyState::default()
        };
        assert_eq!(display_position(&areas, &state), (100.0, 200.0));
        // 来自更大屏幕的位置移回屏幕内。
        state.x = Some(3000.0);
        let (x, _) = display_position(&areas, &state);
        assert!(x + state.width <= 1920.0);
    }

    #[test]
    fn state_parsing_falls_back_to_defaults() {
        assert_eq!(TodoStickyState::parse(""), TodoStickyState::default());
        assert_eq!(
            TodoStickyState::parse("not json"),
            TodoStickyState::default()
        );
        let state = TodoStickyState::parse(
            r#"{"open":true,"x":10,"width":50,"pinned":true,"color":"teal"}"#,
        );
        assert!(state.open && state.pinned);
        // 不认识的颜色按默认颜色。
        assert_eq!(state.color, "");
        assert_eq!(TodoStickyState::parse(r#"{"color":"Blue"}"#).color, "blue");
        // 只有一个坐标的不算保存过位置；过小的尺寸按最小尺寸。
        assert_eq!((state.x, state.y), (None, None));
        assert_eq!(state.width, TODO_STICKY_MIN_WIDTH);
        assert_eq!(state.height, TODO_STICKY_HEIGHT);
    }
}
