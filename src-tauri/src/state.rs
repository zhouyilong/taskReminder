use std::sync::{Arc, Mutex};

use tauri::AppHandle;

use crate::db::DbManager;
use crate::models::NotificationPayload;
use crate::scheduler::ReminderScheduler;
use crate::sync::CloudSyncService;

#[derive(Clone)]
pub struct AppState {
    pub db: DbManager,
    pub scheduler: ReminderScheduler,
    pub sync: CloudSyncService,
    pub notification_snapshot: Arc<Mutex<Option<NotificationPayload>>>,
    pub app_handle: AppHandle,
}
