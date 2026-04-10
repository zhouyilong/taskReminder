use std::sync::{Arc, Mutex};

use crate::db::DbManager;
use crate::models::{NotificationPayload, UiStatePayload};
use crate::scheduler::ReminderScheduler;
use crate::sync::CloudSyncService;

#[derive(Clone)]
pub struct AppState {
    pub db: DbManager,
    pub scheduler: ReminderScheduler,
    pub sync: CloudSyncService,
    pub notification_snapshot: Arc<Mutex<Option<NotificationPayload>>>,
    pub ui_state: Arc<Mutex<Option<UiStatePayload>>>,
}
