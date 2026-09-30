//! 导入导出与本地备份。

use serde::Serialize;
use tauri::{Manager, State};

use crate::commands::{into_api, ApiResult};
use crate::errors::AppError;
use crate::state::AppState;
use crate::time;
use crate::{backup, sync};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    path: String,
    /// 导出日历时无法用日历规则表达而跳过的循环提醒数量。
    skipped: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupListPayload {
    dir: String,
    backups: Vec<backup::BackupInfo>,
}

fn file_dialog(app: &tauri::AppHandle) -> tauri_plugin_dialog::FileDialogBuilder<tauri::Wry> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().file();
    match app.get_webview_window("main") {
        Some(window) => dialog.set_parent(&window),
        None => dialog,
    }
}

/// 导入或恢复后：重新计算调度并通知同步与界面。
fn after_bulk_change(state: &AppState) -> Result<(), AppError> {
    state.scheduler.schedule_existing()?;
    state.sync.notify_local_change()
}

/// 弹出保存对话框并导出；用户取消时返回 None。对话框需要在非主线程阻塞等待，因此是 async 命令。
#[tauri::command]
pub async fn export_data(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    format: String,
) -> ApiResult<Option<ExportResult>> {
    let format = into_api(backup::ExportFormat::parse(&format))?;
    let now = time::now();
    let Some(file) = file_dialog(&app)
        .set_title("导出数据")
        .add_filter(format.filter_name(), &[format.extension()])
        .set_file_name(format.default_file_name(&now))
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let data = into_api(backup::build_backup(
        &state.db,
        &app.package_info().version.to_string(),
    ))?;
    let (content, skipped) = match format {
        backup::ExportFormat::Json => (into_api(backup::render_json(&data))?, 0),
        backup::ExportFormat::Markdown => (backup::render_markdown(&data), 0),
        backup::ExportFormat::Ics => {
            let dtstamp = time::ics_utc_now();
            backup::render_ics(&data, &dtstamp)
        }
    };
    std::fs::write(&path, content).map_err(|e| format!("写入文件失败：{}", e))?;
    Ok(Some(ExportResult {
        path: path.to_string_lossy().to_string(),
        skipped,
    }))
}

/// 选择 JSON 备份并按 id 合并导入；用户取消时返回 None。
#[tauri::command]
pub async fn import_data(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> ApiResult<Option<backup::ImportSummary>> {
    let Some(file) = file_dialog(&app)
        .set_title("导入备份")
        .add_filter("JSON 备份", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取文件失败：{}", e))?;
    let data = into_api(backup::parse_backup(&content))?;
    // 导入前先留一份本地快照，出问题时可以从备份恢复。
    into_api(backup::create_backup(&state.db.db_path(), &time::now()))?;
    let summary = into_api(backup::import_backup(&state.db, &data))?;
    into_api(after_bulk_change(&state))?;
    Ok(Some(summary))
}

#[tauri::command]
pub fn list_backups(state: State<AppState>) -> ApiResult<BackupListPayload> {
    let dir = backup::backup_dir(&state.db.db_path());
    Ok(BackupListPayload {
        dir: dir.to_string_lossy().to_string(),
        backups: backup::list_backups(&dir),
    })
}

#[tauri::command]
pub fn create_backup_now(state: State<AppState>) -> ApiResult<()> {
    into_api(backup::create_backup(&state.db.db_path(), &time::now()))?;
    Ok(())
}

/// 把本地备份按“较新的版本胜出”合并回当前数据库：找回丢失或被误删清理的数据，不会覆盖之后的修改。
#[tauri::command]
pub fn restore_backup(state: State<AppState>, name: String) -> ApiResult<()> {
    let db_path = state.db.db_path();
    let source = into_api(backup::resolve_backup(&db_path, &name))?;
    // 先复制一份再合并，避免合并时改动备份文件（补列等）。
    let temp =
        std::env::temp_dir().join(format!("taskreminder-restore-{}.db", uuid::Uuid::new_v4()));
    std::fs::copy(&source, &temp).map_err(|e| format!("读取备份失败：{}", e))?;
    let result = sync::merge_databases(&db_path, &temp);
    let _ = std::fs::remove_file(&temp);
    into_api(result)?;
    into_api(after_bulk_change(&state))
}
