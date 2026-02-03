-- 迁移脚本: V1.1.0__initial_schema.sql
-- 描述: 初始数据库结构，包含任务表、周期性任务表、设置表和提醒记录表

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    type TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    completed_at TEXT,
    reminder_time TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_reminder_time ON tasks(reminder_time);

CREATE TABLE IF NOT EXISTS recurring_tasks (
    id TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    type TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    completed_at TEXT,
    interval_minutes INTEGER NOT NULL,
    last_triggered TEXT,
    next_trigger TEXT NOT NULL,
    is_paused INTEGER NOT NULL DEFAULT 0,
    start_time TEXT,
    end_time TEXT
);

CREATE INDEX IF NOT EXISTS idx_recurring_tasks_next_trigger ON recurring_tasks(next_trigger);
CREATE INDEX IF NOT EXISTS idx_recurring_tasks_is_paused ON recurring_tasks(is_paused);

CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    auto_start_enabled INTEGER NOT NULL DEFAULT 0,
    sound_enabled INTEGER NOT NULL DEFAULT 1,
    snooze_minutes INTEGER NOT NULL DEFAULT 5
);

INSERT OR IGNORE INTO settings (id, auto_start_enabled, sound_enabled, snooze_minutes)
VALUES (1, 0, 1, 5);

CREATE TABLE IF NOT EXISTS reminder_records (
    id TEXT PRIMARY KEY,
    reminder_id TEXT NOT NULL,
    description TEXT NOT NULL,
    type TEXT NOT NULL,
    trigger_time TEXT NOT NULL,
    close_time TEXT,
    action TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reminder_records_trigger_time ON reminder_records(trigger_time);
CREATE INDEX IF NOT EXISTS idx_reminder_records_type ON reminder_records(type);
CREATE INDEX IF NOT EXISTS idx_reminder_records_reminder_id ON reminder_records(reminder_id);
