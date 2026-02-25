-- 迁移脚本: V1.4.6__add_sticky_note_opacity.sql
-- 描述: 增加桌面便签透明度配置字段

ALTER TABLE settings ADD COLUMN sticky_note_opacity REAL NOT NULL DEFAULT 0.95;
