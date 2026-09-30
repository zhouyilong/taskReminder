-- 迁移脚本: V2.0.4__add_sticky_snap.sql
-- 描述: 便签贴边吸附开关（本机设置，不参与同步），默认开启

ALTER TABLE settings ADD COLUMN sticky_snap_enabled INTEGER NOT NULL DEFAULT 1;
