-- 迁移脚本: V1.4.5__merge_sticky_notes_into_tasks.sql
-- 描述: 便签与待办共用 tasks 表，迁移 sticky_notes 数据

ALTER TABLE tasks ADD COLUMN sticky_content TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN sticky_pos_x REAL NOT NULL DEFAULT 48;
ALTER TABLE tasks ADD COLUMN sticky_pos_y REAL NOT NULL DEFAULT 76;
ALTER TABLE tasks ADD COLUMN sticky_is_open INTEGER NOT NULL DEFAULT 0;

UPDATE tasks
SET
  description = COALESCE(
    (
      SELECT CASE
        WHEN sn.title IS NULL OR TRIM(sn.title) = '' THEN tasks.description
        ELSE sn.title
      END
      FROM sticky_notes sn
      WHERE sn.id = tasks.id AND sn.note_type = 'TASK'
      LIMIT 1
    ),
    description
  ),
  sticky_content = COALESCE(
    (
      SELECT sn.content
      FROM sticky_notes sn
      WHERE sn.id = tasks.id AND sn.note_type = 'TASK'
      LIMIT 1
    ),
    sticky_content
  ),
  sticky_pos_x = COALESCE(
    (
      SELECT sn.pos_x
      FROM sticky_notes sn
      WHERE sn.id = tasks.id AND sn.note_type = 'TASK'
      LIMIT 1
    ),
    sticky_pos_x
  ),
  sticky_pos_y = COALESCE(
    (
      SELECT sn.pos_y
      FROM sticky_notes sn
      WHERE sn.id = tasks.id AND sn.note_type = 'TASK'
      LIMIT 1
    ),
    sticky_pos_y
  ),
  sticky_is_open = COALESCE(
    (
      SELECT sn.is_open
      FROM sticky_notes sn
      WHERE sn.id = tasks.id AND sn.note_type = 'TASK'
      LIMIT 1
    ),
    sticky_is_open
  ),
  updated_at = COALESCE(
    (
      SELECT sn.updated_at
      FROM sticky_notes sn
      WHERE sn.id = tasks.id AND sn.note_type = 'TASK'
      LIMIT 1
    ),
    updated_at,
    created_at
  )
WHERE EXISTS (
  SELECT 1
  FROM sticky_notes sn
  WHERE sn.id = tasks.id AND sn.note_type = 'TASK'
);

INSERT INTO tasks (
  id, description, type, status, created_at, completed_at, reminder_time, updated_at, deleted_at,
  sticky_content, sticky_pos_x, sticky_pos_y, sticky_is_open
)
SELECT
  sn.id,
  CASE
    WHEN sn.title IS NULL OR TRIM(sn.title) = '' THEN '便签'
    ELSE sn.title
  END,
  'ONE_TIME',
  'PENDING',
  COALESCE(sn.created_at, datetime('now')),
  NULL,
  NULL,
  COALESCE(sn.updated_at, sn.created_at, datetime('now')),
  NULL,
  COALESCE(sn.content, ''),
  COALESCE(sn.pos_x, 48),
  COALESCE(sn.pos_y, 76),
  COALESCE(sn.is_open, 0)
FROM sticky_notes sn
WHERE sn.note_type = 'CUSTOM'
  AND NOT EXISTS (
    SELECT 1 FROM tasks t WHERE t.id = sn.id
  );

DROP TABLE IF EXISTS sticky_notes;

CREATE INDEX IF NOT EXISTS idx_tasks_sticky_is_open ON tasks(sticky_is_open);
