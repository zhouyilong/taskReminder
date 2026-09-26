// 每周多天位掩码：周一 = bit0 … 周日 = bit6（与后端 recurrence.rs 一致）。

export const WEEKDAY_LABELS = ["一", "二", "三", "四", "五", "六", "日"] as const;

export const WEEKDAY_MASK_WORKDAYS = 0b0011111;
export const WEEKDAY_MASK_WEEKEND = 0b1100000;
export const WEEKDAY_MASK_ALL = 0b1111111;

/** 周几（1 = 周一 … 7 = 周日）对应的位。 */
export const weekdayBit = (weekday: number) => 1 << (weekday - 1);

/** 读取任务的位掩码；旧数据只有单个 scheduleWeekday 时换算。 */
export const resolveWeekdayMask = (mask?: number | null, weekday?: number | null) => {
  if (mask && mask & WEEKDAY_MASK_ALL) {
    return mask & WEEKDAY_MASK_ALL;
  }
  if (weekday && weekday >= 1 && weekday <= 7) {
    return weekdayBit(weekday);
  }
  return 0;
};

/** 掩码中最早的一天，写入 scheduleWeekday 以兼容旧版本。 */
export const firstWeekday = (mask: number) => {
  for (let day = 1; day <= 7; day += 1) {
    if (mask & weekdayBit(day)) {
      return day;
    }
  }
  return null;
};

export const formatWeekdayMask = (mask: number) => {
  const normalized = mask & WEEKDAY_MASK_ALL;
  if (!normalized) {
    return "-";
  }
  if (normalized === WEEKDAY_MASK_ALL) {
    return "每天";
  }
  if (normalized === WEEKDAY_MASK_WORKDAYS) {
    return "周一至周五";
  }
  if (normalized === WEEKDAY_MASK_WEEKEND) {
    return "周末";
  }
  const days = WEEKDAY_LABELS.filter((_, index) => normalized & (1 << index));
  return `周${days.join("、")}`;
};
