-- 迁移脚本: V1.2.0__add_webdav_settings.sql
-- 描述: 增加 WebDAV 云同步相关设置字段

ALTER TABLE settings ADD COLUMN webdav_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN webdav_url TEXT;
ALTER TABLE settings ADD COLUMN webdav_username TEXT;
ALTER TABLE settings ADD COLUMN webdav_password TEXT;
ALTER TABLE settings ADD COLUMN webdav_root_path TEXT;
ALTER TABLE settings ADD COLUMN webdav_sync_interval_minutes INTEGER NOT NULL DEFAULT 60;
ALTER TABLE settings ADD COLUMN webdav_last_sync_time TEXT;
ALTER TABLE settings ADD COLUMN webdav_last_local_change_time TEXT;
ALTER TABLE settings ADD COLUMN webdav_device_id TEXT;

UPDATE settings
SET webdav_device_id = COALESCE(webdav_device_id, lower(hex(randomblob(16))))
WHERE id = 1;
