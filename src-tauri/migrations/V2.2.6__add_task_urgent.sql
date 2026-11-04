-- 迁移脚本: V2.2.6__add_task_urgent.sql
-- 描述:
--   tasks.urgent  待办是否加急（同步列，0 / 1，v2.2.5）。待办清单便签与列表中醒目标识。
-- 带字面量默认值，2.0.3 起的设备合并时会原样保留；旧版本不认识加急，只是不显示标识。

ALTER TABLE tasks ADD COLUMN urgent INTEGER NOT NULL DEFAULT 0;
