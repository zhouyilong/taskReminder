-- 迁移脚本: V2.1.0__add_due_color_order_recurring_tags.sql
-- 描述: v2.1 新增的同步列（均可空或带默认值，2.0.3 起的设备合并时会原样保留）：
--   tasks.due_at          截止时间（提醒时间仍存 reminder_time，= 截止时间 - 提前量）
--   tasks.sticky_color    便签颜色（空 = 默认颜色）
--   tasks.sort_order      手动排序位置（空 = 未手动排序）
--   recurring_tasks.tags  循环提醒的标签（逗号分隔，与待办相同的规范化）

ALTER TABLE tasks ADD COLUMN due_at TEXT;
ALTER TABLE tasks ADD COLUMN sticky_color TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN sort_order REAL;
ALTER TABLE recurring_tasks ADD COLUMN tags TEXT NOT NULL DEFAULT '';
