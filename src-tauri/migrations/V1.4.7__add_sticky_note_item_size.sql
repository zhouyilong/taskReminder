-- 迁移脚本: V1.4.7__add_sticky_note_item_size.sql
-- 描述: 为每张便签增加宽高持久化字段

ALTER TABLE tasks ADD COLUMN sticky_width REAL NOT NULL DEFAULT 284;
ALTER TABLE tasks ADD COLUMN sticky_height REAL NOT NULL DEFAULT 280;
