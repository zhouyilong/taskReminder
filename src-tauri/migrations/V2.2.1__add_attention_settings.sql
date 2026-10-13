-- 迁移脚本: V2.2.1__add_attention_settings.sql
-- 描述: 提醒触达（本机设置，不参与同步）：
--   attention_effect          全屏注意效果：off 关闭 / edge 边缘光晕 / banner 光晕 + 顶部横幅（默认）
--   attention_until_handled   效果持续到提醒处理完（0 = 约 6 秒后淡出）
--   attention_repeat_minutes  提醒未处理时每隔几分钟再次提醒（0 = 不重复）
--   notification_screen       提醒弹窗出现的屏幕：cursor 鼠标所在屏幕（默认）/ primary 主屏幕

ALTER TABLE settings ADD COLUMN attention_effect TEXT NOT NULL DEFAULT 'banner';

ALTER TABLE settings ADD COLUMN attention_until_handled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE settings ADD COLUMN attention_repeat_minutes INTEGER NOT NULL DEFAULT 0;

ALTER TABLE settings ADD COLUMN notification_screen TEXT NOT NULL DEFAULT 'cursor';
