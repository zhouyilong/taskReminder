use crate::backup;
use crate::db::{tombstone_retention_days, DbManager};
use crate::models::DEFAULT_COMPLETED_RETENTION_DAYS;
use crate::time;

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
            let completed_retention = db_cleanup
                .completed_retention_days()
                .unwrap_or(DEFAULT_COMPLETED_RETENTION_DAYS);
            let _ = db_cleanup
                .cleanup_data(tombstone_retention_days(sync_enabled), completed_retention);
        }
    });

    // 每天一份本地快照：启动 1 分钟后检查一次，之后每小时检查（跨天或长时间运行时补上）。
    let db_backup = db.clone();
    tauri::async_runtime::spawn(async move {
        sleep(std::time::Duration::from_secs(60)).await;
        loop {
            if let Err(err) = backup::ensure_daily_backup(&db_backup.db_path(), &time::now()) {
                eprintln!("[backup] 自动备份失败: {}", err);
            }
            sleep(std::time::Duration::from_secs(3600)).await;
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
