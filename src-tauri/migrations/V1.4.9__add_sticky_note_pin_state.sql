-- 迁移脚本: V1.4.9__add_sticky_note_pin_state.sql
-- 描述: 为便签增加锚定状态持久化字段

ALTER TABLE tasks ADD COLUMN sticky_is_pinned INTEGER NOT NULL DEFAULT 0;
