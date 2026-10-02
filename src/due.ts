// 截止时间与提前提醒（v2.1）：提醒时间 = 截止时间 - 提前量，仍存于 reminderTime。
import { toLocalDateTimeString } from "./format";
import type { Task } from "./types";

/** 提前提醒选项（分钟）；-1 表示只记截止时间、不提醒。 */
export const LEAD_OPTIONS: Array<{ value: number; label: string }> = [
  { value: 0, label: "准时提醒" },
  { value: 5, label: "提前 5 分钟" },
  { value: 15, label: "提前 15 分钟" },
  { value: 30, label: "提前 30 分钟" },
  { value: 60, label: "提前 1 小时" },
  { value: 1440, label: "提前 1 天" },
  { value: -1, label: "不提醒" }
];

export const NO_REMINDER = -1;
export type LeadChoice = number | "custom";

const parseLocal = (value?: string | null) => {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
};

/** 按截止时间与提前量算出提醒时间（`datetime-local` 格式）；不提醒或截止时间无效时为空字符串。 */
export const reminderForDue = (dueLocal: string, lead: number): string => {
  const due = parseLocal(dueLocal);
  if (!due || lead < 0) return "";
  return toLocalDateTimeString(new Date(due.getTime() - lead * 60_000)).slice(0, 16);
};

/** 从截止时间与提醒时间反推提前量：对应某个选项时返回该选项，否则为 "custom"。 */
export const leadOf = (dueLocal: string, reminderLocal: string): LeadChoice => {
  if (!reminderLocal) return NO_REMINDER;
  const due = parseLocal(dueLocal);
  const reminder = parseLocal(reminderLocal);
  if (!due || !reminder) return "custom";
  const minutes = Math.round((due.getTime() - reminder.getTime()) / 60_000);
  return LEAD_OPTIONS.some(option => option.value === minutes && option.value >= 0) ? minutes : "custom";
};

/** 列表、日历、时间线中待办所在的时间：有截止时间时按截止时间，否则按提醒时间。 */
export const taskAnchorTime = (task: Pick<Task, "dueAt" | "reminderTime">) => task.dueAt || task.reminderTime || null;

/** 提醒时间与截止时间不同（提前提醒）时，返回提醒时间，用于在截止时间旁附注。 */
export const separateReminder = (task: Pick<Task, "dueAt" | "reminderTime">) =>
  task.dueAt && task.reminderTime && task.reminderTime.slice(0, 16) !== task.dueAt.slice(0, 16) ? task.reminderTime : null;

/**
 * 把待办移到新的时间（日历拖动改期）：有截止时间时移动截止时间，提醒时间随之平移、保持提前量；
 * 没有截止时间时只改提醒时间。返回要提交的字段。
 */
export const moveTaskTimes = (
  task: Pick<Task, "dueAt" | "reminderTime">,
  anchor: string
): { reminderTime: string | null; dueAt?: string | null } => {
  if (!task.dueAt) {
    return { reminderTime: anchor };
  }
  const from = parseLocal(task.dueAt);
  const to = parseLocal(anchor);
  const reminder = parseLocal(task.reminderTime);
  if (!from || !to || !reminder) {
    return { dueAt: anchor, reminderTime: task.reminderTime ?? null };
  }
  const shifted = new Date(reminder.getTime() + (to.getTime() - from.getTime()));
  return { dueAt: anchor, reminderTime: toLocalDateTimeString(shifted) };
};
