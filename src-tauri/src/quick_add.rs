use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::errors::AppError;

pub const QUICK_ADD_LABEL: &str = "quick-add";
const QUICK_ADD_WIDTH: f64 = 460.0;
const QUICK_ADD_HEIGHT: f64 = 220.0;

/// 显示快速添加窗口（不存在则创建），置顶并聚焦。
pub fn show_window(app: &AppHandle) -> Result<(), AppError> {
    let window = if let Some(existing) = app.get_webview_window(QUICK_ADD_LABEL) {
        existing
    } else {
        WebviewWindowBuilder::new(
            app,
            QUICK_ADD_LABEL,
            WebviewUrl::App("quick-add.html".into()),
        )
        .title("快速添加待办")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .inner_size(QUICK_ADD_WIDTH, QUICK_ADD_HEIGHT)
        .center()
        .build()
        .map_err(|e| AppError::System(e.to_string()))?
    };
    let _ = window.center();
    window.show().map_err(|e| AppError::System(e.to_string()))?;
    let _ = window.set_focus();
    // 让已打开的窗口清空上一次的输入并重新聚焦输入框。
    let _ = app.emit_to(QUICK_ADD_LABEL, "quick-add-reset", ());
    Ok(())
}
