-- 迁移脚本: V1.4.1__add_recurring_modes.sql
-- 描述: 为循环提醒增加多种循环模式字段（每日/每周/每月/Cron）

ALTER TABLE recurring_tasks ADD COLUMN repeat_mode TEXT NOT NULL DEFAULT 'INTERVAL_RANGE';
ALTER TABLE recurring_tasks ADD COLUMN schedule_time TEXT;
ALTER TABLE recurring_tasks ADD COLUMN schedule_weekday INTEGER;
ALTER TABLE recurring_tasks ADD COLUMN schedule_day INTEGER;
ALTER TABLE recurring_tasks ADD COLUMN cron_expression TEXT;

UPDATE recurring_tasks
SET repeat_mode = 'INTERVAL_RANGE'
WHERE repeat_mode IS NULL OR TRIM(repeat_mode) = '';
