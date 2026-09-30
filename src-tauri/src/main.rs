#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod backup;
mod commands;
mod db;
mod errors;
mod holiday_update;
mod holidays;
mod kinds;
mod maintenance;
mod models;
mod notification_queue;
mod paths;
mod quick_add;
mod quiet_hours;
mod recurrence;
mod scheduler;
mod secrets;
mod shortcuts;
mod single_instance;
mod state;
mod sync;
mod sync_crypto;
mod time;
mod tray;
mod windows;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::commands::{data, notification, recurring, settings, sticky, system, tasks, trash};
use crate::db::DbManager;
use crate::errors::AppError;
use crate::notification_queue::NotificationQueue;
use crate::scheduler::ReminderScheduler;
use crate::single_instance::InstanceLock;
use crate::state::AppState;
use crate::sync::CloudSyncService;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(windows::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            tasks::list_active_tasks,
            tasks::list_completed_tasks,
            recurring::list_recurring_tasks,
            notification::list_reminder_records,
            tasks::create_task,
            tasks::update_task,
            tasks::complete_task,
            tasks::uncomplete_task,
            tasks::delete_task,
            recurring::create_recurring_task,
            recurring::update_recurring_task,
            recurring::pause_recurring_task,
            recurring::resume_recurring_task,
            recurring::delete_recurring_task,
            notification::delete_reminder_record,
            notification::delete_reminder_records,
            trash::list_trash,
            trash::restore_task,
            trash::restore_recurring_task,
            trash::purge_trash,
            recurring::preview_recurring_triggers,
            recurring::check_holiday_updates,
            settings::get_settings,
            settings::save_settings,
            sticky::list_sticky_note_summaries,
            sticky::show_sticky_note,
            sticky::get_sticky_note_by_window_label,
            sticky::get_sticky_note,
            sticky::open_sticky_note,
            sticky::create_sticky_note,
            sticky::save_sticky_note_content,
            sticky::update_sticky_note_title,
            sticky::update_sticky_note_reminder,
            sticky::move_sticky_note,
            sticky::close_sticky_note,
            sticky::close_sticky_note_by_window_label,
            sticky::set_sticky_note_pinned_by_window_label,
            sticky::get_sticky_note_pinned_by_window_label,
            settings::test_webdav,
            settings::sync_now,
            settings::change_sync_passphrase,
            settings::set_autostart,
            notification::ack_notification,
            notification::ack_all_notifications,
            notification::complete_notification,
            tasks::quick_add_task,
            settings::apply_quick_add_shortcut,
            recurring::get_holiday_years,
            notification::snooze_notification,
            settings::get_sync_status,
            notification::get_notification_queue,
            system::get_current_theme,
            system::get_debug_info,
            system::is_dev_mode,
            system::emit_ui_state_changed,
            system::get_ui_state,
            data::export_data,
            data::import_data,
            data::list_backups,
            data::create_backup_now,
            data::restore_backup
        ])
        .setup(|app| {
            let result: Result<(), AppError> = (|| {
                let app_handle = app.handle();
                let data_dir = paths::resolve_data_dir(app_handle)?;
                let lock_path = paths::lock_path(&data_dir);
                let dev_mode = paths::is_dev_mode();
                let dialog_title = if dev_mode {
                    "任务提醒 [开发]"
                } else {
                    "任务提醒"
                };
                match InstanceLock::try_lock(&lock_path)? {
                    Some(lock) => {
                        app.manage(lock);
                    }
                    None => {
                        eprintln!("{}: 应用已经在运行", dialog_title);
                        app_handle.exit(0);
                        return Ok(());
                    }
                }

                tray::setup_tray(app_handle).map_err(|e| AppError::System(e.to_string()))?;

                if autostart::launched_from_autostart() {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.minimize();
                        let _ = window.hide();
                    }
                }

                // 开发模式：窗口标题加 [开发] 标识
                if dev_mode {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.set_title("任务提醒应用 [开发]");
                    }
                }

                holidays::load(&data_dir);
                let db_path = paths::db_path(&data_dir);
                let is_first_launch = !db_path.exists();
                let db = DbManager::new(db_path)?;
                let sync = CloudSyncService::new(app_handle.clone(), db.clone());
                let scheduler = ReminderScheduler::new(
                    app_handle.clone(),
                    db.clone(),
                    sync.clone(),
                    NotificationQueue::new(),
                );
                scheduler.schedule_existing()?;
                scheduler.start_watchdog();
                sync.start()?;
                maintenance::start_maintenance(db.clone());

                let mut settings = db.load_settings()?;
                let autostart_enabled = autostart::is_autostart_enabled().unwrap_or(false);
                let autostart_current =
                    autostart::is_autostart_configuration_current().unwrap_or(false);
                if !dev_mode && is_first_launch && !autostart_enabled {
                    settings.auto_start_enabled = true;
                    let _ = db.save_settings(&settings);
                    let _ = autostart::enable_autostart();
                } else {
                    if settings.auto_start_enabled != autostart_enabled {
                        settings.auto_start_enabled = autostart_enabled;
                        let _ = db.save_settings(&settings);
                    }
                    if settings.auto_start_enabled && !autostart_current {
                        let _ = autostart::enable_autostart();
                    }
                }
                let db_for_restore = db.clone();

                let state = AppState {
                    db,
                    scheduler,
                    sync,
                    ui_state: Arc::new(Mutex::new(None)),
                };
                app.manage(state);
                tray::request_refresh(app_handle);
                // 快捷键被占用等失败不应阻止应用启动，设置界面可重新应用并查看原因。
                if let Ok(settings) = db_for_restore.load_settings() {
                    if let Err(err) = shortcuts::apply_shortcuts(app_handle, &settings) {
                        eprintln!("[shortcuts] {}", err);
                    }
                }
                windows::sticky::restore_open_sticky_note_items(app_handle, &db_for_restore)?;
                holiday_update::start(app_handle.clone(), data_dir.clone());
                Ok(())
            })();
            result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}
