-- 跨版本同步测试的示例数据（库结构 2.2.5 新增的同步列，sync_compat 测试使用）。
-- 在 2.2.5 起的库上、执行 sync-sample-2.0 / 2.1 / 2.2 之后执行：循环提醒的临时提醒
-- （一条替代下一次常规提醒，一条在已结束、暂停的提醒上只多提醒一次）。
-- 新版本新增同步列时不要改这里：为新版本另写一份夹具。

UPDATE recurring_tasks
SET temp_trigger = '2026-10-05T08:45:00', temp_replaces = '2026-10-05T09:00:00'
WHERE id = 'v22-rec-count';

UPDATE recurring_tasks
SET temp_trigger = '2026-10-02T20:30:00', temp_replaces = NULL
WHERE id = 'v22-rec-ended';
