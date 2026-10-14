// 全屏提醒效果（v2.2.1）：设置选项、说明文案，以及效果页面展示的文字与主题（纯函数）。
// 效果窗口由后端在每块屏幕上各开一个（`attention.rs`），内容在页面加载前注入到 `window.__ATTENTION__`。
import { markdownToPreviewText, stripLeadingListMarker } from "./markdown";
import type { AttentionEffect, NotificationScreen } from "./types";

/** 一次效果的时长，与后端 `attention::EFFECT_DURATION_MS` 一致。 */
export const ATTENTION_DURATION_MS = 6000;
/** 结束前的淡出时长（包含在 `ATTENTION_DURATION_MS` 内）。 */
export const ATTENTION_FADE_MS = 600;
/** 弹出时间比原定时间晚超过该值，视为“错过的提醒”（与提醒弹窗一致）。 */
export const MISSED_THRESHOLD_MS = 2 * 60 * 1000;

export type PopupDirection = "here" | "left" | "right" | "up" | "down";

export interface AttentionPayload {
  effect: AttentionEffect;
  untilHandled: boolean;
  /** 光晕强度（`ATTENTION_INTENSITY_MIN`…`ATTENTION_INTENSITY_MAX`）。 */
  intensity: number;
  reminderType: string;
  description: string;
  scheduledTime?: string | null;
  /** 队列中的提醒总数（含展示的这一条）。 */
  count: number;
  /** 再次提醒（提醒未处理、到了设置的间隔）。 */
  repeat: boolean;
  preview: boolean;
  /** 提醒弹窗的主题设置：system / app / light / dark。 */
  theme: string;
  /** 提醒弹窗所在屏幕相对于本屏的方向。 */
  popupDirection: PopupDirection | string;
}

/** 队列变化时后端推送的更新（`attention-queue` 事件）。 */
export type AttentionQueueUpdate = Pick<AttentionPayload, "reminderType" | "description" | "scheduledTime" | "count">;

export const ATTENTION_EFFECT_OPTIONS: Array<{ value: AttentionEffect; label: string }> = [
  { value: "off", label: "关闭" },
  { value: "edge", label: "边缘光晕" },
  { value: "banner", label: "光晕 + 顶部横幅" }
];

/** 光晕强度，与后端 `attention::INTENSITY_*` 一致；2（标准）即 2.2.1 的效果。 */
export const ATTENTION_INTENSITY_MIN = 1;
export const ATTENTION_INTENSITY_MAX = 5;
export const ATTENTION_INTENSITY_DEFAULT = 2;

export const ATTENTION_INTENSITY_OPTIONS: Array<{ value: number; label: string }> = [
  { value: 1, label: "柔和" },
  { value: 2, label: "标准" },
  { value: 3, label: "较强" },
  { value: 4, label: "强烈" },
  { value: 5, label: "最强" }
];

/** 超出范围或不是整数的强度按默认（标准）处理。 */
export const normalizeAttentionIntensity = (value: unknown): number =>
  typeof value === "number" &&
  Number.isInteger(value) &&
  value >= ATTENTION_INTENSITY_MIN &&
  value <= ATTENTION_INTENSITY_MAX
    ? value
    : ATTENTION_INTENSITY_DEFAULT;

export const attentionIntensityLabel = (value: number): string =>
  ATTENTION_INTENSITY_OPTIONS.find(item => item.value === normalizeAttentionIntensity(value))?.label ?? "标准";

// 每档光晕的尺寸：描边、近处光晕、柔光，以及较强档位额外的一层向屏幕中间延伸的深层柔光
// （按屏幕短边 vmin 计算，大屏小屏都能延伸到相近的比例）。
const EDGE_LEVELS: Record<number, { ring: number; glow: [number, number]; soft: [number, number]; deep: [number, number] }> = {
  1: { ring: 2, glow: [36, 8], soft: [120, 36], deep: [0, 0] },
  2: { ring: 3, glow: [48, 12], soft: [180, 56], deep: [0, 0] },
  3: { ring: 4, glow: [64, 18], soft: [240, 80], deep: [24, 4] },
  4: { ring: 5, glow: [84, 26], soft: [320, 110], deep: [34, 8] },
  5: { ring: 6, glow: [110, 36], soft: [420, 150], deep: [46, 12] }
};

/** 边缘光晕的 CSS 变量（`AttentionApp.vue` 的 `.attention-edge` 使用）。 */
export const attentionEdgeVars = (intensity: number): Record<string, string> => {
  const level = EDGE_LEVELS[normalizeAttentionIntensity(intensity)];
  return {
    "--attention-ring-width": `${level.ring}px`,
    "--attention-glow-blur": `${level.glow[0]}px`,
    "--attention-glow-spread": `${level.glow[1]}px`,
    "--attention-soft-blur": `${level.soft[0]}px`,
    "--attention-soft-spread": `${level.soft[1]}px`,
    "--attention-deep-blur": `${level.deep[0]}vmin`,
    "--attention-deep-spread": `${level.deep[1]}vmin`
  };
};

export const ATTENTION_REPEAT_OPTIONS: Array<{ value: number; label: string }> = [
  { value: 0, label: "不重复" },
  { value: 2, label: "每 2 分钟" },
  { value: 5, label: "每 5 分钟" },
  { value: 10, label: "每 10 分钟" }
];

export const NOTIFICATION_SCREEN_OPTIONS: Array<{ value: NotificationScreen; label: string }> = [
  { value: "cursor", label: "鼠标所在屏幕" },
  { value: "primary", label: "主屏幕" }
];

/** 设置中“全屏提醒效果”的说明。 */
export const attentionEffectHint = (effect: AttentionEffect, untilHandled: boolean): string => {
  if (effect === "off") {
    return "到点时只弹出提醒窗口";
  }
  const what = effect === "banner" ? "每块屏幕边缘亮起柔光，顶部显示提醒内容" : "每块屏幕边缘亮起柔光";
  const until = untilHandled ? "直到处理完提醒" : "约 6 秒后淡出";
  return `${what}，${until}；不挡操作、不抢焦点，勿扰时段内不播放`;
};

/** 设置中“未处理时再次提醒”的说明。 */
export const attentionRepeatHint = (minutes: number, effect: AttentionEffect): string => {
  if (minutes <= 0) {
    return "提醒弹出后不再重复";
  }
  const replay = effect === "off" ? "" : "、重播全屏效果";
  return `提醒没有处理时，每 ${minutes} 分钟把弹窗移到鼠标所在屏幕${replay}，开启提示音时再响一次`;
};

const pad = (value: number) => value.toString().padStart(2, "0");

/** 原定时间：当天只显示钟点，否则附带日期。 */
export const formatScheduledClock = (date: Date, now: Date): string => {
  const clock = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  const sameDay =
    date.getFullYear() === now.getFullYear() && date.getMonth() === now.getMonth() && date.getDate() === now.getDate();
  return sameDay ? clock : `${date.getMonth() + 1}月${date.getDate()}日 ${clock}`;
};

const parseLocal = (value?: string | null): Date | null => {
  if (!value) return null;
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
};

/** 错过的提醒（晚于原定时间 2 分钟以上）返回“原定 08:30”，否则为空。 */
export const missedLabel = (scheduledTime: string | null | undefined, now: Date): string => {
  const scheduled = parseLocal(scheduledTime);
  if (!scheduled || now.getTime() - scheduled.getTime() < MISSED_THRESHOLD_MS) {
    return "";
  }
  return `原定 ${formatScheduledClock(scheduled, now)}`;
};

/** 横幅的小标题：预览 / 再次提醒 / 错过的提醒 / 任务提醒 / 循环提醒。 */
export const attentionEyebrow = (
  payload: Pick<AttentionPayload, "preview" | "repeat" | "reminderType" | "scheduledTime">,
  now: Date
): string => {
  if (payload.preview) return "效果预览";
  const kind = payload.reminderType === "RECURRING" ? "循环提醒" : "任务提醒";
  const missed = missedLabel(payload.scheduledTime, now);
  if (payload.repeat) return `${kind} · 还没有处理`;
  if (missed) return `错过的提醒 · ${missed}`;
  return kind;
};

/** 横幅标题：描述的纯文本预览（去掉 Markdown 与前导列表标记）。 */
export const attentionTitle = (description: string | null | undefined): string =>
  stripLeadingListMarker(markdownToPreviewText(description, "")) || "提醒";

const DIRECTION_LABELS: Record<string, string> = {
  left: "左侧",
  right: "右侧",
  up: "上方",
  down: "下方"
};

/** 横幅提示：去哪块屏幕处理，以及队列中还有几条。 */
export const popupHint = (direction: string, count: number): string => {
  const where = DIRECTION_LABELS[direction] ? `提醒窗口在${DIRECTION_LABELS[direction]}屏幕` : "在右下角处理";
  return count > 1 ? `${where} · 共 ${count} 条` : where;
};

/**
 * 效果页面的主题：明确设置了浅色 / 深色就用它；“跟随应用”读主窗口保存的主题，
 * 没有保存过或“跟随系统”时按系统配色。
 */
export const resolveAttentionTheme = (
  mode: string | null | undefined,
  appTheme: string | null | undefined,
  systemLight: boolean
): "light" | "dark" => {
  if (mode === "light" || mode === "dark") return mode;
  if (mode !== "system" && (appTheme === "light" || appTheme === "dark")) return appTheme;
  return systemLight ? "light" : "dark";
};

/** 读取注入的效果内容，缺字段时补默认值（防止页面因数据不全而报错）。 */
export const normalizeAttentionPayload = (raw: unknown): AttentionPayload | null => {
  if (!raw || typeof raw !== "object") return null;
  const value = raw as Partial<AttentionPayload>;
  const effect: AttentionEffect = value.effect === "edge" || value.effect === "off" ? value.effect : "banner";
  return {
    effect,
    untilHandled: value.untilHandled === true,
    intensity: normalizeAttentionIntensity(value.intensity),
    reminderType: typeof value.reminderType === "string" ? value.reminderType : "TASK",
    description: typeof value.description === "string" ? value.description : "",
    scheduledTime: typeof value.scheduledTime === "string" ? value.scheduledTime : null,
    count: typeof value.count === "number" && value.count > 0 ? Math.floor(value.count) : 1,
    repeat: value.repeat === true,
    preview: value.preview === true,
    theme: typeof value.theme === "string" ? value.theme : "app",
    popupDirection: typeof value.popupDirection === "string" ? value.popupDirection : "here"
  };
};
