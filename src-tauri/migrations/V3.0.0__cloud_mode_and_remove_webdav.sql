-- v3.0.0：正式移除 WebDAV 同步，引入本地/云端双模式与账号镜像库同步状态表。
-- 历史 webdav_* 与 sync_passphrase 列不执行 DROP COLUMN 以保持旧 SQLite 兼容，
-- 仅清空历史敏感字段并默认保持本地模式（零数据丢失）。

UPDATE settings
SET webdav_enabled = 0,
    webdav_password = '',
    sync_passphrase = ''
WHERE id = 1;

ALTER TABLE settings ADD COLUMN app_mode TEXT NOT NULL DEFAULT 'local';
ALTER TABLE settings ADD COLUMN cloud_server_url TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN cloud_user_id TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN cloud_username TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN cloud_nickname TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN cloud_user_role TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN cloud_must_change_password INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN cloud_refresh_token TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN last_merged_account_id TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN last_merged_username TEXT NOT NULL DEFAULT '';

-- 账号镜像库本地同步状态（每个数据库独立一行）
CREATE TABLE IF NOT EXISTS cloud_sync_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_server_seq INTEGER NOT NULL DEFAULT 0,
    last_sync_time TEXT,
    last_sync_status TEXT,
    last_sync_error TEXT,
    clock_offset_ms INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO cloud_sync_state (id, last_server_seq, clock_offset_ms)
VALUES (1, 0, 0);

-- 云端模式待推送变更队列（本地写入后记录，收到服务端 ACK 后移除）
CREATE TABLE IF NOT EXISTS pending_mutations (
    table_name TEXT NOT NULL,
    row_id TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (table_name, row_id)
);
