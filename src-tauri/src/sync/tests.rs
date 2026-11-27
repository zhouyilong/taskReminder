use std::path::PathBuf;

use rusqlite::Connection;
use uuid::Uuid;

use super::merge_databases;
use crate::db::DbManager;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("taskreminder-merge-{}-{}", tag, Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn merge_databases_picks_newer_lww_and_preserves_tombstones() {
    let dir = temp_dir("lww");
    let local_path = dir.join("local.db");
    let remote_path = dir.join("remote.db");
    let local_db = DbManager::new(local_path.clone()).unwrap();
    let remote_db = DbManager::new(remote_path.clone()).unwrap();

    // 本地新建一条任务（较旧）
    let conn_l = Connection::open(&local_path).unwrap();
    conn_l
        .execute(
            "INSERT INTO tasks (id, description, type, status, created_at, updated_at)
             VALUES ('t1', '旧标题', 'ONE_TIME', 'PENDING', '2026-11-27T08:00:00', '2026-11-27T09:00:00')",
            [],
        )
        .unwrap();
    conn_l
        .execute(
            "INSERT INTO tasks (id, description, type, status, created_at, updated_at)
             VALUES ('t-local-only', '本地独有', 'ONE_TIME', 'PENDING', '2026-11-27T08:00:00', '2026-11-27T08:00:00')",
            [],
        )
        .unwrap();
    drop(conn_l);

    // 远端对 t1 做了较新的软删除（墓碑），并新增一条远端独有任务
    let conn_r = Connection::open(&remote_path).unwrap();
    conn_r
        .execute(
            "INSERT INTO tasks (id, description, type, status, created_at, updated_at, deleted_at)
             VALUES ('t1', '旧标题', 'ONE_TIME', 'PENDING', '2026-11-27T08:00:00', '2026-11-27T10:00:00', '2026-11-27T10:00:00')",
            [],
        )
        .unwrap();
    conn_r
        .execute(
            "INSERT INTO tasks (id, description, type, status, created_at, updated_at)
             VALUES ('t-remote-only', '远端独有', 'ONE_TIME', 'PENDING', '2026-11-27T08:30:00', '2026-11-27T08:30:00')",
            [],
        )
        .unwrap();
    drop(conn_r);

    merge_databases(&local_path, &remote_path).unwrap();

    let active = local_db.list_active_tasks().unwrap();
    let active_ids: Vec<&str> = active.iter().map(|t| t.id.as_str()).collect();
    assert!(active_ids.contains(&"t-local-only"));
    assert!(active_ids.contains(&"t-remote-only"));
    assert!(!active_ids.contains(&"t1"), "较新的墓碑应当胜出，不得复活");

    drop(local_db);
    drop(remote_db);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn dual_db_hot_switch_preserves_device_settings_and_isolates_business_data() {
    let dir = temp_dir("dual-db");
    let local_path = dir.join("taskreminder.db");
    let db = DbManager::new(local_path).unwrap();

    let mut s = db.load_settings().unwrap();
    s.snooze_minutes = 15;
    db.save_settings(&s).unwrap();
    let local_task = db.create_task("本地任务", None).unwrap();

    // 切换到云端账号镜像库
    db.switch_to_account("user-alice").unwrap();
    assert!(db.is_cloud_active());
    assert_eq!(db.list_active_tasks().unwrap().len(), 0);
    // 设备级设置保持不变
    assert_eq!(db.load_settings().unwrap().snooze_minutes, 15);

    // 在云端库写入数据应记入 pending_mutations
    let cloud_task = db.create_task("云端任务", None).unwrap();
    assert_eq!(db.pending_mutation_count().unwrap(), 1);
    assert_eq!(db.list_active_tasks().unwrap()[0].id, cloud_task.id);

    // 切回本地库，原本地任务毫发无损
    db.switch_to_local();
    assert!(!db.is_cloud_active());
    let locals = db.list_active_tasks().unwrap();
    assert_eq!(locals.len(), 1);
    assert_eq!(locals[0].id, local_task.id);

    let _ = std::fs::remove_dir_all(dir);
}
