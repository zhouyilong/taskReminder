-- 迁移脚本: V2.2.5__add_recurring_temp_and_todo_sticky.sql
-- 描述:
--   recurring_tasks.temp_trigger   临时提醒时间（同步列，空 = 没有临时提醒）；到点提醒一次后清空。
--   recurring_tasks.temp_replaces  临时提醒替代的那一次常规提醒的原定时间（同步列，空 = 不替代、只多提醒一次）。
--   settings.todo_sticky_state     待办清单便签的窗口状态（本机设置，JSON：是否打开、位置、尺寸、是否锚定）。
-- 两个同步列都可空，2.0.3 起的设备合并时会原样保留；旧版本不认识临时提醒，会照常在原定时间提醒。

ALTER TABLE recurring_tasks ADD COLUMN temp_trigger TEXT;
ALTER TABLE recurring_tasks ADD COLUMN temp_replaces TEXT;
ALTER TABLE settings ADD COLUMN todo_sticky_state TEXT NOT NULL DEFAULT '';
