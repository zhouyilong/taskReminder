-- 迁移脚本: V2.2.2__add_attention_intensity.sql
-- 描述: 全屏提醒效果的光晕强度（本机设置，不参与同步）：
--   attention_intensity  1 柔和 / 2 标准（默认，即 2.2.1 的效果）/ 3 较强 / 4 强烈 / 5 最强；越强光晕越宽、越向屏幕中间延伸

ALTER TABLE settings ADD COLUMN attention_intensity INTEGER NOT NULL DEFAULT 2;
