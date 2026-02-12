-- 迁移脚本: V1.4.3__add_sticky_notes_table.sql
-- 描述: 增加任务便签表，用于按待办管理便签内容与位置

CREATE TABLE IF NOT EXISTS sticky_notes (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL DEFAULT '',
    pos_x REAL NOT NULL DEFAULT 48,
    pos_y REAL NOT NULL DEFAULT 76,
    is_open INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sticky_notes_updated_at ON sticky_notes(updated_at);
CREATE INDEX IF NOT EXISTS idx_sticky_notes_is_open ON sticky_notes(is_open);
