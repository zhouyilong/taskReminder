-- 迁移脚本: V2.0.0__add_task_tags_and_priority.sql
-- 描述: 待办新增标签（逗号分隔，已规范化）与优先级（0 无 / 1 低 / 2 中 / 3 高）

ALTER TABLE tasks ADD COLUMN tags TEXT NOT NULL DEFAULT '';

ALTER TABLE tasks ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority);
