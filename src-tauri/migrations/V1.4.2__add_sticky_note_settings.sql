-- 迁移脚本: V1.4.2__add_sticky_note_settings.sql
-- 描述: 增加桌面便签配置字段

ALTER TABLE settings ADD COLUMN sticky_note_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN sticky_note_content TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN sticky_note_width INTEGER NOT NULL DEFAULT 360;
ALTER TABLE settings ADD COLUMN sticky_note_height INTEGER NOT NULL DEFAULT 520;
ALTER TABLE settings ADD COLUMN sticky_note_x REAL;
ALTER TABLE settings ADD COLUMN sticky_note_y REAL;
