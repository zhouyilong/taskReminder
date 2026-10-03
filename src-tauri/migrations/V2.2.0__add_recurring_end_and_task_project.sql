-- 迁移脚本: V2.2.0__add_recurring_end_and_task_project.sql
-- 描述: v2.2 新增的同步列（均可空或带默认值，2.0.3 起的设备合并时会原样保留）：
--   recurring_tasks.ends_on          结束日期（YYYY-MM-DD，含当天；空 = 不限）
--   recurring_tasks.remaining_count  剩余提醒次数（空 = 不限；到 0 时结束）
--   tasks.project                    项目名（空 = 未分组）
-- 到达结束条件时同时写 is_paused = 1，不认识这两列的旧版本也会停止提醒。

ALTER TABLE recurring_tasks ADD COLUMN ends_on TEXT;
ALTER TABLE recurring_tasks ADD COLUMN remaining_count INTEGER;
ALTER TABLE tasks ADD COLUMN project TEXT NOT NULL DEFAULT '';
