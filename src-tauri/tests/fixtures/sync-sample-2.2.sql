-- 跨版本同步测试的示例数据（v2.2 新增的同步列，sync_compat 测试使用）。
-- 在 2.2.0 起的库上、执行 sync-sample-2.0.sql 与 sync-sample-2.1.sql 之后执行：待办项目、
-- 循环提醒的结束日期与剩余次数（含一条已到结束条件、写入暂停的提醒）。
-- 新版本新增同步列时不要改这里：为新版本另写一份夹具。

INSERT INTO tasks (id, description, type, status, created_at, completed_at, reminder_time,
                   updated_at, deleted_at, sticky_content, sticky_pos_x, sticky_pos_y,
                   sticky_width, sticky_height, sticky_is_open, sticky_is_pinned, tags, priority,
                   due_at, sticky_color, sort_order, project)
VALUES
  ('v22-task-project', '买瓷砖', 'ONE_TIME', 'PENDING', '2026-10-03T09:00:00', NULL, '2026-10-10T10:00:00',
   '2026-10-03T09:00:00', NULL, '', 48.0, 76.0, 284.0, 280.0, 0, 0, '采购', 2,
   NULL, '', NULL, '装修'),
  ('v22-task-done-project', '量尺寸', 'ONE_TIME', 'COMPLETED', '2026-10-01T09:00:00', '2026-10-02T18:00:00', NULL,
   '2026-10-02T18:00:00', NULL, '', 48.0, 76.0, 284.0, 280.0, 0, 0, '', 0,
   NULL, '', NULL, '装修');

INSERT INTO recurring_tasks (id, description, type, status, created_at, completed_at, interval_minutes,
                             last_triggered, next_trigger, is_paused, start_time, end_time, repeat_mode,
                             schedule_time, schedule_weekday, schedule_day, cron_expression,
                             updated_at, deleted_at, schedule_weekdays, tags, ends_on, remaining_count)
VALUES
  ('v22-rec-until', '吃药', 'RECURRING', 'PENDING', '2026-10-01T09:00:00', NULL, 60,
   '2026-10-03T08:00:00', '2026-10-04T08:00:00', 0, NULL, NULL, 'DAILY',
   '08:00', NULL, NULL, NULL, '2026-10-03T08:00:00', NULL, NULL, '健康', '2026-10-14', NULL),
  ('v22-rec-count', '复诊', 'RECURRING', 'PENDING', '2026-10-01T09:00:00', NULL, 60,
   NULL, '2026-10-05T09:00:00', 0, NULL, NULL, 'WEEKLY',
   '09:00', 1, NULL, NULL, '2026-10-01T09:00:00', NULL, 1, '', NULL, 3),
  ('v22-rec-ended', '训练营打卡', 'RECURRING', 'PENDING', '2026-09-01T09:00:00', NULL, 60,
   '2026-09-30T21:00:00', '2026-10-01T21:00:00', 1, NULL, NULL, 'DAILY',
   '21:00', NULL, NULL, NULL, '2026-09-30T21:00:00', NULL, NULL, '', NULL, 0);
