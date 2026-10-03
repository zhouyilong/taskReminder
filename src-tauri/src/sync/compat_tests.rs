//! 跨版本同步测试：按各发布版本的迁移建库并写入示例数据（`tests/fixtures/sync-sample-2.0.sql`，
//! 2.1.0 起再加 `sync-sample-2.1.sql`），检查与当前版本双向合并、从旧版本升级都不丢数据；
//! 再用“模拟更新的版本”（多同步列、多一种循环模式）的库检查不认识的列与值原样保留。
//!
//! 发布新版本时：把它的库结构版本加到 `RELEASES`；若新增了同步列，为它另写一份夹具。

use std::path::{Path, PathBuf};

use rusqlite::{types::Value, Connection};

use super::{export_local_snapshot_bytes, merge_databases, merge_snapshot_bytes};
use crate::db::{create_schema_up_to, DbManager};
use crate::kinds::RepeatMode;
use crate::sync_schema::{table_columns, SYNC_TABLES};

/// （应用版本，发布时的库结构版本）。
const RELEASES: &[(&str, &str)] = &[
    ("2.0.0", "2.0.1"),
    ("2.0.1", "2.0.3"),
    ("2.0.2", "2.0.5"),
    ("2.0.3", "2.0.5"),
    ("2.1.0", "2.1.0"),
    ("2.1.1", "2.1.1"),
];
const SAMPLE: &str = include_str!("../../tests/fixtures/sync-sample-2.0.sql");
/// v2.1 新增同步列的示例数据，只写入库结构 2.1.0 起的库。
const SAMPLE_2_1: &str = include_str!("../../tests/fixtures/sync-sample-2.1.sql");
const V2_1_SCHEMA: &str = "2.1.0";

fn parse_version(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map(|part| part.parse().unwrap())
        .collect()
}

/// 库结构版本是否已有 v2.1 的同步列。
fn has_v2_1(schema_version: &str) -> bool {
    parse_version(schema_version) >= parse_version(V2_1_SCHEMA)
}

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
    if has_v2_1(schema_version) {
        conn.execute_batch(SAMPLE_2_1).unwrap();
    }
}

/// 当前版本的库，含一条本机新建的待办。
fn current_db(path: &Path) -> (DbManager, String) {
    let db = DbManager::new(path.to_path_buf()).unwrap();
    let task = db.create_task("当前版本新建", None).unwrap();
    (db, task.id)
}

type Rows = Vec<(String, Vec<(String, Value)>)>;

/// 示例数据所属版本已有的列：之后版本新增的列旧数据里没有值，不参与比较。
fn sample_columns(table: &str, schema_version: &str) -> Vec<String> {
    let conn = Connection::open_in_memory().unwrap();
    create_schema_up_to(&conn, schema_version).unwrap();
    table_columns(&conn, table)
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect()
}

/// 读出一张表中指定 id 的行（只取示例数据版本已有的同步列），用于比较合并前后是否一致。
fn sync_rows(path: &Path, table: &str, ids: &[&str], schema_version: &str) -> Rows {
    let spec = SYNC_TABLES.iter().find(|t| t.name == table).unwrap();
    let known = sample_columns(table, schema_version);
    let conn = Connection::open(path).unwrap();
    let columns: Vec<&str> = spec
        .columns
        .iter()
        .map(|c| c.name)
        .filter(|name| *name != "updated_at" && known.iter().any(|k| k == name))
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
const V2_1_TASK_IDS: &[&str] = &["v21-task-due", "v21-task-due-only", "v21-task-future-color"];
const V2_1_RECURRING_IDS: &[&str] = &["v21-rec-last-day", "v21-rec-last-workday"];
const V2_1_RECORD_IDS: &[&str] = &["v21-record-1"];

/// 示例数据中的全部行。2.0.x 的库只比较 2.0 示例数据已有的列；2.1.0 起的库再加上 v2.1 的行与列。
fn all_rows(path: &Path, schema_version: &str) -> (Rows, Rows, Rows) {
    let v2_1 = has_v2_1(schema_version);
    let columns_at = if v2_1 { V2_1_SCHEMA } else { "2.0.5" };
    let ids = |base: &[&'static str], added: &[&'static str]| -> Vec<&'static str> {
        let mut ids = base.to_vec();
        if v2_1 {
            ids.extend_from_slice(added);
        }
        ids
    };
    (
        sync_rows(path, "tasks", &ids(TASK_IDS, V2_1_TASK_IDS), columns_at),
        sync_rows(
            path,
            "recurring_tasks",
            &ids(RECURRING_IDS, V2_1_RECURRING_IDS),
            columns_at,
        ),
        sync_rows(
            path,
            "reminder_records",
            &ids(RECORD_IDS, V2_1_RECORD_IDS),
            columns_at,
        ),
    )
}

/// 2.1.0 起的示例数据：v2.1 字段（含不认识的便签颜色）与两种月末模式读出来都对。
fn assert_v2_1_sample(db: &DbManager, release: &str) {
    let task = db.get_task("v21-task-due").unwrap().unwrap();
    assert_eq!(
        task.due_at.as_deref(),
        Some("2026-10-16T18:00:00"),
        "{}",
        release
    );
    assert_eq!(task.sticky_color, "blue", "{}", release);
    assert_eq!(task.sort_order, Some(1.5), "{}", release);
    let future = db.get_task("v21-task-future-color").unwrap().unwrap();
    assert_eq!(future.sticky_color, "teal", "{}", release);
    let last_day = db.get_recurring_task("v21-rec-last-day").unwrap().unwrap();
    assert_eq!(
        last_day.repeat_mode,
        RepeatMode::MonthlyLastDay,
        "{}",
        release
    );
    assert_eq!(last_day.tags, vec!["家务"], "{}", release);
    let last_workday = db
        .get_recurring_task("v21-rec-last-workday")
        .unwrap()
        .unwrap();
    assert_eq!(
        last_workday.repeat_mode,
        RepeatMode::MonthlyLastWorkday,
        "{}",
        release
    );
    assert_eq!(last_workday.tags, vec!["工作", "报销"], "{}", release);
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
        let expected = all_rows(&old, schema);

        let (current, new_id) = current_db(&dir.path("current.db"));
        merge_databases(&current.db_path(), &old).unwrap();

        assert_eq!(
            all_rows(&current.db_path(), schema),
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
        if has_v2_1(schema) {
            assert_v2_1_sample(&current, release);
        }
    }
}

#[test]
fn release_database_receives_current_snapshot() {
    for (release, schema) in RELEASES {
        let dir = TempDir::new("push");
        let old = dir.path("old.db");
        release_db(&old, schema);
        let expected = all_rows(&old, schema);

        // 当前版本上传的快照与旧版本建出的库合并：旧数据不变，当前版本新建的行（含 v2.1 新列的值）到达，
        // 表结构补齐到与当前版本一致（合并时补上后来加入的同步列）。
        let (current, new_id) = current_db(&dir.path("current.db"));
        current
            .set_task_due(&new_id, Some("2026-10-09T18:00:00"))
            .unwrap();
        let snapshot = export_local_snapshot_bytes(&current.db_path()).unwrap();
        merge_snapshot_bytes(&old, &snapshot).unwrap();

        let fresh = DbManager::new(dir.path("fresh.db")).unwrap();
        for table in SYNC_TABLES {
            assert_eq!(
                column_names(&old, table.name),
                column_names(&fresh.db_path(), table.name),
                "{} {}",
                release,
                table.name
            );
        }
        assert_eq!(all_rows(&old, schema), expected, "{}", release);
        let conn = Connection::open(&old).unwrap();
        let due: Option<String> = conn
            .query_row("SELECT due_at FROM tasks WHERE id = ?", [&new_id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(due.as_deref(), Some("2026-10-09T18:00:00"), "{}", release);
    }
}

#[test]
fn upgrading_release_database_keeps_data() {
    for (release, schema) in RELEASES {
        let dir = TempDir::new("upgrade");
        let path = dir.path("old.db");
        release_db(&path, schema);
        let expected = all_rows(&path, schema);

        let db = DbManager::new(path.clone()).unwrap();
        assert_eq!(all_rows(&path, schema), expected, "从 {} 升级", release);
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
        if has_v2_1(schema) {
            assert_v2_1_sample(&db, release);
        }
        assert_eq!(all_rows(&path, schema), expected, "{}", release);
    }
}

/// “模拟更新的版本”：在 2.0.2 的库上多两个同步列，并把一条待办改得比本机新。
fn future_db(path: &Path) {
    release_db(path, "2.0.5");
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "ALTER TABLE tasks ADD COLUMN location TEXT;
         ALTER TABLE tasks ADD COLUMN accent TEXT NOT NULL DEFAULT 'yellow';
         ALTER TABLE recurring_tasks ADD COLUMN category TEXT NOT NULL DEFAULT '';
         UPDATE tasks SET location = '会议室 A', accent = 'blue',
                          updated_at = '2026-10-01T08:00:00'
          WHERE id = 'old-task-pending';
         UPDATE recurring_tasks SET category = '健康' WHERE id = 'old-rec-weekly';",
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

    // 1. 当前版本拉取更新版本 的快照：新列补到本机并带上值。
    let (current, _) = current_db(&dir.path("current.db"));
    merge_databases(&current.db_path(), &future).unwrap();
    assert_eq!(
        text(
            &current.db_path(),
            "SELECT location || '|' || accent FROM tasks WHERE id = 'old-task-pending'"
        )
        .as_deref(),
        Some("会议室 A|blue")
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

    // 3. 更新版本的设备合并当前版本上传的快照：本机的修改胜出，新列的值仍在。
    let snapshot = export_local_snapshot_bytes(&current.db_path()).unwrap();
    merge_snapshot_bytes(&future, &snapshot).unwrap();
    assert_eq!(
        text(
            &future,
            "SELECT description || '|' || reminder_time || '|' || location || '|' || accent
               FROM tasks WHERE id = 'old-task-pending'"
        )
        .as_deref(),
        Some("写周报（改）|2026-10-09T18:00:00|会议室 A|blue")
    );
    assert_eq!(
        text(
            &future,
            "SELECT is_paused || '|' || category FROM recurring_tasks WHERE id = 'old-rec-weekly'"
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

#[test]
fn v2_1_fields_survive_edits_from_2_0_devices() {
    let dir = TempDir::new("v21");
    let (current, _) = current_db(&dir.path("current.db"));
    let old = dir.path("old.db");
    release_db(&old, "2.0.5");
    merge_databases(&current.db_path(), &old).unwrap();

    // 本机（v2.1）给旧待办设置截止时间、便签颜色、排序，给循环提醒加标签。
    current
        .set_task_due("old-task-pending", Some("2026-10-09T18:00:00"))
        .unwrap();
    current
        .set_sticky_note_color("old-task-pending", "blue")
        .unwrap();
    current
        .set_task_sort_orders(&[("old-task-pending".to_string(), 1.5)])
        .unwrap();
    let mut weekly = current
        .get_recurring_task("old-rec-weekly")
        .unwrap()
        .unwrap();
    weekly.tags = vec!["健康".to_string()];
    current.update_recurring_task(&weekly).unwrap();

    // 2.0.x 设备随后修改了同一条待办与循环提醒（它的库没有这些列），并上传。
    let conn = Connection::open(&old).unwrap();
    conn.execute_batch(
        "UPDATE tasks SET description = '写周报（旧设备改）', updated_at = '2999-01-01T00:00:00'
          WHERE id = 'old-task-pending';
         UPDATE recurring_tasks SET description = '健身（旧设备改）', updated_at = '2999-01-01T00:00:00'
          WHERE id = 'old-rec-weekly';",
    )
    .unwrap();
    drop(conn);
    merge_databases(&current.db_path(), &old).unwrap();

    let task = current.get_task("old-task-pending").unwrap().unwrap();
    assert_eq!(task.description, "写周报（旧设备改）");
    assert_eq!(task.due_at.as_deref(), Some("2026-10-09T18:00:00"));
    assert_eq!(task.sticky_color, "blue");
    assert_eq!(task.sort_order, Some(1.5));
    let weekly = current
        .get_recurring_task("old-rec-weekly")
        .unwrap()
        .unwrap();
    assert_eq!(weekly.description, "健身（旧设备改）");
    assert_eq!(weekly.tags, vec!["健康"]);
}

#[test]
fn pre_2_0_device_edits_keep_tags_and_priority() {
    // 1.x 的库没有 tags / priority 列（V2.0.0 加入）。
    let dir = TempDir::new("pre20");
    let old = dir.path("old.db");
    {
        let conn = Connection::open(&old).unwrap();
        create_schema_up_to(&conn, "1.6.0").unwrap();
        conn.execute(
            "INSERT INTO tasks (id, description, type, status, created_at, updated_at)
             VALUES ('shared', '旧设备改过', 'ONE_TIME', 'PENDING', '2026-01-01T00:00:00', '2999-01-01T00:00:00')",
            [],
        )
        .unwrap();
    }
    let (current, _) = current_db(&dir.path("current.db"));
    let conn = Connection::open(current.db_path()).unwrap();
    conn.execute(
        "INSERT INTO tasks (id, description, type, status, created_at, updated_at, tags, priority)
         VALUES ('shared', '本机', 'ONE_TIME', 'PENDING', '2026-01-01T00:00:00', '2026-02-01T00:00:00', '工作', 3)",
        [],
    )
    .unwrap();
    drop(conn);

    merge_databases(&current.db_path(), &old).unwrap();
    let task = current.get_task("shared").unwrap().unwrap();
    assert_eq!(task.description, "旧设备改过");
    assert_eq!(task.tags, vec!["工作"]);
    assert_eq!(task.priority, 3);
}
