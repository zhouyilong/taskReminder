//! 窗口管理与窗口事件。

pub mod sticky;

use tauri::{Window, WindowEvent};

/// 主窗口关闭时隐藏到托盘；便签窗口的事件交给 `sticky` 处理。
pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    if window.label() == "main" {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window.hide();
        }
    }
    if window.label().starts_with(sticky::STICKY_NOTE_ITEM_PREFIX) {
        sticky::handle_window_event(window, event);
    }
}
