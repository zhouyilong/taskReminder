use crate::db::{tombstone_retention_days, DbManager};

use tokio::time::sleep;

pub fn start_maintenance(db: DbManager) {
    let db_cleanup = db.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            sleep(std::time::Duration::from_secs(3600)).await;
            let sync_enabled = db_cleanup
                .load_settings()
                .map(|settings| settings.webdav_enabled)
                .unwrap_or(true);
            let _ = db_cleanup.cleanup_data(tombstone_retention_days(sync_enabled));
        }
    });

    let db_optimize = db.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            sleep(std::time::Duration::from_secs(6 * 3600)).await;
            let _ = db_optimize.optimize_database();
        }
    });
}
