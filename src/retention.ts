// 已完成待办的保留期（本机设置，与后端 `models::COMPLETED_RETENTION_OPTIONS` 一致）。

export const DEFAULT_COMPLETED_RETENTION_DAYS = 30;

export const COMPLETED_RETENTION_OPTIONS: Array<{ value: number; label: string }> = [
  { value: 30, label: "30 天" },
  { value: 90, label: "90 天" },
  { value: 365, label: "1 年" },
  { value: 0, label: "永久" }
];

/** 不认识的值按默认处理（与后端相同）。 */
export const normalizeCompletedRetention = (value: number | null | undefined) =>
  COMPLETED_RETENTION_OPTIONS.some(item => item.value === value) ? (value as number) : DEFAULT_COMPLETED_RETENTION_DAYS;

export const completedRetentionLabel = (value: number | null | undefined) =>
  COMPLETED_RETENTION_OPTIONS.find(item => item.value === normalizeCompletedRetention(value))!.label;

/** 设置中的说明：默认档位的数量上限，以及同步时以最短的设备为准。 */
export const completedRetentionHint = (value: number | null | undefined, syncEnabled: boolean) => {
  const days = normalizeCompletedRetention(value);
  const base = days === DEFAULT_COMPLETED_RETENTION_DAYS ? "并只保留最近 100 条，" : "";
  const local = days === 0 ? "已完成的待办不会自动清理" : `完成超过 ${completedRetentionLabel(days)}的待办${base}自动移入回收站`;
  return syncEnabled
    ? `${local}；开启云同步时以所有设备中最短的为准（2.1.0 及更早的版本固定 30 天）`
    : local;
};

/**
 * 统计范围是否超出了已完成待办的保留期：超出时“完成数”与趋势不完整。
 * 永久保留时不会超出。
 */
export const retentionCoversRange = (retentionDays: number | null | undefined, rangeDays: number) => {
  const days = normalizeCompletedRetention(retentionDays);
  return days === 0 || days >= rangeDays;
};
