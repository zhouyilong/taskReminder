-- 迁移脚本: V2.0.2__add_quiet_hours_notification_and_sticky_shortcut.sql
-- 描述: 勿扰时段、系统原生通知开关、显示/隐藏全部便签的全局快捷键（均为本机设置，不参与同步）
-- 注: V2.0.1 迁移已随应用 2.0.0 发布，迁移版本号按数字比较，所以应用 2.0.1 的迁移从 2.0.2 开始编号。

ALTER TABLE settings ADD COLUMN quiet_hours_enabled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE settings ADD COLUMN quiet_hours_start TEXT NOT NULL DEFAULT '22:00';

ALTER TABLE settings ADD COLUMN quiet_hours_end TEXT NOT NULL DEFAULT '08:00';

ALTER TABLE settings ADD COLUMN native_notification_enabled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE settings ADD COLUMN sticky_toggle_shortcut TEXT NOT NULL DEFAULT '';
