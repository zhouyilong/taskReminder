use crate::db::DbManager;

use tokio::time::sleep;

pub fn start_maintenance(db: DbManager) {
    let db_cleanup = db.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            sleep(std::time::Duration::from_secs(3600)).await;
            let _ = db_cleanup.cleanup_data();
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
