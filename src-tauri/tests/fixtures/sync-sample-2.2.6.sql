-- 跨版本同步测试的示例数据（库结构 2.2.6 新增的同步列，sync_compat 测试使用）。
-- 在 2.2.6 起的库上、执行 sync-sample-2.0 / 2.1 / 2.2 / 2.2.5 之后执行：一条加急的待办。
-- 新版本新增同步列时不要改这里：为新版本另写一份夹具。

UPDATE tasks SET urgent = 1 WHERE id = 'v22-task-project';
