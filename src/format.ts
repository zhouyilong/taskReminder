// 主窗口各视图共用的格式化与时间工具。数据库中的时间统一为本地时间 YYYY-MM-DDTHH:mm:ss。
import type { UserAction } from "./types";
import { markdownToPreviewText, stripLeadingListMarker } from "./markdown";

export const isLinuxPlatform =
  typeof navigator !== "undefined" && /linux/i.test(navigator.userAgent);

export const formatDateTime = (value?: string | null) => {
  if (!value) {
    return "-";
  }
  return value.replace("T", " ");
};

/** 取 HH:mm。 */
export const formatClock = (value?: string | null) => {
  if (!value) {
    return "--:--";
  }
  return value.slice(11, 16) || "--:--";
};

export const reminderTone = (value?: string | null, now = Date.now()) => {
  if (!value) {
    return "";
  }
  const time = new Date(value).getTime();
  if (Number.isNaN(time)) {
    return "";
  }
  return time < now ? "is-overdue" : "is-upcoming";
};

export const formatBytes = (value: number) => {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const precision = size >= 10 || unitIndex === 0 ? 0 : 1;
  return `${size.toFixed(precision)} ${units[unitIndex]}`;
};

export const formatAction = (action: UserAction | string) => {
  switch (action) {
    case "DISMISSED":
      return "已关闭";
    case "SNOOZED":
      return "已推迟";
    case "COMPLETED":
      return "已完成";
    case "PENDING":
      return "待处理";
    default:
      return action;
  }
};

export const taskStickyPreview = (value: string | null | undefined) => {
  return markdownToPreviewText(value);
};

export const recordDescription = (value: string | null | undefined) => {
  // 提醒记录里有些描述来自 Markdown 列表，前面可能还夹带零宽字符，
  // 这里统一去掉前导列表标记，避免复选框和描述之间出现多余的点。
  const text = stripLeadingListMarker(markdownToPreviewText(value, ""));
  return text || "-";
};

export const toDatetimeLocal = (value?: string | null) => {
  if (!value) {
    return "";
  }
  return value.slice(0, 16);
};

export const fromDatetimeLocal = (value: string) => {
  if (!value) {
    return null;
  }
  return value.length === 16 ? `${value}:00` : value;
};

const pad = (value: number) => String(value).padStart(2, "0");

/** 本地日期键 YYYY-MM-DD。 */
export const dateKey = (date: Date) =>
  `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;

/** 与数据库一致的本地时间字符串 YYYY-MM-DDTHH:mm:ss。 */
export const toLocalDateTimeString = (date: Date) =>
  `${dateKey(date)}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;

export const addDays = (date: Date, days: number) => {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
};

const WEEKDAY_NAMES = ["日", "一", "二", "三", "四", "五", "六"];

/** 如 “9月26日 周六”。 */
export const formatMonthDay = (date: Date) =>
  `${date.getMonth() + 1}月${date.getDate()}日 周${WEEKDAY_NAMES[date.getDay()]}`;

export const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);
