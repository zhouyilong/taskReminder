//! 跨版本同步测试：按 2.0.0 / 2.0.1 / 2.0.2 发布时的迁移建库并写入示例数据
//! （`tests/fixtures/sync-sample-2.0.sql`），检查与当前版本双向合并、从旧版本升级都不丢数据；
//! 再用“模拟 v2.1”（多同步列、多一种循环模式）的库检查不认识的列与值原样保留。
//!
//! 发布新版本时：把它的库结构版本加到 `RELEASES`；若新增了同步列，为它另写一份夹具。

use std::path::{Path, PathBuf};

use rusqlite::{types::Value, Connection};

use super::{export_local_snapshot_bytes, merge_databases, merge_snapshot_bytes};
use crate::db::{create_schema_up_to, DbManager};
use crate::kinds::RepeatMode;
use crate::sync_schema::{table_columns, SYNC_TABLES};

/// （应用版本，发布时的库结构版本）。
const RELEASES: &[(&str, &str)] = &[("2.0.0", "2.0.1"), ("2.0.1", "2.0.3"), ("2.0.2", "2.0.5")];
const SAMPLE: &str = include_str!("../../tests/fixtures/sync-sample-2.0.sql");

struct TempDir(PathBuf);

impl TempDir {
    fn new(kind: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "taskreminder-compat-{}-{}",
            kind,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 建出某个发布版本的库并写入示例数据（不经过当前版本的代码）。
fn release_db(path: &Path, schema_version: &str) {
    let conn = Connection::open(path).unwrap();
    create_schema_up_to(&conn, schema_version).unwrap();
    conn.execute_batch(SAMPLE).unwrap();
}

/// 当前版本的库，含一条本机新建的待办。
fn current_db(path: &Path) -> (DbManager, String) {
    let db = DbManager::new(path.to_path_buf()).unwrap();
    let task = db.create_task("当前版本新建", None).unwrap();
    (db, task.id)
}

type Rows = Vec<(String, Vec<(String, Value)>)>;

/// 读出一张表中指定 id 的行（只取参与同步的列），用于比较合并前后是否一致。
fn sync_rows(path: &Path, table: &str, ids: &[&str]) -> Rows {
    let spec = SYNC_TABLES.iter().find(|t| t.name == table).unwrap();
    let conn = Connection::open(path).unwrap();
    let columns: Vec<&str> = spec
        .columns
        .iter()
        .map(|c| c.name)
        .filter(|name| *name != "updated_at")
        .collect();
    ids.iter()
        .map(|id| {
            let values = conn
                .query_row(
                    &format!("SELECT {} FROM {} WHERE id = ?", columns.join(", "), table),
                    [id],
                    |row| {
                        (0..columns.len())
                            .map(|i| row.get::<_, Value>(i))
                            .collect::<Result<Vec<_>, _>>()
                    },
                )
                .unwrap_or_else(|e| panic!("{}.{} 缺失: {}", table, id, e));
            (
                id.to_string(),
                columns.iter().map(|c| c.to_string()).zip(values).collect(),
            )
        })
        .collect()
}

const TASK_IDS: &[&str] = &["old-task-pending", "old-task-done", "old-task-deleted"];
const RECURRING_IDS: &[&str] = &["old-rec-workday", "old-rec-weekly", "old-rec-future-mode"];
const RECORD_IDS: &[&str] = &["old-record-1", "old-record-2"];

fn all_rows(path: &Path) -> (Rows, Rows, Rows) {
    (
        sync_rows(path, "tasks", TASK_IDS),
        sync_rows(path, "recurring_tasks", RECURRING_IDS),
        sync_rows(path, "reminder_records", RECORD_IDS),
    )
}

fn column_names(path: &Path, table: &str) -> Vec<String> {
    let conn = Connection::open(path).unwrap();
    let mut names: Vec<String> = table_columns(&conn, table)
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    names.sort();
    names
}

fn assert_unknown_mode_untouched(db: &DbManager) {
    let task = db
        .get_recurring_task("old-rec-future-mode")
        .unwrap()
        .unwrap();
    assert_eq!(task.repeat_mode, RepeatMode::parse("BIWEEKLY"));
    assert!(!task.repeat_mode.is_known());
    assert_eq!(task.schedule_time.as_deref(), Some("08:00"));
    assert_eq!(task.next_trigger, "2026-10-02T08:00:00");
}

#[test]
fn current_device_merges_release_snapshots_without_loss() {
    for (release, schema) in RELEASES {
        let dir = TempDir::new("pull");
        let old = dir.path("old.db");
        release_db(&old, schema);
        let expected = all_rows(&old);

        let (current, new_id) = current_db(&dir.path("current.db"));
        merge_databases(&current.db_path(), &old).unwrap();

        assert_eq!(
            all_rows(&current.db_path()),
            expected,
            "从 {} 合并",
            release
        );
        assert!(current.get_task(&new_id).unwrap().is_some(), "{}", release);
        // 旧版本没有本版本不认识的列：合并后本机的表结构不变。
        for table in SYNC_TABLES {
            assert_eq!(
                column_names(&current.db_path(), table.name),
                column_names(
                    &DbManager::new(dir.path("fresh.db")).unwrap().db_path(),
                    table.name
                ),
                "{} {}",
                release,
                table.name
            );
        }
        // 本机专用的便签锚定状态不随同步过来。
        assert!(!current.get_sticky_note_pinned("old-task-pending").unwrap());
        assert_unknown_mode_untouched(&current);
    }
}

#[test]
fn release_device_receives_current_snapshot_without_column_diff() {
    for (release, schema) in RELEASES {
        let dir = TempDir::new("push");
        let old = dir.path("old.db");
        release_db(&old, schema);
        let expected = all_rows(&old);
        let old_columns: Vec<_> = SYNC_TABLES
            .iter()
            .map(|t| column_names(&old, t.name))
            .collect();

        // 当前版本上传的快照与旧版本的库合并：旧库的同步表不会多出列（当前版本没有新增同步列），
        // 旧数据不变，当前版本新建的行到达。
        let (current, new_id) = current_db(&dir.path("current.db"));
        let snapshot = export_local_snapshot_bytes(&current.db_path()).unwrap();
        merge_snapshot_bytes(&old, &snapshot).unwrap();

        let merged_columns: Vec<_> = SYNC_TABLES
            .iter()
            .map(|t| column_names(&old, t.name))
            .collect();
        assert_eq!(merged_columns, old_columns, "{}", release);
        assert_eq!(all_rows(&old), expected, "{}", release);
        let conn = Connection::open(&old).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE id = ?",
                [&new_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "{}", release);
    }
}

#[test]
fn upgrading_release_database_keeps_data() {
    for (release, schema) in RELEASES {
        let dir = TempDir::new("upgrade");
        let path = dir.path("old.db");
        release_db(&path, schema);
        let expected = all_rows(&path);

        let db = DbManager::new(path.clone()).unwrap();
        assert_eq!(all_rows(&path), expected, "从 {} 升级", release);
        assert!(
            db.get_sticky_note_pinned("old-task-pending").unwrap(),
            "{}",
            release
        );
        let task = db.get_task("old-task-pending").unwrap().unwrap();
        assert_eq!(task.tags, vec!["工作", "周报"]);
        assert_eq!(task.priority, 2);
        // 读取不会改写不认识的循环模式。
        db.list_recurring_tasks().unwrap();
        assert_unknown_mode_untouched(&db);
        assert_eq!(all_rows(&path), expected, "{}", release);
    }
}

/// “模拟 v2.1”：在 2.0.2 的库上多两个同步列，并把一条待办改得比本机新。
fn future_db(path: &Path) {
    release_db(path, "2.0.5");
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "ALTER TABLE tasks ADD COLUMN due_at TEXT;
         ALTER TABLE tasks ADD COLUMN color TEXT NOT NULL DEFAULT 'yellow';
         ALTER TABLE recurring_tasks ADD COLUMN tags TEXT NOT NULL DEFAULT '';
         UPDATE tasks SET due_at = '2026-10-10T18:00:00', color = 'blue',
                          updated_at = '2026-10-01T08:00:00'
          WHERE id = 'old-task-pending';
         UPDATE recurring_tasks SET tags = '健康' WHERE id = 'old-rec-weekly';",
    )
    .unwrap();
}

fn text(path: &Path, sql: &str) -> Option<String> {
    Connection::open(path)
        .unwrap()
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

#[test]
fn future_columns_survive_a_round_trip_through_this_version() {
    let _now = crate::time::fix_now("2026-10-01T10:00");
    let dir = TempDir::new("future");
    let future = dir.path("future.db");
    future_db(&future);

    // 1. 当前版本拉取 v2.1 的快照：新列补到本机并带上值。
    let (current, _) = current_db(&dir.path("current.db"));
    merge_databases(&current.db_path(), &future).unwrap();
    assert_eq!(
        text(
            &current.db_path(),
            "SELECT due_at || '|' || color FROM tasks WHERE id = 'old-task-pending'"
        )
        .as_deref(),
        Some("2026-10-10T18:00:00|blue")
    );

    // 2. 在当前版本修改这条待办（标题、提醒时间）与这条循环提醒（暂停）。
    current
        .update_task(
            "old-task-pending",
            "写周报（改）",
            None,
            Some("2026-10-09T18:00:00".to_string()),
            None,
        )
        .unwrap();
    current.pause_recurring_task("old-rec-weekly").unwrap();
    assert_unknown_mode_untouched(&current);

    // 3. v2.1 设备合并当前版本上传的快照：本机的修改胜出，新列的值仍在。
    let snapshot = export_local_snapshot_bytes(&current.db_path()).unwrap();
    merge_snapshot_bytes(&future, &snapshot).unwrap();
    assert_eq!(
        text(
            &future,
            "SELECT description || '|' || reminder_time || '|' || due_at || '|' || color
               FROM tasks WHERE id = 'old-task-pending'"
        )
        .as_deref(),
        Some("写周报（改）|2026-10-09T18:00:00|2026-10-10T18:00:00|blue")
    );
    assert_eq!(
        text(
            &future,
            "SELECT is_paused || '|' || tags FROM recurring_tasks WHERE id = 'old-rec-weekly'"
        )
        .as_deref(),
        Some("1|健康")
    );
    assert_eq!(
        text(
            &future,
            "SELECT repeat_mode FROM recurring_tasks WHERE id = 'old-rec-future-mode'"
        )
        .as_deref(),
        Some("BIWEEKLY")
    );
}
