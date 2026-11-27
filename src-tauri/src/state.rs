use std::sync::{Arc, Mutex};

use crate::cloud::CloudSyncService;
use crate::db::DbManager;
use crate::models::UiStatePayload;
use crate::scheduler::ReminderScheduler;

#[derive(Clone)]
pub struct AppState {
    pub db: DbManager,
    pub scheduler: ReminderScheduler,
    pub sync: CloudSyncService,
    pub ui_state: Arc<Mutex<Option<UiStatePayload>>>,
}
