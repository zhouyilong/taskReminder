-- 迁移脚本: V1.4.0__add_notification_theme.sql
-- 描述: 增加提醒弹窗主题设置字段

ALTER TABLE settings ADD COLUMN notification_theme TEXT NOT NULL DEFAULT 'app';
