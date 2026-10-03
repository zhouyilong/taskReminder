-- 跨版本同步测试的示例数据（v2.1 新增的同步列，sync_compat 测试使用）。
-- 在 2.1.0 起的库上、执行 sync-sample-2.0.sql 之后执行：截止时间、便签颜色（含不认识的颜色）、
-- 手动排序、循环提醒的标签，以及两种月末循环模式。
-- 新版本新增同步列时不要改这里：为新版本另写一份夹具。

INSERT INTO tasks (id, description, type, status, created_at, completed_at, reminder_time,
                   updated_at, deleted_at, sticky_content, sticky_pos_x, sticky_pos_y,
                   sticky_width, sticky_height, sticky_is_open, sticky_is_pinned, tags, priority,
                   due_at, sticky_color, sort_order)
VALUES
  ('v21-task-due', '交季度报告', 'ONE_TIME', 'PENDING', '2026-10-01T09:00:00', NULL, '2026-10-16T17:00:00',
   '2026-10-01T09:00:00', NULL, '- [x] 初稿' || char(10) || '- [ ] 审阅', 200.0, 160.0, 284.0, 280.0, 1, 0, '工作', 3,
   '2026-10-16T18:00:00', 'blue', 1.5),
  ('v21-task-due-only', '只有截止时间', 'ONE_TIME', 'PENDING', '2026-10-01T09:00:00', NULL, NULL,
   '2026-10-01T09:00:00', NULL, '', 48.0, 76.0, 284.0, 280.0, 0, 0, '', 0,
   '2026-10-20T12:00:00', '', 2.0),
  ('v21-task-future-color', '更新版本才有的颜色', 'ONE_TIME', 'PENDING', '2026-10-01T09:00:00', NULL, NULL,
   '2026-10-01T09:00:00', NULL, '', 48.0, 76.0, 284.0, 280.0, 0, 0, '', 0,
   NULL, 'teal', NULL);

INSERT INTO recurring_tasks (id, description, type, status, created_at, completed_at, interval_minutes,
                             last_triggered, next_trigger, is_paused, start_time, end_time, repeat_mode,
                             schedule_time, schedule_weekday, schedule_day, cron_expression,
                             updated_at, deleted_at, schedule_weekdays, tags)
VALUES
  ('v21-rec-last-day', '交房租', 'RECURRING', 'PENDING', '2026-10-01T09:00:00', NULL, 60,
   NULL, '2026-10-31T20:00:00', 0, NULL, NULL, 'MONTHLY_LAST_DAY',
   '20:00', NULL, NULL, NULL, '2026-10-01T09:00:00', NULL, NULL, '家务'),
  ('v21-rec-last-workday', '提交报销', 'RECURRING', 'PENDING', '2026-10-01T09:00:00', NULL, 60,
   NULL, '2026-10-30T17:00:00', 0, NULL, NULL, 'MONTHLY_LAST_WORKDAY',
   '17:00', NULL, NULL, NULL, '2026-10-01T09:00:00', NULL, NULL, '工作,报销');

INSERT INTO reminder_records (id, reminder_id, description, type, trigger_time, close_time, action,
                              updated_at, deleted_at)
VALUES
  ('v21-record-1', 'v21-rec-last-day', '交房租', 'RECURRING', '2026-09-30T20:00:00', '2026-09-30T20:05:00',
   'COMPLETED', '2026-09-30T20:05:00', NULL);
