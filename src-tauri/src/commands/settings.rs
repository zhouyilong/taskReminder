//! 设置、云同步、开机自启与全局快捷键。

use tauri::{Emitter, State};

use crate::commands::{into_api, ApiResult};
use crate::models::{AppSettings, SyncStatus};
use crate::state::AppState;
use crate::{autostart, quick_add, sync};

const STICKY_NOTE_MIN_OPACITY: f64 = 0.35;
const STICKY_NOTE_MAX_OPACITY: f64 = 1.0;
const STICKY_NOTE_DEFAULT_OPACITY: f64 = 0.95;
const WINDOW_MIN_OPACITY: f64 = 0.3;
const WINDOW_MAX_OPACITY: f64 = 1.0;
const WINDOW_DEFAULT_OPACITY: f64 = 1.0;

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> ApiResult<AppSettings> {
    let mut settings = into_api(state.db.load_settings())?;
    if let Ok(enabled) = autostart::is_autostart_enabled() {
        if settings.auto_start_enabled != enabled {
            settings.auto_start_enabled = enabled;
            let _ = state.db.save_settings(&settings);
        }
    }
    Ok(settings)
}

#[tauri::command]
pub fn save_settings(
    app: tauri::AppHandle,
    state: State<AppState>,
    settings: AppSettings,
) -> ApiResult<()> {
    into_api(state.db.save_settings(&settings))?;
    let mut sanitized_settings = settings.clone();
    sanitized_settings.sticky_note_opacity =
        normalize_sticky_note_opacity(sanitized_settings.sticky_note_opacity);
    sanitized_settings.window_opacity = normalize_window_opacity(sanitized_settings.window_opacity);
    // 广播给便签等窗口的设置不需要携带密码。
    sanitized_settings.webdav_password.clear();
    sanitized_settings.sync_passphrase.clear();
    let _ = app.emit("sticky-note-settings-updated", sanitized_settings.clone());
    let _ = app.emit("settings-updated", sanitized_settings);
    into_api(state.sync.update_settings())?;
    into_api(state.sync.notify_local_change())?;
    // 关闭或调整勿扰时段后，立即弹出积压的提醒。
    into_api(state.scheduler.release_quiet_hold())?;
    Ok(())
}

/// 按当前设置重新注册快速添加快捷键，失败时返回原因供设置界面展示。
#[tauri::command]
pub fn apply_quick_add_shortcut(app: tauri::AppHandle, state: State<AppState>) -> ApiResult<()> {
    let settings = into_api(state.db.load_settings())?;
    into_api(quick_add::apply_shortcut(&app, &settings))
}

#[tauri::command]
pub fn test_webdav(settings: AppSettings) -> ApiResult<WebDavTestResult> {
    match sync::test_webdav(&settings) {
        Ok((ok, message)) => Ok(WebDavTestResult { ok, message }),
        Err(err) => Ok(WebDavTestResult {
            ok: false,
            message: err.to_string(),
        }),
    }
}

#[derive(serde::Serialize)]
pub struct WebDavTestResult {
    ok: bool,
    message: String,
}

#[tauri::command]
pub fn sync_now(state: State<AppState>, reason: String) -> ApiResult<()> {
    into_api(state.sync.request_sync(&reason))
}

#[tauri::command]
pub fn set_autostart(state: State<AppState>, enabled: bool) -> ApiResult<()> {
    if enabled {
        into_api(autostart::enable_autostart())?;
    } else {
        into_api(autostart::disable_autostart())?;
    }
    let mut settings = into_api(state.db.load_settings())?;
    settings.auto_start_enabled = enabled;
    into_api(state.db.save_settings(&settings))?;
    Ok(())
}

#[tauri::command]
pub fn get_sync_status(state: State<AppState>) -> ApiResult<SyncStatus> {
    into_api(state.sync.get_status())
}

fn normalize_sticky_note_opacity(value: f64) -> f64 {
    if !value.is_finite() {
        return STICKY_NOTE_DEFAULT_OPACITY;
    }
    value.clamp(STICKY_NOTE_MIN_OPACITY, STICKY_NOTE_MAX_OPACITY)
}

fn normalize_window_opacity(value: f64) -> f64 {
    if !value.is_finite() {
        return WINDOW_DEFAULT_OPACITY;
    }
    value.clamp(WINDOW_MIN_OPACITY, WINDOW_MAX_OPACITY)
}
