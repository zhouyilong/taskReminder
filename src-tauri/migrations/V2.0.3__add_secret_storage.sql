-- 迁移脚本: V2.0.3__add_secret_storage.sql
-- 描述: 记录 WebDAV 密码与同步密码的存放位置：db（本地数据库）或 keyring（Windows 凭据管理器，数据库中留空）

ALTER TABLE settings ADD COLUMN secret_storage TEXT NOT NULL DEFAULT 'db';
