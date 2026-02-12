-- 迁移脚本: V1.4.4__add_sticky_note_meta.sql
-- 描述: 为便签增加标题与来源类型（TASK/CUSTOM）

ALTER TABLE sticky_notes ADD COLUMN title TEXT NOT NULL DEFAULT '';
ALTER TABLE sticky_notes ADD COLUMN note_type TEXT NOT NULL DEFAULT 'TASK';

UPDATE sticky_notes
SET title = CASE
    WHEN title IS NULL OR TRIM(title) = '' THEN '便签'
    ELSE title
END;

UPDATE sticky_notes
SET note_type = CASE
    WHEN note_type IS NULL OR TRIM(note_type) = '' THEN 'TASK'
    ELSE note_type
END;
