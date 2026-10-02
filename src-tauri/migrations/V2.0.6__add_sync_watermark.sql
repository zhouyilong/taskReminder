-- 迁移脚本: V2.0.6__add_sync_watermark.sql
-- 描述: 同步水位（本机专用，不参与合并，上传的快照中清空）：上次成功上传的时间、远端身份，
--       以及那份快照中每一行的 id。用于识别“超过墓碑保留期后已在其他设备删除并清理”的行。

CREATE TABLE IF NOT EXISTS sync_watermark (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    uploaded_at TEXT NOT NULL,
    remote TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_watermark_rows (
    table_name TEXT NOT NULL,
    id TEXT NOT NULL,
    PRIMARY KEY (table_name, id)
) WITHOUT ROWID;
