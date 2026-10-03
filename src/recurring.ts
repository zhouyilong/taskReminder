// 循环提醒的规则展示、表单草稿、校验与提交载荷，新建与编辑共用。
import { normalizeTags } from "./tasks";
import type { RecurringMode, RecurringTask } from "./types";
import {
  WEEKDAY_MASK_ALL,
  WEEKDAY_MASK_WORKDAYS,
  firstWeekday,
  formatWeekdayMask,
  resolveWeekdayMask
} from "./weekdays";

export const recurringModeOptions: { value: RecurringMode; label: string }[] = [
  { value: "INTERVAL_RANGE", label: "区间间隔" },
  { value: "DAILY", label: "每天固定时间" },
  { value: "WORKDAY", label: "法定工作日" },
  { value: "WEEKLY", label: "每周固定时间" },
  { value: "MONTHLY", label: "每月固定时间" },
  { value: "MONTHLY_LAST_DAY", label: "每月最后一天" },
  { value: "MONTHLY_LAST_WORKDAY", label: "每月最后一个工作日" },
  { value: "CRON", label: "Cron 表达式" },
];

/**
 * 本版本是否认识这个循环模式。不认识的模式来自更新版本的设备：后端不触发也不改写，
 * 界面上标为“需升级”，也不允许编辑（编辑表单会把它改成某个已知模式，覆盖新版本的规则）。
 */
export const isSupportedRecurringMode = (mode?: string | null) =>
  recurringModeOptions.some(item => item.value === mode);

/** 重复次数上限，与后端 MAX_REPEAT_COUNT 一致。 */
export const MAX_REPEAT_COUNT = 9999;

const datePart = (value?: string | null) => (value ? value.slice(0, 10) : "");

/**
 * 是否已到结束条件（v2.2，与后端 has_ended 一致）：剩余次数用完，或下次触发晚于结束日期当天。
 * 已结束的提醒同时是暂停状态（旧版本也会停止提醒），界面上显示为“已结束”。
 */
export const isRecurringEnded = (task: Pick<RecurringTask, "remainingCount" | "endsOn" | "nextTrigger">) => {
  if (typeof task.remainingCount === "number" && task.remainingCount <= 0) {
    return true;
  }
  const end = datePart(task.endsOn);
  const next = datePart(task.nextTrigger);
  return Boolean(end && next && next > end);
};

/** 结束日期显示为“12月31日”，不是今年的带上年份。 */
const formatEndDate = (value: string, now: Date) => {
  const [year, month, day] = value.split("-").map(Number);
  if (!year || !month || !day) {
    return value;
  }
  return `${year === now.getFullYear() ? "" : `${year}年`}${month}月${day}日`;
};

/** 结束条件的简短说明，如“到 12月31日”“剩 3 次”；没有结束条件时返回空字符串。 */
export const formatRecurringEnd = (
  task: Pick<RecurringTask, "remainingCount" | "endsOn">,
  now = new Date()
) => {
  const parts: string[] = [];
  const end = datePart(task.endsOn);
  if (end) {
    parts.push(`到 ${formatEndDate(end, now)}`);
  }
  if (typeof task.remainingCount === "number") {
    parts.push(`剩 ${Math.max(0, task.remainingCount)} 次`);
  }
  return parts.join("，");
};

export type RecurringStatus = "unsupported" | "ended" | "paused" | "running";

export const recurringStatus = (
  task: Pick<RecurringTask, "repeatMode" | "isPaused" | "remainingCount" | "endsOn" | "nextTrigger">
): RecurringStatus => {
  if (!isSupportedRecurringMode(task.repeatMode)) return "unsupported";
  if (task.isPaused && isRecurringEnded(task)) return "ended";
  return task.isPaused ? "paused" : "running";
};

export const RECURRING_STATUS_LABELS: Record<RecurringStatus, string> = {
  unsupported: "需升级",
  ended: "已结束",
  paused: "已暂停",
  running: "运行中",
};

/** 能否“跳过本次”：运行中、本机认识的模式，且有下次触发时间。 */
export const canSkipRecurring = (task: Pick<RecurringTask, "isPaused" | "repeatMode" | "nextTrigger">) =>
  !task.isPaused && isSupportedRecurringMode(task.repeatMode) && Boolean(task.nextTrigger);

export const formatRecurringMode = (mode?: RecurringMode | string | null) => {
  // 空值按区间间隔（旧数据的默认值）；其他不认识的模式不能冒充区间间隔。
  if (!mode) {
    return "区间间隔";
  }
  const resolved = recurringModeOptions.find(item => item.value === mode);
  return resolved ? resolved.label : "不支持的模式";
};

export const formatRecurringRule = (task: RecurringTask) => {
  switch (task.repeatMode) {
    case "DAILY":
      return `每天 ${task.scheduleTime || "-"}`;
    case "WEEKLY":
      return `${formatWeekdayMask(resolveWeekdayMask(task.scheduleWeekdays, task.scheduleWeekday))} ${task.scheduleTime || "-"}`;
    case "WORKDAY":
      return `法定工作日 ${task.scheduleTime || "-"}`;
    case "MONTHLY":
      return `每月 ${task.scheduleDay || "-"} 日 ${task.scheduleTime || "-"}`;
    case "MONTHLY_LAST_DAY":
      return `每月最后一天 ${task.scheduleTime || "-"}`;
    case "MONTHLY_LAST_WORKDAY":
      return `每月最后一个工作日 ${task.scheduleTime || "-"}`;
    case "CRON":
      return task.cronExpression || "-";
    case "INTERVAL_RANGE": {
      const start = task.startTime || "00:00";
      const end = task.endTime || "23:59";
      return `每 ${task.intervalMinutes} 分钟（${start} - ${end}）`;
    }
    default:
      return `不支持的循环模式（${task.repeatMode}），请升级应用`;
  }
};

/** 依赖法定节假日数据的模式（法定工作日、每月最后一个工作日）。 */
export const dependsOnHolidays = (mode?: string | null) => mode === "WORKDAY" || mode === "MONTHLY_LAST_WORKDAY";

/** 节假日数据覆盖的年份范围，如“2025–2027 年”；为空时返回“暂无”。 */
export const formatYearRange = (years: number[]) => {
  if (!years.length) {
    return "暂无";
  }
  const sorted = [...years].sort((a, b) => a - b);
  return sorted.length > 1 ? `${sorted[0]}–${sorted[sorted.length - 1]} 年` : `${sorted[0]} 年`;
};

export const formatWorkdayHint = (years: number[]) => {
  // 数据可能来自内置或在线更新，统一说“已有”。
  const coverage = years.length
    ? `已有 ${formatYearRange(years)}安排，其他年份按周一至周五`
    : "暂无节假日数据，按周一至周五";
  return `跳过法定节假日，调休上班日照常提醒（${coverage}）`;
};

export const formatMonthEndWorkdayHint = (years: number[]) => {
  const coverage = years.length ? `已有 ${formatYearRange(years)}安排` : "暂无节假日数据，按周一至周五";
  return `月末遇周末或法定节假日时提前到之前最近的工作日（${coverage}）`;
};

/** 法定工作日提醒提前多少天提示节假日数据缺失。 */
export const HOLIDAY_LOOKAHEAD_DAYS = 30;

/**
 * 从 `from` 起 `lookaheadDays` 天内第一个没有内置节假日数据的年份；都已覆盖时返回 null。
 * `years` 为空视为数据尚未加载，不提示。
 */
export const firstUncoveredHolidayYear = (
  from: Date,
  years: number[],
  lookaheadDays = HOLIDAY_LOOKAHEAD_DAYS
): number | null => {
  if (!years.length || Number.isNaN(from.getTime())) {
    return null;
  }
  const until = new Date(from.getTime());
  until.setDate(until.getDate() + lookaheadDays);
  for (let year = from.getFullYear(); year <= until.getFullYear(); year += 1) {
    if (!years.includes(year)) {
      return year;
    }
  }
  return null;
};

const holidayWarningText = (year: number) => `${year} 年节假日安排尚未内置，暂按周一至周五计算`;

/** 运行中的法定工作日提醒在下次触发后 30 天内遇到未内置的年份时返回提示文案。 */
export const workdayHolidayWarning = (
  task: Pick<RecurringTask, "repeatMode" | "isPaused" | "nextTrigger">,
  years: number[],
  now = new Date()
): string | null => {
  if (!dependsOnHolidays(task.repeatMode) || task.isPaused) {
    return null;
  }
  const next = task.nextTrigger ? new Date(task.nextTrigger) : now;
  const from = Number.isNaN(next.getTime()) || next < now ? now : next;
  const year = firstUncoveredHolidayYear(from, years);
  return year === null ? null : holidayWarningText(year);
};

/** 新建或编辑法定工作日提醒时，从现在起 30 天内遇到未内置的年份时返回提示文案。 */
export const workdayDraftWarning = (years: number[], now = new Date()): string | null => {
  const year = firstUncoveredHolidayYear(now, years);
  return year === null ? null : holidayWarningText(year);
};

export interface RecurringDraft {
  description: string;
  mode: RecurringMode;
  intervalMinutes: number;
  startTime: string;
  endTime: string;
  scheduleTime: string;
  scheduleWeekdays: number;
  scheduleDay: number;
  cronExpression: string;
  /** 标签（v2.1）。 */
  tags: string[];
  /** 结束日期（v2.2，YYYY-MM-DD），空为不限。 */
  endsOn: string;
  /** 次数（v2.2）：新建时为“共几次”，编辑时为“还剩几次”；null 为不限。 */
  count: number | null;
}

export const createRecurringDraft = (): RecurringDraft => ({
  description: "",
  mode: "INTERVAL_RANGE",
  intervalMinutes: 60,
  startTime: "08:00",
  endTime: "17:30",
  scheduleTime: "09:00",
  scheduleWeekdays: WEEKDAY_MASK_WORKDAYS,
  scheduleDay: 1,
  cronExpression: "0 9 * * *",
  tags: [],
  endsOn: "",
  count: null,
});

export const draftFromRecurringTask = (task: RecurringTask): RecurringDraft => ({
  description: task.description,
  mode: (task.repeatMode || "INTERVAL_RANGE") as RecurringMode,
  intervalMinutes: task.intervalMinutes,
  startTime: task.startTime ?? "",
  endTime: task.endTime ?? "",
  scheduleTime: task.scheduleTime ?? "09:00",
  scheduleWeekdays:
    resolveWeekdayMask(task.scheduleWeekdays, task.scheduleWeekday) || WEEKDAY_MASK_WORKDAYS,
  scheduleDay: task.scheduleDay ?? 1,
  cronExpression: task.cronExpression ?? "",
  tags: [...(task.tags ?? [])],
  endsOn: datePart(task.endsOn),
  count: typeof task.remainingCount === "number" ? task.remainingCount : null,
});

/** 次数输入框清空时 v-model.number 给出空字符串，统一成 null。 */
const normalizeCount = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;

/** 结束条件的校验；通过时返回 null。 */
const validateRecurringEnd = (draft: RecurringDraft): string | null => {
  if (draft.endsOn && !/^\d{4}-\d{2}-\d{2}$/.test(draft.endsOn)) {
    return "结束日期格式无效";
  }
  const count = normalizeCount(draft.count);
  if (count !== null && (!Number.isInteger(count) || count < 0 || count > MAX_REPEAT_COUNT)) {
    return `次数需为 0 到 ${MAX_REPEAT_COUNT} 之间的整数`;
  }
  return null;
};

/** 返回错误提示；通过校验时返回 null。 */
export const validateRecurringDraft = (draft: RecurringDraft): string | null =>
  validateRecurringRule(draft) ?? validateRecurringEnd(draft);

const validateRecurringRule = (draft: RecurringDraft): string | null => {
  switch (draft.mode) {
    case "INTERVAL_RANGE":
      if (!Number.isFinite(draft.intervalMinutes) || draft.intervalMinutes < 1) {
        return "间隔分钟数必须大于 0";
      }
      if (draft.startTime && draft.endTime && draft.startTime > draft.endTime) {
        return "开始时间不能晚于结束时间";
      }
      return null;
    case "DAILY":
      return draft.scheduleTime ? null : "每日模式需要选择触发时间";
    case "WEEKLY":
      if (!draft.scheduleTime) {
        return "每周模式需要选择触发时间";
      }
      if (!(draft.scheduleWeekdays & WEEKDAY_MASK_ALL)) {
        return "每周模式至少需要选择一天";
      }
      return null;
    case "WORKDAY":
      return draft.scheduleTime ? null : "工作日模式需要选择触发时间";
    case "MONTHLY_LAST_DAY":
    case "MONTHLY_LAST_WORKDAY":
      return draft.scheduleTime ? null : "每月最后一天模式需要选择触发时间";
    case "MONTHLY":
      if (!draft.scheduleTime) {
        return "每月模式需要选择触发时间";
      }
      if (draft.scheduleDay < 1 || draft.scheduleDay > 31) {
        return "每月模式中的几号必须在 1 到 31 之间";
      }
      return null;
    case "CRON":
      return draft.cronExpression.trim() ? null : "Cron 表达式不能为空";
    default:
      return "未知的循环模式";
  }
};

export const buildRecurringPayload = (draft: RecurringDraft) => {
  const payload = {
    description: draft.description.trim(),
    intervalMinutes: Math.max(1, draft.intervalMinutes || 1),
    startTime: null as string | null,
    endTime: null as string | null,
    repeatMode: draft.mode,
    scheduleTime: null as string | null,
    scheduleWeekday: null as number | null,
    scheduleWeekdays: null as number | null,
    scheduleDay: null as number | null,
    cronExpression: null as string | null,
    tags: normalizeTags(draft.tags ?? []),
    endsOn: draft.endsOn || null,
    remainingCount: normalizeCount(draft.count),
  };
  switch (draft.mode) {
    case "INTERVAL_RANGE":
      payload.startTime = draft.startTime || null;
      payload.endTime = draft.endTime || null;
      break;
    case "DAILY":
    case "WORKDAY":
    case "MONTHLY_LAST_DAY":
    case "MONTHLY_LAST_WORKDAY":
      payload.scheduleTime = draft.scheduleTime || null;
      break;
    case "WEEKLY":
      payload.scheduleTime = draft.scheduleTime || null;
      payload.scheduleWeekdays = draft.scheduleWeekdays & WEEKDAY_MASK_ALL;
      payload.scheduleWeekday = firstWeekday(payload.scheduleWeekdays);
      break;
    case "MONTHLY":
      payload.scheduleTime = draft.scheduleTime || null;
      payload.scheduleDay = draft.scheduleDay;
      break;
    case "CRON":
      payload.cronExpression = draft.cronExpression.trim() || null;
      break;
    default:
      break;
  }
  return payload;
};
