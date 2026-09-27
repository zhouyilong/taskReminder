-- 迁移脚本: V2.0.1__add_sync_encryption.sql
-- 描述: 云同步端到端加密开关与同步密码（仅存本机，上传的快照中会被清空）

ALTER TABLE settings ADD COLUMN sync_encryption_enabled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE settings ADD COLUMN sync_passphrase TEXT NOT NULL DEFAULT '';
