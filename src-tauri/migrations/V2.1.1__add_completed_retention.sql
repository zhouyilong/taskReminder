-- 迁移脚本: V2.1.1__add_completed_retention.sql
-- 描述: 已完成待办的保留天数（本机设置，不参与同步）。30 为默认（与之前相同，并保留最近 100 条上限），
--       90 / 365 为对应天数，0 为永久保留；其他值按 30 处理。

ALTER TABLE settings ADD COLUMN completed_retention_days INTEGER NOT NULL DEFAULT 30;
