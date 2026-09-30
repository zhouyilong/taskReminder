-- 迁移脚本: V2.0.5__add_holiday_auto_update.sql
-- 描述: 节假日数据在线更新开关（本机设置，不参与同步），默认开启

ALTER TABLE settings ADD COLUMN holiday_auto_update INTEGER NOT NULL DEFAULT 1;
