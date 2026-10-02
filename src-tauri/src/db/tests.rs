use super::*;
use crate::kinds::RepeatMode;

fn temp_db() -> (DbManager, PathBuf) {
    let dir = std::env::temp_dir().join(format!("taskreminder-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = DbManager::new(dir.join("test.db")).unwrap();
    (db, dir)
}

fn days_ago(days: i64) -> String {
    format_datetime(&(time::now() - chrono::Duration::days(days)))
}

fn task_row(db: &DbManager, id: &str) -> Option<(String, Option<String>)> {
    let conn = db.get_conn().unwrap();
    conn.query_row(
        "SELECT status, deleted_at FROM tasks WHERE id = ?",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .unwrap()
}

fn sample_recurring() -> RecurringTask {
    RecurringTask {
        id: String::new(),
        description: "weekly".to_string(),
        task_type: TaskType::Recurring,
        status: TaskStatus::Pending,
        created_at: String::new(),
        completed_at: None,
        reminder_time: None,
        updated_at: None,
        deleted_at: None,
        interval_minutes: 60,
        last_triggered: None,
        next_trigger: String::new(),
        is_paused: false,
        start_time: None,
        end_time: None,
        repeat_mode: RepeatMode::Daily,
        schedule_time: Some("09:00".to_string()),
        schedule_weekday: None,
        schedule_weekdays: None,
        schedule_day: None,
        cron_expression: None,
        tags: Vec::new(),
    }
}

#[test]
fn cleanup_tombstones_old_completed_tasks_instead_of_deleting() {
    let (db, dir) = temp_db();
    let old = db.create_task("old", None).unwrap();
    let recent = db.create_task("recent", None).unwrap();
    {
        let conn = db.get_conn().unwrap();
        conn.execute(
            "UPDATE tasks SET status = 'COMPLETED', completed_at = ? WHERE id = ?",
            params![days_ago(40), old.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE tasks SET status = 'COMPLETED', completed_at = ? WHERE id = ?",
            params![days_ago(1), recent.id],
        )
        .unwrap();
    }

    db.cleanup_data(TOMBSTONE_RETENTION_DAYS_LOCAL).unwrap();

    // 过期的已完成任务仍保留一行墓碑，云同步才能把删除传播到其他设备。
    let (_, deleted_at) = task_row(&db, &old.id).expect("old task row kept as tombstone");
    assert!(deleted_at.is_some());
    let (_, deleted_at) = task_row(&db, &recent.id).unwrap();
    assert!(deleted_at.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn purge_removes_only_expired_tombstones() {
    let (db, dir) = temp_db();
    let expired = db.create_task("expired", None).unwrap();
    let fresh = db.create_task("fresh", None).unwrap();
    let alive = db.create_task("alive", None).unwrap();
    {
        let conn = db.get_conn().unwrap();
        conn.execute(
            "UPDATE tasks SET deleted_at = ? WHERE id = ?",
            params![days_ago(10), expired.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE tasks SET deleted_at = ? WHERE id = ?",
            params![days_ago(2), fresh.id],
        )
        .unwrap();
    }

    db.purge_expired_tombstones(7).unwrap();
    assert!(task_row(&db, &expired.id).is_none());
    assert!(task_row(&db, &fresh.id).is_some());
    assert!(task_row(&db, &alive.id).is_some());

    // 开启同步时保留期更长，10 天前的墓碑不应被删除。
    let kept = db.create_task("kept", None).unwrap();
    {
        let conn = db.get_conn().unwrap();
        conn.execute(
            "UPDATE tasks SET deleted_at = ? WHERE id = ?",
            params![days_ago(10), kept.id],
        )
        .unwrap();
    }
    db.purge_expired_tombstones(tombstone_retention_days(true))
        .unwrap();
    assert!(task_row(&db, &kept.id).is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn trash_lists_restores_and_expires_tombstones() {
    let (db, dir) = temp_db();
    let task = db.create_task("trash me", None).unwrap();
    let too_old = db.create_task("too old", None).unwrap();
    let alive = db.create_task("alive", None).unwrap();
    db.delete_task(&task.id).unwrap();
    {
        let conn = db.get_conn().unwrap();
        conn.execute(
            "UPDATE tasks SET deleted_at = ? WHERE id = ?",
            params![days_ago(10), too_old.id],
        )
        .unwrap();
    }

    // 只列出保留期内的墓碑，不含正常行。
    let ids: Vec<String> = db
        .list_deleted_tasks(7)
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(ids, vec![task.id.clone()]);
    assert!(!ids.contains(&alive.id));

    db.restore_task(&task.id).unwrap();
    let (_, deleted_at) = task_row(&db, &task.id).unwrap();
    assert!(deleted_at.is_none());
    assert!(db.list_deleted_tasks(7).unwrap().is_empty());

    // 永久删除：从回收站消失，行仍在（等待同步传播），清理后物理删除。
    db.delete_task(&task.id).unwrap();
    let before = db.get_task(&task.id).unwrap().unwrap().updated_at;
    db.expire_tombstones(TrashTable::Tasks, std::slice::from_ref(&task.id))
        .unwrap();
    assert!(db.list_deleted_tasks(7).unwrap().is_empty());
    let expired = db.get_task(&task.id).unwrap().unwrap();
    assert_eq!(expired.deleted_at.as_deref(), Some(EXPIRED_TOMBSTONE_TIME));
    assert!(expired.updated_at >= before);
    db.purge_expired_tombstones(60).unwrap();
    assert!(db.get_task(&task.id).unwrap().is_none());

    // 不能用“永久删除”删掉正常行。
    db.expire_tombstones(TrashTable::Tasks, std::slice::from_ref(&alive.id))
        .unwrap();
    assert!(db
        .get_task(&alive.id)
        .unwrap()
        .unwrap()
        .deleted_at
        .is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn trash_handles_recurring_tasks() {
    let (db, dir) = temp_db();
    let mut draft = sample_recurring();
    draft.next_trigger = "2026-01-01T09:00:00".to_string();
    let task = db.create_recurring_task(&draft).unwrap();
    db.delete_recurring_task(&task.id).unwrap();
    assert!(db.list_recurring_tasks().unwrap().is_empty());
    assert_eq!(db.list_deleted_recurring_tasks(7).unwrap().len(), 1);

    db.restore_recurring_task(&task.id).unwrap();
    assert_eq!(db.list_recurring_tasks().unwrap().len(), 1);
    assert!(db.list_deleted_recurring_tasks(7).unwrap().is_empty());

    db.delete_recurring_task(&task.id).unwrap();
    db.expire_tombstones(TrashTable::RecurringTasks, std::slice::from_ref(&task.id))
        .unwrap();
    assert!(db.list_deleted_recurring_tasks(60).unwrap().is_empty());
    db.purge_expired_tombstones(60).unwrap();
    assert!(db.get_recurring_task(&task.id).unwrap().is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn purge_also_clears_deleted_reminder_records() {
    let (db, dir) = temp_db();
    let record = db
        .create_reminder_record("task-1", "desc", ReminderKind::Task)
        .unwrap();
    {
        let conn = db.get_conn().unwrap();
        conn.execute(
            "UPDATE reminder_records SET deleted_at = ? WHERE id = ?",
            params![days_ago(30), record.id],
        )
        .unwrap();
    }
    db.purge_expired_tombstones(7).unwrap();
    assert!(db.get_reminder_record(&record.id).unwrap().is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn recurring_weekday_mask_roundtrip_and_quick_add_defaults() {
    let (db, dir) = temp_db();
    let mut draft = sample_recurring();
    draft.next_trigger = "2026-09-28T09:00:00".to_string();
    draft.repeat_mode = RepeatMode::Weekly;
    draft.schedule_weekday = Some(1);
    draft.schedule_weekdays = Some(0b10101);
    let created = db.create_recurring_task(&draft).unwrap();
    let loaded = db.get_recurring_task(&created.id).unwrap().unwrap();
    assert_eq!(loaded.schedule_weekdays, Some(0b10101));

    let mut updated = loaded.clone();
    updated.schedule_weekdays = Some(0b1100000);
    db.update_recurring_task(&updated).unwrap();
    let loaded = db.get_recurring_task(&created.id).unwrap().unwrap();
    assert_eq!(loaded.schedule_weekdays, Some(0b1100000));

    let mut settings = db.load_settings().unwrap();
    assert!(settings.quick_add_enabled);
    assert_eq!(settings.quick_add_shortcut, "CommandOrControl+Alt+N");
    settings.quick_add_enabled = false;
    settings.quick_add_shortcut = "Alt+Space".to_string();
    db.save_settings(&settings).unwrap();
    let settings = db.load_settings().unwrap();
    assert!(!settings.quick_add_enabled);
    assert_eq!(settings.quick_add_shortcut, "Alt+Space");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn task_tags_and_priority_roundtrip() {
    let (db, dir) = temp_db();
    let meta = TaskMeta {
        tags: vec![
            "#工作".to_string(),
            " 周报 ".to_string(),
            "工作".to_string(),
            "a,b".to_string(),
            String::new(),
        ],
        priority: 7,
    };
    let task = db.create_task_with_meta("写周报", None, &meta).unwrap();
    assert_eq!(task.tags, vec!["工作", "周报", "ab"]);
    assert_eq!(task.priority, 3);
    let loaded = db.get_task(&task.id).unwrap().unwrap();
    assert_eq!(loaded.tags, task.tags);
    assert_eq!(loaded.priority, 3);

    // 不传 meta 时保留原有标签与优先级。
    db.update_task(&task.id, "写周报 v2", None, None, None)
        .unwrap();
    let loaded = db.get_task(&task.id).unwrap().unwrap();
    assert_eq!(loaded.description, "写周报 v2");
    assert_eq!(loaded.tags, vec!["工作", "周报", "ab"]);
    assert_eq!(loaded.priority, 3);

    let cleared = TaskMeta {
        tags: Vec::new(),
        priority: -1,
    };
    db.update_task(&task.id, "写周报 v2", None, None, Some(&cleared))
        .unwrap();
    let loaded = db.get_task(&task.id).unwrap().unwrap();
    assert!(loaded.tags.is_empty());
    assert_eq!(loaded.priority, 0);

    // 旧版本创建的行没有标签：默认空。
    let plain = db.create_task("plain", None).unwrap();
    let loaded = db.get_task(&plain.id).unwrap().unwrap();
    assert!(loaded.tags.is_empty());
    assert_eq!(loaded.priority, 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn has_reminder_record_since_compares_trigger_time() {
    let (db, dir) = temp_db();
    let record = db
        .create_reminder_record("task-1", "desc", ReminderKind::Task)
        .unwrap();
    let trigger = parse_datetime_any(&record.trigger_time).unwrap();

    assert!(db
        .has_reminder_record_since("task-1", &(trigger - chrono::Duration::minutes(5)))
        .unwrap());
    assert!(db.has_reminder_record_since("task-1", &trigger).unwrap());
    assert!(!db
        .has_reminder_record_since("task-1", &(trigger + chrono::Duration::minutes(5)))
        .unwrap());
    assert!(!db
        .has_reminder_record_since("task-2", &(trigger - chrono::Duration::minutes(5)))
        .unwrap());

    // 已软删除的记录也算触发过，删除记录不应导致重复弹出。
    db.delete_reminder_record(&record.id).unwrap();
    assert!(db.has_reminder_record_since("task-1", &trigger).unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

fn sticky_row(db: &DbManager, id: &str) -> (f64, f64, f64, f64, String) {
    let conn = db.get_conn().unwrap();
    conn.query_row(
        "SELECT sticky_pos_x, sticky_pos_y, sticky_width, sticky_height, updated_at FROM tasks WHERE id = ?",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )
    .unwrap()
}

fn backdate_task(db: &DbManager, id: &str) {
    let conn = db.get_conn().unwrap();
    conn.execute(
        "UPDATE tasks SET updated_at = '2020-01-01 00:00:00' WHERE id = ?",
        [id],
    )
    .unwrap();
}

#[test]
fn sticky_move_keeps_negative_coordinates_and_skips_unchanged() {
    let (db, dir) = temp_db();
    let note = db
        .create_custom_sticky_note("note", None, Some(48.0), Some(76.0), None, None)
        .unwrap();
    let id = note.task_id.clone();

    // 主屏左侧副屏的负坐标原样保存。
    db.move_sticky_note(&id, -1500.0, -100.0).unwrap();
    let (x, y, _, _, _) = sticky_row(&db, &id);
    assert_eq!((x, y), (-1500.0, -100.0));

    // 位置不变（打开窗口时 set_position 触发的 Moved）不改 updated_at，避免产生同步改动。
    backdate_task(&db, &id);
    db.move_sticky_note(&id, -1500.2, -100.0).unwrap();
    let (_, _, _, _, updated_at) = sticky_row(&db, &id);
    assert_eq!(updated_at, "2020-01-01 00:00:00");

    db.move_sticky_note(&id, 200.0, -100.0).unwrap();
    let (x, _, _, _, updated_at) = sticky_row(&db, &id);
    assert_eq!(x, 200.0);
    assert_ne!(updated_at, "2020-01-01 00:00:00");

    // 尺寸同理。
    let (_, _, width, height, _) = sticky_row(&db, &id);
    backdate_task(&db, &id);
    db.resize_sticky_note(&id, width, height).unwrap();
    assert_eq!(sticky_row(&db, &id).4, "2020-01-01 00:00:00");
    db.resize_sticky_note(&id, width + 40.0, height).unwrap();
    assert_eq!(sticky_row(&db, &id).2, width + 40.0);
    assert_ne!(sticky_row(&db, &id).4, "2020-01-01 00:00:00");

    // 已关闭的便签被移动时重新记为打开。
    db.close_sticky_note(&id).unwrap();
    backdate_task(&db, &id);
    db.move_sticky_note(&id, 200.0, -100.0).unwrap();
    assert!(db.get_sticky_note(&id).unwrap().unwrap().is_open);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unknown_repeat_mode_survives_read_and_write() {
    // 模拟 v2.1 新增的模式经同步写入本机数据库。
    let (db, dir) = temp_db();
    let created = db.create_recurring_task(&sample_recurring()).unwrap();
    {
        let conn = db.get_conn().unwrap();
        conn.execute(
            "UPDATE recurring_tasks SET repeat_mode = 'BIWEEKLY', schedule_time = '09:00', cron_expression = 'custom' WHERE id = ?",
            [&created.id],
        )
        .unwrap();
    }
    let mut task = db.get_recurring_task(&created.id).unwrap().unwrap();
    assert_eq!(
        task.repeat_mode,
        RepeatMode::Unknown("BIWEEKLY".to_string())
    );
    // 暂停等只改其他字段的写入不会改动模式与规则字段。
    task.is_paused = true;
    db.update_recurring_task(&task).unwrap();
    let conn = db.get_conn().unwrap();
    let (mode, time, cron): (String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT repeat_mode, schedule_time, cron_expression FROM recurring_tasks WHERE id = ?",
            [&created.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(mode, "BIWEEKLY");
    assert_eq!(time.as_deref(), Some("09:00"));
    assert_eq!(cron.as_deref(), Some("custom"));
    drop(conn);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn ipc_json_is_unchanged_by_typed_fields() {
    let (db, dir) = temp_db();
    let task = db.create_task("待办", None).unwrap();
    let json = serde_json::to_value(&task).unwrap();
    assert_eq!(json["type"], "ONE_TIME");
    assert_eq!(json["status"], "PENDING");
    db.complete_task(&task.id).unwrap();
    let done = serde_json::to_value(db.get_task(&task.id).unwrap().unwrap()).unwrap();
    assert_eq!(done["status"], "COMPLETED");

    let recurring = db.create_recurring_task(&sample_recurring()).unwrap();
    let json = serde_json::to_value(&recurring).unwrap();
    assert_eq!(json["type"], "RECURRING");
    assert_eq!(json["repeatMode"], recurring.repeat_mode.as_str());

    let record = db
        .create_reminder_record("task-1", "desc", ReminderKind::Task)
        .unwrap();
    let json = serde_json::to_value(&record).unwrap();
    assert_eq!(json["type"], "TASK");
    assert_eq!(json["action"], "PENDING");

    // 前端发来的字符串照常解析。
    let parsed: ReminderRecord = serde_json::from_value(json).unwrap();
    assert_eq!(parsed.action, ReminderAction::Pending);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tombstone_cutoff_counts_back_from_now() {
    let _now = time::fix_now("2026-09-30T12:00:00");
    assert_eq!(tombstone_cutoff(7), "2026-09-23T12:00:00");
    assert_eq!(tombstone_cutoff(60), "2026-08-01T12:00:00");
    // 至少保留 1 天。
    assert_eq!(tombstone_cutoff(0), "2026-09-29T12:00:00");
}

#[test]
fn deleting_or_completing_task_closes_its_sticky_note() {
    let (db, dir) = temp_db();
    let deleted = db
        .create_custom_sticky_note("删除", Some("内容"), None, None, None, None)
        .unwrap();
    let completed = db
        .create_custom_sticky_note("完成", Some("内容"), None, None, None, None)
        .unwrap();
    assert!(
        db.get_sticky_note(&deleted.task_id)
            .unwrap()
            .unwrap()
            .is_open
    );

    db.delete_task(&deleted.task_id).unwrap();
    assert!(
        !db.get_sticky_note(&deleted.task_id)
            .unwrap()
            .unwrap()
            .is_open
    );
    // 从回收站恢复时不会重新弹出便签。
    db.restore_task(&deleted.task_id).unwrap();
    assert!(
        !db.get_sticky_note(&deleted.task_id)
            .unwrap()
            .unwrap()
            .is_open
    );

    db.complete_task(&completed.task_id).unwrap();
    assert!(
        !db.get_sticky_note(&completed.task_id)
            .unwrap()
            .unwrap()
            .is_open
    );
    db.uncomplete_task(&completed.task_id).unwrap();
    assert!(
        !db.get_sticky_note(&completed.task_id)
            .unwrap()
            .unwrap()
            .is_open
    );
    // 便签内容保留，可以手动重新打开。
    assert_eq!(
        db.get_sticky_note(&completed.task_id)
            .unwrap()
            .unwrap()
            .content,
        "内容"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reopening_sticky_note_keeps_negative_coordinates() {
    let (db, dir) = temp_db();
    let note = db
        .create_custom_sticky_note("note", None, Some(48.0), Some(76.0), None, None)
        .unwrap();
    db.move_sticky_note(&note.task_id, -1500.0, -100.0).unwrap();
    db.close_sticky_note(&note.task_id).unwrap();
    let reopened = db
        .open_sticky_note(&note.task_id, None, Some(48.0), Some(76.0))
        .unwrap();
    assert_eq!((reopened.pos_x, reopened.pos_y), (-1500.0, -100.0));
    let stored = db.get_sticky_note(&note.task_id).unwrap().unwrap();
    assert_eq!((stored.pos_x, stored.pos_y), (-1500.0, -100.0));
    assert!(stored.is_open);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sticky_snap_setting_defaults_on_and_roundtrips() {
    let (db, dir) = temp_db();
    assert!(db.sticky_snap_enabled().unwrap());
    let mut settings = db.load_settings().unwrap();
    assert!(settings.sticky_snap_enabled);
    settings.sticky_snap_enabled = false;
    db.save_settings(&settings).unwrap();
    assert!(!db.load_settings().unwrap().sticky_snap_enabled);
    assert!(!db.sticky_snap_enabled().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn migration_skips_columns_already_adopted_by_sync() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE tasks (id TEXT PRIMARY KEY, due_at TEXT)")
        .unwrap();
    // 同步已把 due_at 补到本机，之后的迁移再加这一列时跳过；其他语句照常执行。
    execute_sql_script(
        &conn,
        "-- 迁移\nALTER TABLE tasks ADD COLUMN due_at TEXT;\nALTER TABLE tasks ADD lead_minutes INTEGER NOT NULL DEFAULT 0;",
    )
    .unwrap();
    let columns = crate::sync_schema::table_columns(&conn, "tasks").unwrap();
    let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["id", "due_at", "lead_minutes"]);
    // 其他错误仍然报出。
    assert!(execute_sql_script(&conn, "ALTER TABLE missing ADD COLUMN x TEXT").is_err());
}

#[test]
fn import_keeps_columns_unknown_to_this_version() {
    let (db, dir) = temp_db();
    let recurring = db.create_recurring_task(&sample_recurring()).unwrap();
    {
        let conn = Connection::open(db.db_path()).unwrap();
        conn.execute_batch(&format!(
            "ALTER TABLE recurring_tasks ADD COLUMN category TEXT;
             UPDATE recurring_tasks SET category = '健身' WHERE id = '{}';",
            recurring.id
        ))
        .unwrap();
    }
    let mut imported = db.get_recurring_task(&recurring.id).unwrap().unwrap();
    imported.description = "imported".to_string();
    imported.updated_at = Some("2999-01-01T00:00:00".to_string());
    db.import_rows(&[], &[imported], &[]).unwrap();

    let conn = Connection::open(db.db_path()).unwrap();
    let (description, category): (String, Option<String>) = conn
        .query_row(
            "SELECT description, category FROM recurring_tasks WHERE id = ?",
            [&recurring.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(description, "imported");
    assert_eq!(category.as_deref(), Some("健身"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn holiday_auto_update_defaults_on_and_roundtrips() {
    let (db, dir) = temp_db();
    assert!(db.holiday_auto_update_enabled().unwrap());
    let mut settings = db.load_settings().unwrap();
    assert!(settings.holiday_auto_update);
    settings.holiday_auto_update = false;
    db.save_settings(&settings).unwrap();
    assert!(!db.load_settings().unwrap().holiday_auto_update);
    assert!(!db.holiday_auto_update_enabled().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn batch_update_tasks_changes_only_live_rows_that_differ() {
    let (db, dir) = temp_db();
    let _now = time::fix_now("2026-10-02T10:00");
    let a = db
        .create_task_with_meta(
            "a",
            None,
            &TaskMeta {
                tags: vec!["工作".to_string()],
                priority: 1,
            },
        )
        .unwrap();
    let b = db.create_task("b", None).unwrap();
    let gone = db.create_task("gone", None).unwrap();
    db.delete_task(&gone.id).unwrap();
    let ids = vec![
        a.id.clone(),
        b.id.clone(),
        gone.id.clone(),
        "missing".to_string(),
    ];

    let changed = db
        .batch_update_tasks(
            &ids,
            &TaskBatchOp::AddTags(vec!["#周报".into(), "工作".into()]),
        )
        .unwrap();
    assert_eq!(changed, vec![a.id.clone(), b.id.clone()]);
    assert_eq!(
        db.get_task(&a.id).unwrap().unwrap().tags,
        vec!["工作", "周报"]
    );
    assert_eq!(
        db.get_task(&b.id).unwrap().unwrap().tags,
        vec!["周报", "工作"]
    );
    // 再加一次相同的标签：没有变化，不写入。
    assert!(db
        .batch_update_tasks(&ids, &TaskBatchOp::AddTags(vec!["周报".into()]))
        .unwrap()
        .is_empty());

    let changed = db
        .batch_update_tasks(&ids, &TaskBatchOp::SetPriority(1))
        .unwrap();
    assert_eq!(changed, vec![b.id.clone()], "a 本来就是 1");
    let changed = db
        .batch_update_tasks(
            &ids,
            &TaskBatchOp::SetReminder(Some("2026-10-03T09:00:00".into())),
        )
        .unwrap();
    assert_eq!(changed.len(), 2);
    assert_eq!(
        db.get_task(&b.id)
            .unwrap()
            .unwrap()
            .reminder_time
            .as_deref(),
        Some("2026-10-03T09:00:00")
    );

    db.complete_task(&a.id).unwrap();
    let changed = db.batch_update_tasks(&ids, &TaskBatchOp::Complete).unwrap();
    assert_eq!(changed, vec![b.id.clone()], "已完成的跳过");
    let changed = db.batch_update_tasks(&ids, &TaskBatchOp::Delete).unwrap();
    assert_eq!(changed, vec![a.id.clone(), b.id.clone()]);
    assert!(db.list_active_tasks().unwrap().is_empty());
    assert!(db.list_completed_tasks().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
