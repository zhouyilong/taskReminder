// 循环提醒的规则展示、表单草稿、校验与提交载荷，新建与编辑共用。
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
  { value: "CRON", label: "Cron 表达式" },
];

export const formatRecurringMode = (mode?: RecurringMode | string | null) => {
  const resolved = recurringModeOptions.find(item => item.value === mode);
  return resolved ? resolved.label : "区间间隔";
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
    case "CRON":
      return task.cronExpression || "-";
    case "INTERVAL_RANGE":
    default: {
      const start = task.startTime || "00:00";
      const end = task.endTime || "23:59";
      return `每 ${task.intervalMinutes} 分钟（${start} - ${end}）`;
    }
  }
};

export const formatWorkdayHint = (years: number[]) => {
  const coverage = years.length
    ? `已内置 ${years[0]}${years.length > 1 ? `–${years[years.length - 1]}` : ""} 年安排，其他年份按周一至周五`
    : "暂无节假日数据，按周一至周五";
  return `跳过法定节假日，调休上班日照常提醒（${coverage}）`;
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
  if (task.repeatMode !== "WORKDAY" || task.isPaused) {
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
});

/** 返回错误提示；通过校验时返回 null。 */
export const validateRecurringDraft = (draft: RecurringDraft): string | null => {
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
  };
  switch (draft.mode) {
    case "INTERVAL_RANGE":
      payload.startTime = draft.startTime || null;
      payload.endTime = draft.endTime || null;
      break;
    case "DAILY":
    case "WORKDAY":
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
