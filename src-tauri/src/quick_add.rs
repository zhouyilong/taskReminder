use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::errors::AppError;
use crate::models::AppSettings;

pub const QUICK_ADD_LABEL: &str = "quick-add";
const QUICK_ADD_WIDTH: f64 = 460.0;
const QUICK_ADD_HEIGHT: f64 = 220.0;

/// 按设置重新注册“快速添加”全局快捷键。
/// 先注销本应用注册过的全部快捷键，再按需注册新的；失败时返回可展示给用户的原因。
pub fn apply_shortcut(app: &AppHandle, settings: &AppSettings) -> Result<(), AppError> {
    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|e| AppError::System(format!("注销快捷键失败: {}", e)))?;
    if !settings.quick_add_enabled {
        return Ok(());
    }
    let accelerator = settings.quick_add_shortcut.trim();
    if accelerator.is_empty() {
        return Err(AppError::Invalid("快捷键不能为空".to_string()));
    }
    shortcuts
        .on_shortcut(accelerator, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(err) = show_window(app) {
                    eprintln!("[quick-add] 打开快速添加窗口失败: {}", err);
                }
            }
        })
        .map_err(|e| {
            AppError::Invalid(format!(
                "快捷键 {} 注册失败（格式无效或已被其他程序占用）: {}",
                accelerator, e
            ))
        })
}

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
