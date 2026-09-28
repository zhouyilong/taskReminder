//! 系统与界面状态：主题、UI 状态广播、开发模式与调试信息。

use chrono::Local;
use serde::Serialize;
use tauri::State;

use crate::commands::{into_api, ApiResult};
use crate::models::UiStatePayload;
use crate::paths;
use crate::state::AppState;
use crate::windows::sticky::emit_ui_state_to_sticky_windows;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugInfo {
    db_path: String,
    db_size: u64,
    db_last_modified: Option<String>,
    active_tasks: usize,
    completed_tasks: usize,
    recurring_tasks: usize,
    reminder_records: usize,
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
    let color = unsafe { GetSysColor(COLOR_WINDOW) };
    let r = (color & 0x0000_00FF) as f32;
    let g = ((color & 0x0000_FF00) >> 8) as f32;
    let b = ((color & 0x00FF_0000) >> 16) as f32;
    let luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
    Some(if luminance < 0.5 { "dark" } else { "light" }.to_string())
}

#[tauri::command]
pub fn is_dev_mode() -> bool {
    paths::is_dev_mode()
}

/// 向所有窗口广播 UI 状态变化（缩放、主题、透明度）
/// 使用 Rust emit 确保 dev 模式下外部 webview 也能接收
#[tauri::command]
pub fn emit_ui_state_changed(
    app: tauri::AppHandle,
    state: State<AppState>,
    ui_scale: f64,
    theme: String,
    window_opacity: f64,
) -> ApiResult<()> {
    let payload = UiStatePayload {
        ui_scale,
        theme,
        window_opacity,
    };
    let mut snapshot = state.ui_state.lock().map_err(|e| e.to_string())?;
    *snapshot = Some(payload.clone());
    drop(snapshot);
    emit_ui_state_to_sticky_windows(&app, &payload);
    Ok(())
}

#[tauri::command]
pub fn get_ui_state(state: State<AppState>) -> ApiResult<Option<UiStatePayload>> {
    let snapshot = state.ui_state.lock().map_err(|e| e.to_string())?;
    Ok(snapshot.clone())
}

#[tauri::command]
pub fn get_current_theme(window: tauri::Window) -> ApiResult<String> {
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
pub fn get_debug_info(state: State<AppState>) -> ApiResult<DebugInfo> {
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
