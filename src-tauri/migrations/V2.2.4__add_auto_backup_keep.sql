-- 迁移脚本: V2.2.4__add_auto_backup_keep.sql
-- 描述: 每日自动备份保留的份数（本机设置，不参与同步）。可选 7（默认，与之前相同）/ 14 / 30，其他值按 7 处理。

ALTER TABLE settings ADD COLUMN auto_backup_keep INTEGER NOT NULL DEFAULT 7;
