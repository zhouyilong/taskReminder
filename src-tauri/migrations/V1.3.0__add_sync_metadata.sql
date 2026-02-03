-- 迁移脚本: V1.3.0__add_sync_metadata.sql
-- 描述: 增加行级同步时间戳与 WebDAV 同步状态字段

ALTER TABLE tasks ADD COLUMN updated_at TEXT;
ALTER TABLE tasks ADD COLUMN deleted_at TEXT;

ALTER TABLE recurring_tasks ADD COLUMN updated_at TEXT;
ALTER TABLE recurring_tasks ADD COLUMN deleted_at TEXT;

ALTER TABLE reminder_records ADD COLUMN updated_at TEXT;
ALTER TABLE reminder_records ADD COLUMN deleted_at TEXT;

ALTER TABLE settings ADD COLUMN webdav_last_sync_status TEXT;
ALTER TABLE settings ADD COLUMN webdav_last_sync_error TEXT;

UPDATE tasks SET updated_at = COALESCE(updated_at, created_at);
UPDATE recurring_tasks SET updated_at = COALESCE(updated_at, created_at);
UPDATE reminder_records SET updated_at = COALESCE(updated_at, trigger_time);
