-- 迁移脚本: V1.4.8__add_window_opacity.sql
-- 描述: 增加主窗口整体透明度配置字段

ALTER TABLE settings ADD COLUMN window_opacity REAL NOT NULL DEFAULT 1.0;
