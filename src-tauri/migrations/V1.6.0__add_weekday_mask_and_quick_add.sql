-- 迁移脚本: V1.6.0__add_weekday_mask_and_quick_add.sql
-- 描述: 每周模式支持多天（位掩码，周一 = bit0 … 周日 = bit6）；新增快速添加全局快捷键设置

ALTER TABLE recurring_tasks ADD COLUMN schedule_weekdays INTEGER;

UPDATE recurring_tasks
SET schedule_weekdays = (1 << (schedule_weekday - 1))
WHERE repeat_mode = 'WEEKLY' AND schedule_weekday BETWEEN 1 AND 7;

ALTER TABLE settings ADD COLUMN quick_add_enabled INTEGER NOT NULL DEFAULT 1;

ALTER TABLE settings ADD COLUMN quick_add_shortcut TEXT NOT NULL DEFAULT 'CommandOrControl+Alt+N';
