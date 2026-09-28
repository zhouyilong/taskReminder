//! 全局快捷键：快速添加、显示/隐藏全部便签。

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::errors::AppError;
use crate::models::AppSettings;
use crate::quick_add;
use crate::windows::sticky;

/// 按设置重新注册全部全局快捷键：先注销本应用注册过的快捷键，再逐个注册。
/// 某个快捷键失败不影响其他快捷键，失败原因合并后返回给设置界面展示。
pub fn apply_shortcuts(app: &AppHandle, settings: &AppSettings) -> Result<(), AppError> {
    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|e| AppError::System(format!("注销快捷键失败: {}", e)))?;

    let mut errors = Vec::new();
    let quick_add = settings.quick_add_shortcut.trim();
    if settings.quick_add_enabled {
        if quick_add.is_empty() {
            errors.push("快速添加快捷键不能为空".to_string());
        } else if let Err(err) = shortcuts.on_shortcut(quick_add, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(err) = quick_add::show_window(app) {
                    eprintln!("[quick-add] 打开快速添加窗口失败: {}", err);
                }
            }
        }) {
            errors.push(register_error("快速添加", quick_add, err));
        }
    }

    let sticky_toggle = settings.sticky_toggle_shortcut.trim();
    if !sticky_toggle.is_empty() {
        if settings.quick_add_enabled && sticky_toggle.eq_ignore_ascii_case(quick_add) {
            errors.push("显示/隐藏便签的快捷键与快速添加相同".to_string());
        } else if let Err(err) = shortcuts.on_shortcut(sticky_toggle, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                sticky::toggle_all_sticky_windows(app);
            }
        }) {
            errors.push(register_error("显示/隐藏便签", sticky_toggle, err));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::Invalid(errors.join("；")))
    }
}

fn register_error(name: &str, accelerator: &str, err: impl std::fmt::Display) -> String {
    format!(
        "{}快捷键 {} 注册失败（格式无效或已被其他程序占用）: {}",
        name, accelerator, err
    )
}
