// 提醒记录的一键清理（v2.2.8）：按“半年前的当天零点”计算截止时间，统计将删除的条数。
import { toLocalDateTimeString } from "./format";

/** 默认清理多少个月之前的记录。 */
export const RECORD_CLEANUP_MONTHS = 6;

/**
 * `months` 个月前那一天的零点（本地时间，`YYYY-MM-DDT00:00:00`）。
 * 目标月份没有这一天时取该月最后一天（如 8 月 31 日的半年前是 2 月 28 / 29 日）。
 */
export const monthsAgoCutoff = (now: Date, months = RECORD_CLEANUP_MONTHS): string => {
  const target = new Date(now.getFullYear(), now.getMonth() - months, 1);
  const lastDay = new Date(target.getFullYear(), target.getMonth() + 1, 0).getDate();
  target.setDate(Math.min(now.getDate(), lastDay));
  return toLocalDateTimeString(target);
};

/** 触发时间早于截止时间的记录（与后端 `delete_reminder_records_before` 的比较一致）。 */
export const recordsBefore = <T extends { triggerTime: string }>(records: readonly T[], cutoff: string): T[] =>
  records.filter(record => record.triggerTime < cutoff);
