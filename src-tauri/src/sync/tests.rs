use super::*;

fn temp_db(name: &str) -> (DbManager, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "taskreminder-sync-test-{}-{}",
        name,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    (DbManager::new(path.clone()).unwrap(), dir)
}

fn set_completed(path: &std::path::Path, id: &str, days_ago: i64) {
    let completed = time::format_datetime(&(time::now() - chrono::Duration::days(days_ago)));
    let conn = Connection::open(path).unwrap();
    conn.execute(
        "UPDATE tasks SET status = 'COMPLETED', completed_at = ?1, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![completed, id],
    )
    .unwrap();
}

fn copy_task(from: &std::path::Path, to: &std::path::Path, id: &str) {
    let conn = Connection::open(to).unwrap();
    let escaped = from.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("ATTACH DATABASE '{}' AS src", escaped))
        .unwrap();
    conn.execute(
        "INSERT INTO tasks SELECT * FROM src.tasks WHERE id = ?",
        [id],
    )
    .unwrap();
    conn.execute_batch("DETACH DATABASE src").unwrap();
}

fn deleted_at(path: &std::path::Path, id: &str) -> Option<Option<String>> {
    let conn = Connection::open(path).unwrap();
    conn.query_row("SELECT deleted_at FROM tasks WHERE id = ?", [id], |row| {
        row.get::<_, Option<String>>(0)
    })
    .ok()
}

#[test]
fn cleaned_up_completed_task_does_not_resurrect_after_merge() {
    let (local, local_dir) = temp_db("local");
    let (remote, remote_dir) = temp_db("remote");
    let task = local.create_task("old completed", None).unwrap();
    set_completed(&local.db_path(), &task.id, 40);
    // 远端（另一台设备上传的库）仍保留这条已完成任务。
    copy_task(&local.db_path(), &remote.db_path(), &task.id);

    local
        .cleanup_data(crate::db::TOMBSTONE_RETENTION_DAYS_SYNC)
        .unwrap();
    merge_databases(&local.db_path(), &remote.db_path()).unwrap();

    let merged = deleted_at(&local.db_path(), &task.id).expect("row exists");
    assert!(merged.is_some(), "tombstone should win over older live row");

    let _ = std::fs::remove_dir_all(local_dir);
    let _ = std::fs::remove_dir_all(remote_dir);
}

#[test]
fn purged_from_trash_does_not_come_back_after_merge() {
    let (local, local_dir) = temp_db("local");
    let (remote, remote_dir) = temp_db("remote");
    let task = local.create_task("trashed", None).unwrap();
    local.delete_task(&task.id).unwrap();
    // 远端保留同一条墓碑（删除已同步过）。
    copy_task(&local.db_path(), &remote.db_path(), &task.id);
    {
        let conn = Connection::open(remote.db_path()).unwrap();
        conn.execute(
            "UPDATE tasks SET updated_at = '2000-01-01T00:00:00' WHERE id = ?",
            [&task.id],
        )
        .unwrap();
    }

    local
        .expire_tombstones(crate::db::TrashTable::Tasks, std::slice::from_ref(&task.id))
        .unwrap();
    merge_databases(&local.db_path(), &remote.db_path()).unwrap();
    // 与 perform_sync 一致：合并后、上传前清理过期墓碑。
    local
        .purge_expired_tombstones(crate::db::TOMBSTONE_RETENTION_DAYS_SYNC)
        .unwrap();
    assert!(deleted_at(&local.db_path(), &task.id).is_none());

    let _ = std::fs::remove_dir_all(local_dir);
    let _ = std::fs::remove_dir_all(remote_dir);
}

#[test]
fn merge_keeps_newer_row() {
    let (local, local_dir) = temp_db("local");
    let (remote, remote_dir) = temp_db("remote");
    let task = local.create_task("shared", None).unwrap();
    copy_task(&local.db_path(), &remote.db_path(), &task.id);
    {
        let conn = Connection::open(remote.db_path()).unwrap();
        conn.execute(
            "UPDATE tasks SET description = 'remote edit', tags = '工作,周报', priority = 2, updated_at = '2999-01-01T00:00:00' WHERE id = ?",
            [&task.id],
        )
        .unwrap();
    }
    merge_databases(&local.db_path(), &remote.db_path()).unwrap();
    let merged = local.get_task(&task.id).unwrap().unwrap();
    assert_eq!(merged.description, "remote edit");
    assert_eq!(merged.tags, vec!["工作", "周报"]);
    assert_eq!(merged.priority, 2);

    let _ = std::fs::remove_dir_all(local_dir);
    let _ = std::fs::remove_dir_all(remote_dir);
}

fn exec(path: &std::path::Path, sql: &str) {
    Connection::open(path).unwrap().execute_batch(sql).unwrap();
}

fn text_column(path: &std::path::Path, table: &str, column: &str, id: &str) -> Option<String> {
    let conn = Connection::open(path).unwrap();
    conn.query_row(
        &format!("SELECT {} FROM {} WHERE id = ?", column, table),
        [id],
        |row| row.get(0),
    )
    .unwrap()
}

fn has_table_column(path: &std::path::Path, table: &str, column: &str) -> bool {
    let conn = Connection::open(path).unwrap();
    table_columns(&conn, table)
        .unwrap()
        .iter()
        .any(|c| c.name == column)
}

/// 模拟更新版本（v2.1）的设备：多一个同步列 `due_at`。
fn add_future_column(path: &std::path::Path) {
    exec(path, "ALTER TABLE tasks ADD COLUMN due_at TEXT");
}

#[test]
fn unknown_remote_column_is_adopted_with_its_values() {
    let (local, local_dir) = temp_db("local");
    let (remote, remote_dir) = temp_db("remote");
    let task = local.create_task("shared", None).unwrap();
    copy_task(&local.db_path(), &remote.db_path(), &task.id);
    add_future_column(&remote.db_path());
    exec(
        &remote.db_path(),
        &format!(
            "UPDATE tasks SET due_at = '2026-10-08T18:00:00', updated_at = '2999-01-01T00:00:00' WHERE id = '{}'",
            task.id
        ),
    );

    merge_databases(&local.db_path(), &remote.db_path()).unwrap();
    assert!(has_table_column(&local.db_path(), "tasks", "due_at"));
    assert_eq!(
        text_column(&local.db_path(), "tasks", "due_at", &task.id).as_deref(),
        Some("2026-10-08T18:00:00")
    );
    // 新列不影响本版本读取。
    assert_eq!(
        local.get_task(&task.id).unwrap().unwrap().description,
        "shared"
    );

    let _ = std::fs::remove_dir_all(local_dir);
    let _ = std::fs::remove_dir_all(remote_dir);
}

#[test]
fn local_edit_keeps_adopted_column_and_uploads_it() {
    let (local, local_dir) = temp_db("local");
    let (remote, remote_dir) = temp_db("remote");
    let task = local.create_task("shared", None).unwrap();
    copy_task(&local.db_path(), &remote.db_path(), &task.id);
    add_future_column(&remote.db_path());
    exec(
        &remote.db_path(),
        &format!(
            "UPDATE tasks SET due_at = '2026-10-08T18:00:00', updated_at = '2000-01-02T00:00:00' WHERE id = '{}'",
            task.id
        ),
    );
    merge_databases(&local.db_path(), &remote.db_path()).unwrap();

    // 本机（不认识 due_at 的版本）修改标题后再次同步：本机行胜出，due_at 仍在。
    local
        .update_task(&task.id, "local edit", None, None, None)
        .unwrap();
    merge_databases(&local.db_path(), &remote.db_path()).unwrap();
    assert_eq!(
        text_column(&local.db_path(), "tasks", "due_at", &task.id).as_deref(),
        Some("2026-10-08T18:00:00")
    );

    // 上传的快照由更新版本的设备合并：标题改动与 due_at 都在。
    let (newer, newer_dir) = temp_db("newer");
    add_future_column(&newer.db_path());
    let snapshot = export_local_snapshot_bytes(&local.db_path()).unwrap();
    merge_snapshot_bytes(&newer.db_path(), &snapshot).unwrap();
    assert_eq!(
        text_column(&newer.db_path(), "tasks", "description", &task.id).as_deref(),
        Some("local edit")
    );
    assert_eq!(
        text_column(&newer.db_path(), "tasks", "due_at", &task.id).as_deref(),
        Some("2026-10-08T18:00:00")
    );

    let _ = std::fs::remove_dir_all(local_dir);
    let _ = std::fs::remove_dir_all(remote_dir);
    let _ = std::fs::remove_dir_all(newer_dir);
}

#[test]
fn older_remote_without_column_keeps_local_value() {
    let (local, local_dir) = temp_db("local");
    let (remote, remote_dir) = temp_db("remote");
    let task = local.create_task("shared", None).unwrap();
    copy_task(&local.db_path(), &remote.db_path(), &task.id);
    // 本机是更新版本（有 due_at），远端来自不认识它的旧版本，并且旧版本改过这一行。
    add_future_column(&local.db_path());
    exec(
        &local.db_path(),
        &format!(
            "UPDATE tasks SET due_at = '2026-10-08T18:00:00' WHERE id = '{}'",
            task.id
        ),
    );
    exec(
        &remote.db_path(),
        &format!(
            "UPDATE tasks SET description = 'old device edit', updated_at = '2999-01-01T00:00:00' WHERE id = '{}'",
            task.id
        ),
    );

    merge_databases(&local.db_path(), &remote.db_path()).unwrap();
    assert_eq!(
        text_column(&local.db_path(), "tasks", "description", &task.id).as_deref(),
        Some("old device edit")
    );
    assert_eq!(
        text_column(&local.db_path(), "tasks", "due_at", &task.id).as_deref(),
        Some("2026-10-08T18:00:00")
    );

    let _ = std::fs::remove_dir_all(local_dir);
    let _ = std::fs::remove_dir_all(remote_dir);
}

#[test]
fn unaddable_remote_column_is_skipped_without_failing() {
    let (local, local_dir) = temp_db("local");
    let (remote, remote_dir) = temp_db("remote");
    let task = local.create_task("shared", None).unwrap();
    copy_task(&local.db_path(), &remote.db_path(), &task.id);
    // 远端表被重建为带 NOT NULL 且无默认值的新列：本机无法给已有行补值，只能跳过。
    exec(
        &remote.db_path(),
        "CREATE TABLE tasks_new AS SELECT *, 'x' AS strict_col FROM tasks;
         DROP TABLE tasks;
         CREATE TABLE tasks AS SELECT * FROM tasks_new WHERE 0;
         DROP TABLE tasks_new;",
    );
    {
        // 用完整定义重建，确保 strict_col 为 NOT NULL 无默认值。
        let conn = Connection::open(remote.db_path()).unwrap();
        conn.execute_batch("DROP TABLE tasks").unwrap();
        conn.execute_batch(
            "CREATE TABLE tasks (id TEXT PRIMARY KEY, description TEXT NOT NULL, type TEXT NOT NULL,
             status TEXT NOT NULL, created_at TEXT NOT NULL, completed_at TEXT, reminder_time TEXT,
             strict_col TEXT NOT NULL)",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, description, type, status, created_at, strict_col)
             VALUES ('remote-only', 'from remote', 'ONE_TIME', 'PENDING', '2026-01-01T00:00:00', 'v')",
            [],
        )
        .unwrap();
    }

    merge_databases(&local.db_path(), &remote.db_path()).unwrap();
    assert!(!has_table_column(&local.db_path(), "tasks", "strict_col"));
    assert!(local.get_task("remote-only").unwrap().is_some());
    assert!(local.get_task(&task.id).unwrap().is_some());

    let _ = std::fs::remove_dir_all(local_dir);
    let _ = std::fs::remove_dir_all(remote_dir);
}

#[test]
fn local_only_columns_are_not_synced() {
    let (local, local_dir) = temp_db("local");
    let (remote, remote_dir) = temp_db("remote");
    let task = local.create_task("shared", None).unwrap();
    let other = local.create_task("other", None).unwrap();
    copy_task(&local.db_path(), &remote.db_path(), &task.id);
    copy_task(&local.db_path(), &remote.db_path(), &other.id);
    local.set_sticky_note_pinned(&task.id, true).unwrap();
    exec(
        &remote.db_path(),
        &format!(
            "UPDATE tasks SET description = 'remote edit', sticky_is_pinned = 1,
                              updated_at = '2999-01-01T00:00:00' WHERE id = '{}';
             UPDATE tasks SET description = 'remote edit', sticky_is_pinned = 0,
                              updated_at = '2999-01-01T00:00:00' WHERE id = '{}';",
            other.id, task.id
        ),
    );
    merge_databases(&local.db_path(), &remote.db_path()).unwrap();
    // 远端行胜出：同步列取远端的值，锚定状态各自保留本机的值。
    assert_eq!(
        local.get_task(&task.id).unwrap().unwrap().description,
        "remote edit"
    );
    assert!(local.get_sticky_note_pinned(&task.id).unwrap());
    assert_eq!(
        local.get_task(&other.id).unwrap().unwrap().description,
        "remote edit"
    );
    assert!(!local.get_sticky_note_pinned(&other.id).unwrap());

    let _ = std::fs::remove_dir_all(local_dir);
    let _ = std::fs::remove_dir_all(remote_dir);
}

#[derive(Default)]
struct MemoryStore {
    files: std::cell::RefCell<HashMap<String, Vec<u8>>>,
}

impl MemoryStore {
    fn file(&self, name: &str) -> Option<Vec<u8>> {
        self.files.borrow().get(name).cloned()
    }
}

impl RemoteStore for MemoryStore {
    fn exists(&self, name: &str) -> Result<bool, AppError> {
        Ok(self.files.borrow().contains_key(name))
    }

    fn get(&self, name: &str) -> Result<Vec<u8>, AppError> {
        self.file(name)
            .ok_or_else(|| AppError::Sync("not found".to_string()))
    }

    fn put(&self, name: &str, data: Vec<u8>) -> Result<(), AppError> {
        self.files.borrow_mut().insert(name.to_string(), data);
        Ok(())
    }
}

#[test]
fn sync_state_parses_codes_and_legacy_text() {
    for state in [
        SyncState::Never,
        SyncState::Syncing,
        SyncState::Success,
        SyncState::FirstSync,
        SyncState::LockBusy,
        SyncState::Failed,
        SyncState::PassphraseMismatch,
    ] {
        assert_eq!(SyncState::parse(state.code()), Some(state));
    }
    assert_eq!(SyncState::parse("同步成功"), Some(SyncState::Success));
    assert_eq!(SyncState::parse("首次同步完成"), Some(SyncState::FirstSync));
    assert_eq!(SyncState::parse("同步失败"), Some(SyncState::Failed));
    assert_eq!(
        SyncState::parse("锁被占用，稍后重试"),
        Some(SyncState::LockBusy)
    );
    assert_eq!(SyncState::parse("未知"), None);

    assert_eq!(status_code(None), "never");
    assert_eq!(status_code(Some("同步成功")), "success");
    assert_eq!(status_code(Some("first_sync")), "first_sync");
    assert_eq!(status_code(Some("未知")), "未知");
}

#[test]
fn dirty_check_uses_status_codes_and_legacy_text() {
    let (db, dir) = temp_db("dirty");
    let mut settings = db.load_settings().unwrap();
    settings.webdav_last_local_change_time = Some("2026-09-27T10:00:00".to_string());
    settings.webdav_last_sync_time = Some("2026-09-27T11:00:00".to_string());

    for (status, dirty) in [
        ("success", false),
        ("first_sync", false),
        ("同步成功", false),
        ("首次同步完成", false),
        ("failed", true),
        ("syncing", true),
        ("lock_busy", true),
        ("同步失败", true),
    ] {
        settings.webdav_last_sync_status = Some(status.to_string());
        assert_eq!(compute_dirty_from_settings(&settings), dirty, "{status}");
    }

    // 成功同步之后又有本地修改：仍需同步。
    settings.webdav_last_sync_status = Some("success".to_string());
    settings.webdav_last_local_change_time = Some("2026-09-27T12:00:00".to_string());
    assert!(compute_dirty_from_settings(&settings));

    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn update_sync_status_stores_code() {
    let (db, dir) = temp_db("status-code");
    let settings = db
        .update_sync_status(SyncState::Success.code(), None)
        .unwrap();
    assert_eq!(settings.webdav_last_sync_status.as_deref(), Some("success"));
    assert!(!compute_dirty_from_settings(&settings));
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

fn settings_for(db: &DbManager, encrypted: bool, passphrase: &str) -> AppSettings {
    let mut settings = db.load_settings().unwrap();
    settings.webdav_password = "webdav-secret".to_string();
    settings.sync_encryption_enabled = encrypted;
    settings.sync_passphrase = passphrase.to_string();
    db.save_settings(&settings).unwrap();
    settings
}

fn sync(
    store: &MemoryStore,
    db: &DbManager,
    settings: &AppSettings,
) -> Result<RemoteSyncResult, AppError> {
    sync_with_remote(store, db, settings, crate::sync_crypto::TEST_PARAMS)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn task_titles(db: &DbManager) -> Vec<String> {
    let mut titles: Vec<String> = db
        .list_active_tasks()
        .unwrap()
        .into_iter()
        .map(|task| task.description)
        .collect();
    titles.sort();
    titles
}

const PASS: &str = "correct horse battery";

#[test]
fn plain_sync_uploads_scrubbed_snapshot_and_merges() {
    let (a, a_dir) = temp_db("plain-a");
    let (b, b_dir) = temp_db("plain-b");
    let store = MemoryStore::default();
    a.create_task("来自 A", None).unwrap();
    b.create_task("来自 B", None).unwrap();

    let settings_a = settings_for(&a, false, "");
    assert_eq!(
        sync(&store, &a, &settings_a).unwrap(),
        RemoteSyncResult::FirstUpload
    );
    let uploaded = store.file(REMOTE_DB_NAME).unwrap();
    assert!(uploaded.starts_with(SQLITE_MAGIC));
    // 上传的快照不含 WebDAV 密码。
    assert!(!contains(&uploaded, b"webdav-secret"));

    let settings_b = settings_for(&b, false, "");
    assert_eq!(
        sync(&store, &b, &settings_b).unwrap(),
        RemoteSyncResult::Merged
    );
    assert_eq!(task_titles(&b), vec!["来自 A", "来自 B"]);
    // 本机设置不受影响。
    assert_eq!(b.load_settings().unwrap().webdav_password, "webdav-secret");

    let _ = std::fs::remove_dir_all(a_dir);
    let _ = std::fs::remove_dir_all(b_dir);
}

#[test]
fn encrypted_sync_roundtrips_without_plaintext_on_server() {
    let (a, a_dir) = temp_db("enc-a");
    let (b, b_dir) = temp_db("enc-b");
    let store = MemoryStore::default();
    a.create_task("机密会议", None).unwrap();

    let settings_a = settings_for(&a, true, PASS);
    assert_eq!(
        sync(&store, &a, &settings_a).unwrap(),
        RemoteSyncResult::FirstUpload
    );
    let encrypted = store.file(REMOTE_ENC_NAME).unwrap();
    assert!(crate::sync_crypto::is_encrypted(&encrypted));
    assert!(!contains(&encrypted, "机密会议".as_bytes()));
    assert!(!contains(&encrypted, PASS.as_bytes()));
    assert!(store
        .file(REMOTE_DB_NAME)
        .unwrap()
        .starts_with(PLACEHOLDER_MARKER));

    let settings_b = settings_for(&b, true, PASS);
    assert_eq!(
        sync(&store, &b, &settings_b).unwrap(),
        RemoteSyncResult::Merged
    );
    assert_eq!(task_titles(&b), vec!["机密会议"]);

    let _ = std::fs::remove_dir_all(a_dir);
    let _ = std::fs::remove_dir_all(b_dir);
}

#[test]
fn enabling_encryption_migrates_plain_remote_and_blocks_plain_devices() {
    let (a, a_dir) = temp_db("mig-a");
    let (b, b_dir) = temp_db("mig-b");
    let store = MemoryStore::default();
    a.create_task("A 的待办", None).unwrap();
    b.create_task("B 的待办", None).unwrap();

    let plain_a = settings_for(&a, false, "");
    sync(&store, &a, &plain_a).unwrap();

    // B 开启加密：合并明文后改为上传加密文件，明文文件被占位说明替换。
    let encrypted_b = settings_for(&b, true, PASS);
    assert_eq!(
        sync(&store, &b, &encrypted_b).unwrap(),
        RemoteSyncResult::Merged
    );
    assert_eq!(task_titles(&b), vec!["A 的待办", "B 的待办"]);
    assert!(store.file(REMOTE_ENC_NAME).is_some());
    let placeholder = store.file(REMOTE_DB_NAME).unwrap();
    assert!(placeholder.starts_with(PLACEHOLDER_MARKER));

    // A 仍未开启加密：报错且不上传明文。
    let err = sync(&store, &a, &plain_a).unwrap_err().to_string();
    assert!(err.contains("端到端加密"), "{}", err);
    assert_eq!(store.file(REMOTE_DB_NAME).unwrap(), placeholder);

    // 旧版本会把占位说明当作数据库合并：必须失败，才不会继续上传明文。
    let placeholder_path = a_dir.join("placeholder.db");
    std::fs::write(&placeholder_path, &placeholder).unwrap();
    assert!(merge_databases(&a.db_path(), &placeholder_path).is_err());

    let _ = std::fs::remove_dir_all(a_dir);
    let _ = std::fs::remove_dir_all(b_dir);
}

#[test]
fn wrong_passphrase_aborts_without_overwriting_remote() {
    let (a, a_dir) = temp_db("wrong-a");
    let (b, b_dir) = temp_db("wrong-b");
    let store = MemoryStore::default();
    a.create_task("A 的待办", None).unwrap();
    b.create_task("B 的待办", None).unwrap();
    sync(&store, &a, &settings_for(&a, true, PASS)).unwrap();
    let before = store.file(REMOTE_ENC_NAME).unwrap();

    let wrong = settings_for(&b, true, "another passphrase");
    let err = sync(&store, &b, &wrong).unwrap_err();
    assert!(matches!(err, AppError::SyncPassphrase(_)), "{:?}", err);
    assert_eq!(failure_state(&err), SyncState::PassphraseMismatch);
    assert!(err.to_string().contains("同步密码错误"), "{}", err);
    assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), before);
    assert_eq!(task_titles(&b), vec!["B 的待办"]);

    // 密码太短直接拒绝。
    let short = settings_for(&b, true, "short");
    assert!(sync(&store, &b, &short).is_err());
    assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), before);

    let _ = std::fs::remove_dir_all(a_dir);
    let _ = std::fs::remove_dir_all(b_dir);
}

#[test]
fn placeholder_without_encrypted_file_counts_as_first_sync() {
    let (a, a_dir) = temp_db("ph-a");
    let store = MemoryStore::default();
    store.put(REMOTE_DB_NAME, placeholder_content()).unwrap();
    a.create_task("A 的待办", None).unwrap();

    assert_eq!(
        sync(&store, &a, &settings_for(&a, true, PASS)).unwrap(),
        RemoteSyncResult::FirstUpload
    );
    assert!(store.file(REMOTE_ENC_NAME).is_some());
    // 明文设备同样按首次同步处理，覆盖占位说明。
    let store = MemoryStore::default();
    store.put(REMOTE_DB_NAME, placeholder_content()).unwrap();
    assert_eq!(
        sync(&store, &a, &settings_for(&a, false, "")).unwrap(),
        RemoteSyncResult::FirstUpload
    );
    assert!(store
        .file(REMOTE_DB_NAME)
        .unwrap()
        .starts_with(SQLITE_MAGIC));
    // 其他格式的文件不会被当作数据库合并或覆盖。
    let store = MemoryStore::default();
    store.put(REMOTE_DB_NAME, b"garbage".to_vec()).unwrap();
    assert!(sync(&store, &a, &settings_for(&a, false, "")).is_err());
    assert_eq!(store.file(REMOTE_DB_NAME).unwrap(), b"garbage".to_vec());

    let _ = std::fs::remove_dir_all(a_dir);
}

#[test]
fn connection_test_checks_passphrase() {
    let (a, a_dir) = temp_db("check-a");
    let store = MemoryStore::default();
    let plain = settings_for(&a, false, "");
    assert!(check_encryption(&store, &plain).0);

    sync(&store, &a, &settings_for(&a, true, PASS)).unwrap();
    let (ok, message) = check_encryption(&store, &plain);
    assert!(!ok && message.contains("已加密"), "{}", message);
    let (ok, message) = check_encryption(&store, &settings_for(&a, true, PASS));
    assert!(ok && message.contains("密码正确"), "{}", message);
    let (ok, message) = check_encryption(&store, &settings_for(&a, true, "wrong passphrase"));
    assert!(!ok && message.contains("密码错误"), "{}", message);

    let _ = std::fs::remove_dir_all(a_dir);
}

const NEW_PASS: &str = "new staple passphrase";

fn change(
    store: &dyn RemoteStore,
    db: &DbManager,
    settings: &AppSettings,
    new: &str,
) -> Result<RemoteSyncResult, AppError> {
    change_passphrase_on_remote(store, db, settings, new, crate::sync_crypto::TEST_PARAMS)
}

/// 上传总是失败的远端，用来验证失败时本机仍保留旧密码。
struct FailingPutStore<'a>(&'a MemoryStore);

impl RemoteStore for FailingPutStore<'_> {
    fn exists(&self, name: &str) -> Result<bool, AppError> {
        self.0.exists(name)
    }

    fn get(&self, name: &str) -> Result<Vec<u8>, AppError> {
        self.0.get(name)
    }

    fn put(&self, _name: &str, _data: Vec<u8>) -> Result<(), AppError> {
        Err(AppError::Sync("upload failed".to_string()))
    }
}

#[test]
fn changing_passphrase_reencrypts_and_other_device_recovers() {
    let (a, a_dir) = temp_db("change-a");
    let (b, b_dir) = temp_db("change-b");
    let store = MemoryStore::default();
    a.create_task("A 的待办", None).unwrap();
    b.create_task("B 的待办", None).unwrap();
    sync(&store, &a, &settings_for(&a, true, PASS)).unwrap();
    sync(&store, &b, &settings_for(&b, true, PASS)).unwrap();

    // A 更换密码前又改了数据：先合并远端（B 的待办），再用新密码上传。
    a.create_task("A 更换前新增", None).unwrap();
    let settings_a = settings_for(&a, true, PASS);
    assert_eq!(
        change(&store, &a, &settings_a, NEW_PASS).unwrap(),
        RemoteSyncResult::Merged
    );
    assert_eq!(a.load_settings().unwrap().sync_passphrase, NEW_PASS);
    assert_eq!(
        task_titles(&a),
        vec!["A 更换前新增", "A 的待办", "B 的待办"]
    );
    let encrypted = store.file(REMOTE_ENC_NAME).unwrap();
    assert!(crate::sync_crypto::decrypt(&encrypted, NEW_PASS).is_ok());
    assert!(crate::sync_crypto::decrypt(&encrypted, PASS).is_err());
    assert!(!contains(&encrypted, NEW_PASS.as_bytes()));
    assert!(store
        .file(REMOTE_DB_NAME)
        .unwrap()
        .starts_with(PLACEHOLDER_MARKER));

    // B 仍用旧密码：得到“密码不匹配”，不覆盖远端。
    let err = sync(&store, &b, &settings_for(&b, true, PASS)).unwrap_err();
    assert_eq!(failure_state(&err), SyncState::PassphraseMismatch);
    assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), encrypted);

    // B 填入新密码后恢复同步。
    assert_eq!(
        sync(&store, &b, &settings_for(&b, true, NEW_PASS)).unwrap(),
        RemoteSyncResult::Merged
    );
    assert_eq!(
        task_titles(&b),
        vec!["A 更换前新增", "A 的待办", "B 的待办"]
    );

    let _ = std::fs::remove_dir_all(a_dir);
    let _ = std::fs::remove_dir_all(b_dir);
}

#[test]
fn passphrase_change_is_validated_before_touching_remote() {
    let (a, a_dir) = temp_db("change-check");
    let plain = settings_for(&a, false, "");
    assert!(validate_passphrase_change(&plain, "", NEW_PASS).is_err());
    let settings = settings_for(&a, true, PASS);
    for (current, new, expected) in [
        ("not the current one", NEW_PASS, "不正确"),
        (PASS, "short", "至少"),
        (PASS, PASS, "相同"),
    ] {
        let err = validate_passphrase_change(&settings, current, new)
            .unwrap_err()
            .to_string();
        assert!(err.contains(expected), "{} -> {}", new, err);
    }
    assert!(validate_passphrase_change(&settings, PASS, NEW_PASS).is_ok());
    let _ = std::fs::remove_dir_all(a_dir);
}

#[test]
fn passphrase_change_with_undecryptable_remote_keeps_everything() {
    let (a, a_dir) = temp_db("change-wrong-a");
    let (b, b_dir) = temp_db("change-wrong-b");
    let store = MemoryStore::default();
    // 云端已被另一台设备用别的密码加密（例如那台设备先改过密码）。
    b.create_task("B 的待办", None).unwrap();
    sync(&store, &b, &settings_for(&b, true, "some other passphrase")).unwrap();
    let before = store.file(REMOTE_ENC_NAME).unwrap();

    a.create_task("A 的待办", None).unwrap();
    let err = change(&store, &a, &settings_for(&a, true, PASS), NEW_PASS).unwrap_err();
    assert_eq!(failure_state(&err), SyncState::PassphraseMismatch);
    assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), before);
    assert_eq!(a.load_settings().unwrap().sync_passphrase, PASS);
    assert_eq!(task_titles(&a), vec!["A 的待办"]);

    let _ = std::fs::remove_dir_all(a_dir);
    let _ = std::fs::remove_dir_all(b_dir);
}

#[test]
fn failed_upload_keeps_local_passphrase() {
    let (a, a_dir) = temp_db("change-upload");
    let store = MemoryStore::default();
    a.create_task("A 的待办", None).unwrap();
    sync(&store, &a, &settings_for(&a, true, PASS)).unwrap();
    let before = store.file(REMOTE_ENC_NAME).unwrap();

    let failing = FailingPutStore(&store);
    assert!(change(&failing, &a, &settings_for(&a, true, PASS), NEW_PASS).is_err());
    assert_eq!(a.load_settings().unwrap().sync_passphrase, PASS);
    assert_eq!(store.file(REMOTE_ENC_NAME).unwrap(), before);
    // 仍可用旧密码正常同步。
    assert!(sync(&store, &a, &settings_for(&a, true, PASS)).is_ok());

    let _ = std::fs::remove_dir_all(a_dir);
}

#[test]
fn passphrase_change_on_empty_remote_uploads_with_new_passphrase() {
    let (a, a_dir) = temp_db("change-empty");
    let store = MemoryStore::default();
    a.create_task("A 的待办", None).unwrap();
    assert_eq!(
        change(&store, &a, &settings_for(&a, true, PASS), NEW_PASS).unwrap(),
        RemoteSyncResult::FirstUpload
    );
    let encrypted = store.file(REMOTE_ENC_NAME).unwrap();
    assert!(crate::sync_crypto::decrypt(&encrypted, NEW_PASS).is_ok());
    let _ = std::fs::remove_dir_all(a_dir);
}

#[test]
fn mismatch_pauses_auto_sync_until_passphrase_is_edited() {
    let (a, a_dir) = temp_db("pause");
    let mut previous = settings_for(&a, true, PASS);
    previous.webdav_enabled = true;
    previous.webdav_last_sync_status = Some("failed".to_string());
    assert!(!auto_sync_paused(&previous));

    previous.webdav_last_sync_status = Some(SyncState::PassphraseMismatch.code().to_string());
    assert!(auto_sync_paused(&previous));
    // 仍不匹配时，未改密码的保存（例如只改了同步频率）不会触发同步。
    assert!(!should_resync_after_passphrase_edit(&previous, &previous));

    let mut saved = previous.clone();
    saved.sync_passphrase = NEW_PASS.to_string();
    assert!(should_resync_after_passphrase_edit(&previous, &saved));
    saved.webdav_enabled = false;
    assert!(!should_resync_after_passphrase_edit(&previous, &saved));

    let _ = std::fs::remove_dir_all(a_dir);
}

// ---- 同步水位：长期未同步设备的删除复活 ----

/// 在固定时间执行一步操作。
fn at<T>(when: &str, step: impl FnOnce() -> T) -> T {
    let _now = time::fix_now(when);
    step()
}

fn plain_settings(db: &DbManager) -> AppSettings {
    let mut settings = settings_for(db, false, "");
    settings.webdav_url = "https://dav.example.com".to_string();
    settings.webdav_root_path = "/taskreminder".to_string();
    settings
}

fn all_task_titles(db: &DbManager) -> Vec<String> {
    let mut titles = task_titles(db);
    titles.extend(
        db.list_completed_tasks()
            .unwrap()
            .into_iter()
            .map(|task| format!("done:{}", task.description)),
    );
    titles.sort();
    titles
}

/// A、B 两台设备在 1 月 1 日同步过；A 在 1 月 2 日删除“旧待办”，之后 A 照常同步，
/// 3 月 15 日墓碑过了 60 天保留期被清理。B 直到 3 月 16 日才再次同步。
fn long_offline_scenario(
    store: &MemoryStore,
) -> (DbManager, DbManager, String, Vec<std::path::PathBuf>) {
    let (a, a_dir) = temp_db("a");
    let (b, b_dir) = temp_db("b");
    let sa = plain_settings(&a);
    let sb = plain_settings(&b);
    let old = at("2026-01-01T10:00", || {
        let old = a.create_task("旧待办", None).unwrap();
        a.create_task("保留的待办", None).unwrap();
        sync(store, &a, &sa).unwrap();
        sync(store, &b, &sb).unwrap();
        old
    });
    at("2026-01-02T10:00", || {
        a.delete_task(&old.id).unwrap();
        sync(store, &a, &sa).unwrap();
    });
    at("2026-03-15T10:00", || {
        sync(store, &a, &sa).unwrap();
    });
    assert!(deleted_at(&a.db_path(), &old.id).is_none(), "A 已清理墓碑");
    (a, b, old.id, vec![a_dir, b_dir])
}

#[test]
fn long_offline_device_does_not_resurrect_purged_rows() {
    let store = MemoryStore::default();
    let (a, b, old_id, dirs) = long_offline_scenario(&store);
    let sb = plain_settings(&b);
    let sa = plain_settings(&a);

    at("2026-03-16T10:00", || {
        sync(&store, &b, &sb).unwrap();
    });
    assert!(
        deleted_at(&b.db_path(), &old_id).is_none(),
        "B 删除本机的旧行"
    );
    assert_eq!(task_titles(&b), vec!["保留的待办"]);
    at("2026-03-17T10:00", || {
        sync(&store, &a, &sa).unwrap();
    });
    assert_eq!(task_titles(&a), vec!["保留的待办"], "旧行没有被重新上传");

    for dir in dirs {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn without_watermark_purged_rows_still_come_back() {
    // 对照：升级前（没有水位）的行为，保证上一个测试确实依赖水位。
    let store = MemoryStore::default();
    let (_a, b, old_id, dirs) = long_offline_scenario(&store);
    exec(
        &b.db_path(),
        "DELETE FROM sync_watermark; DELETE FROM sync_watermark_rows;",
    );
    at("2026-03-16T10:00", || {
        sync(&store, &b, &plain_settings(&b)).unwrap();
    });
    assert_eq!(deleted_at(&b.db_path(), &old_id), Some(None));

    for dir in dirs {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn missing_rows_are_kept_within_retention_or_on_another_remote() {
    let store = MemoryStore::default();
    let (b, b_dir) = temp_db("b");
    let (other, other_dir) = temp_db("other");
    let sb = plain_settings(&b);
    at("2026-01-01T10:00", || {
        b.create_task("我的待办", None).unwrap();
        sync(&store, &b, &sb).unwrap();
    });
    // 远端被另一份数据覆盖（例如用旧备份或另一台设备的首次上传）。
    let foreign = at("2026-01-05T10:00", || {
        other.create_task("别人的待办", None).unwrap();
        export_local_snapshot_bytes(&other.db_path()).unwrap()
    });
    store
        .files
        .borrow_mut()
        .insert(REMOTE_DB_NAME.to_string(), foreign.clone());

    // 1. 60 天内：远端缺行只可能是被重置，保留本机数据。
    at("2026-01-20T10:00", || {
        sync(&store, &b, &sb).unwrap();
    });
    assert_eq!(task_titles(&b), vec!["别人的待办", "我的待办"]);

    // 2. 超过 60 天但换了远端目录：水位属于旧远端，不据此删除。
    store
        .files
        .borrow_mut()
        .insert(REMOTE_DB_NAME.to_string(), foreign);
    let mut moved = sb.clone();
    moved.webdav_root_path = "/another".to_string();
    at("2026-05-01T10:00", || {
        sync(&store, &b, &moved).unwrap();
    });
    assert_eq!(task_titles(&b), vec!["别人的待办", "我的待办"]);

    let _ = std::fs::remove_dir_all(b_dir);
    let _ = std::fs::remove_dir_all(other_dir);
}

#[test]
fn rows_created_imported_or_edited_after_watermark_are_kept() {
    let store = MemoryStore::default();
    let (b, b_dir) = temp_db("b");
    let (empty, empty_dir) = temp_db("empty");
    let sb = plain_settings(&b);
    let (untouched, edited) = at("2026-01-01T10:00", || {
        let untouched = b.create_task("未改动", None).unwrap();
        let edited = b.create_task("之后改过", None).unwrap();
        sync(&store, &b, &sb).unwrap();
        (untouched, edited)
    });
    at("2026-02-01T10:00", || {
        // 水位之后：新建一条、修改一条、导入一条更早的旧行（不在上次上传的集合中）。
        b.create_task("之后新建", None).unwrap();
        b.update_task(&edited.id, "之后改过（新）", None, None, None)
            .unwrap();
        let mut imported = b.get_task(&untouched.id).unwrap().unwrap();
        imported.id = "imported-old-row".to_string();
        imported.description = "导入的旧行".to_string();
        imported.created_at = "2025-06-01T09:00:00".to_string();
        imported.updated_at = Some("2025-06-01T09:00:00".to_string());
        b.import_rows(&[imported], &[], &[]).unwrap();
    });
    // 远端此后只剩一份不含这些行的数据（其他设备删除并清理后）。
    let remote = at("2026-03-01T10:00", || {
        export_local_snapshot_bytes(&empty.db_path()).unwrap()
    });
    store
        .files
        .borrow_mut()
        .insert(REMOTE_DB_NAME.to_string(), remote);

    at("2026-03-10T10:00", || {
        sync(&store, &b, &sb).unwrap();
    });
    assert_eq!(
        all_task_titles(&b),
        vec!["之后改过（新）", "之后新建", "导入的旧行"],
        "只删除上次上传后没有改动过的“未改动”"
    );

    let _ = std::fs::remove_dir_all(b_dir);
    let _ = std::fs::remove_dir_all(empty_dir);
}

#[test]
fn watermark_is_saved_after_upload_and_scrubbed_from_snapshot() {
    let store = MemoryStore::default();
    let (b, b_dir) = temp_db("b");
    let sb = plain_settings(&b);
    at("2026-01-01T10:00", || {
        b.create_task("一条", None).unwrap();
        sync(&store, &b, &sb).unwrap();
    });
    let saved = watermark::load(&b.db_path()).unwrap().unwrap();
    assert_eq!(saved.remote, "https://dav.example.com/taskreminder");
    assert_eq!(
        saved.uploaded_at,
        time::parse_datetime_any("2026-01-01T10:00").unwrap()
    );
    assert!(!saved.applies(
        &saved.remote,
        time::parse_datetime_any("2026-02-28T10:00").unwrap()
    ));
    assert!(saved.applies(
        &saved.remote,
        time::parse_datetime_any("2026-03-02T10:00").unwrap()
    ));
    assert!(!saved.applies(
        "https://other",
        time::parse_datetime_any("2027-01-01T10:00").unwrap()
    ));

    // 上传的快照中不含水位。
    let remote_path = b_dir.join("remote.db");
    std::fs::write(&remote_path, store.file(REMOTE_DB_NAME).unwrap()).unwrap();
    let conn = Connection::open(&remote_path).unwrap();
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM sync_watermark_rows", [], |r| r.get(0))
        .unwrap();
    let marks: i64 = conn
        .query_row("SELECT COUNT(*) FROM sync_watermark", [], |r| r.get(0))
        .unwrap();
    assert_eq!((rows, marks), (0, 0));

    let _ = std::fs::remove_dir_all(b_dir);
}
