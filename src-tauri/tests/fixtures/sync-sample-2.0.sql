-- 跨版本同步测试的示例数据（sync_compat 测试使用）。
-- 列与 2.0.0 – 2.0.2 发布版本的同步表一致；先按对应版本的迁移建库，再执行本文件。
-- 新版本新增同步列时不要改这里：为新版本另写一份夹具，旧夹具保持旧版本的样子。

INSERT INTO tasks (id, description, type, status, created_at, completed_at, reminder_time,
                   updated_at, deleted_at, sticky_content, sticky_pos_x, sticky_pos_y,
                   sticky_width, sticky_height, sticky_is_open, sticky_is_pinned, tags, priority)
VALUES
  ('old-task-pending', '写周报', 'ONE_TIME', 'PENDING', '2026-09-01T09:00:00', NULL, '2026-10-09T17:30:00',
   '2026-09-01T09:00:00', NULL, '- [ ] 汇总数据', -1600.0, 120.0, 300.0, 260.0, 1, 1, '工作,周报', 2),
  ('old-task-done', '交电费', 'ONE_TIME', 'COMPLETED', '2026-08-20T08:00:00', '2026-08-21T08:30:00', NULL,
   '2026-08-21T08:30:00', NULL, '', 48.0, 76.0, 284.0, 280.0, 0, 0, '', 0),
  ('old-task-deleted', '已删除的待办', 'ONE_TIME', 'PENDING', '2026-09-10T10:00:00', NULL, NULL,
   '2026-09-12T10:00:00', '2026-09-12T10:00:00', '', 48.0, 76.0, 284.0, 280.0, 0, 0, '', 0);

INSERT INTO recurring_tasks (id, description, type, status, created_at, completed_at, interval_minutes,
                             last_triggered, next_trigger, is_paused, start_time, end_time, repeat_mode,
                             schedule_time, schedule_weekday, schedule_day, cron_expression,
                             updated_at, deleted_at, schedule_weekdays)
VALUES
  ('old-rec-workday', '站会', 'RECURRING', 'PENDING', '2026-09-01T09:00:00', NULL, 60,
   '2026-09-30T09:30:00', '2026-10-08T09:30:00', 0, NULL, NULL, 'WORKDAY',
   '09:30', NULL, NULL, NULL, '2026-09-30T09:30:00', NULL, NULL),
  ('old-rec-weekly', '健身', 'RECURRING', 'PENDING', '2026-09-01T09:00:00', NULL, 60,
   NULL, '2026-10-02T19:00:00', 0, NULL, NULL, 'WEEKLY',
   '19:00', 0, NULL, NULL, '2026-09-01T09:00:00', NULL, 21),
  ('old-rec-future-mode', '更新版本才有的模式', 'RECURRING', 'PENDING', '2026-09-01T09:00:00', NULL, 60,
   NULL, '2026-10-02T08:00:00', 0, NULL, NULL, 'BIWEEKLY',
   '08:00', NULL, NULL, NULL, '2026-09-01T09:00:00', NULL, NULL);

INSERT INTO reminder_records (id, reminder_id, description, type, trigger_time, close_time, action,
                              updated_at, deleted_at)
VALUES
  ('old-record-1', 'old-rec-workday', '站会', 'RECURRING', '2026-09-30T09:30:00', '2026-09-30T09:31:00',
   'COMPLETED', '2026-09-30T09:31:00', NULL),
  ('old-record-2', 'old-task-done', '交电费', 'ONE_TIME', '2026-08-21T08:00:00', '2026-08-21T08:30:00',
   'SNOOZED', '2026-08-21T08:30:00', NULL);
