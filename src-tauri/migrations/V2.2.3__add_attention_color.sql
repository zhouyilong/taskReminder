-- 迁移脚本: V2.2.3__add_attention_color.sql
-- 描述: 全屏提醒效果的光晕颜色（本机设置，不参与同步）：
--   attention_color  #rrggbb，空字符串为默认颜色（跟随主题的蓝色）；错过的提醒仍用警示橙色

ALTER TABLE settings ADD COLUMN attention_color TEXT NOT NULL DEFAULT '';
