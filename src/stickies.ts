// 便签管理列表：状态、筛选、搜索与排序（纯函数，测试见 stickies.spec.ts）。
import { markdownToPlainText } from "./markdown";
import type { StickyNoteSummary } from "./types";

/** 显示中：窗口可见；已隐藏：打开着但被“隐藏全部便签”藏起；已关闭：点了关闭。 */
export type StickyState = "showing" | "hidden" | "closed";
export type StickyFilter = "all" | "open" | "closed";

export const STICKY_STATE_LABELS: Record<StickyState, string> = {
  showing: "显示中",
  hidden: "已隐藏",
  closed: "已关闭",
};

export const STICKY_FILTER_OPTIONS: Array<{ value: StickyFilter; label: string }> = [
  { value: "all", label: "全部" },
  { value: "open", label: "已打开" },
  { value: "closed", label: "已关闭" },
];

const STATE_ORDER: Record<StickyState, number> = { showing: 0, hidden: 1, closed: 2 };

/** 便签颜色（v2.1，与后端 `STICKY_COLORS` 一致）；空字符串为默认颜色。 */
export const STICKY_COLOR_OPTIONS: Array<{ value: string; label: string }> = [
  { value: "", label: "默认" },
  { value: "yellow", label: "黄色" },
  { value: "blue", label: "蓝色" },
  { value: "green", label: "绿色" },
  { value: "pink", label: "粉色" },
  { value: "purple", label: "紫色" },
  { value: "gray", label: "灰色" },
];

/** 认识的颜色返回对应的 CSS 类；默认或不认识的颜色（来自更新版本）按默认颜色显示。 */
export const stickyColorClass = (color?: string | null) =>
  color && STICKY_COLOR_OPTIONS.some(option => option.value === color) ? `sticky-color-${color}` : "";

/** 色点的 CSS 类：认识的颜色为 is-<颜色>，默认或不认识的为 is-default（空心）。 */
export const stickyDotClass = (color?: string | null) =>
  stickyColorClass(color) ? `is-${color}` : "is-default";

/** 颜色筛选：all 为不限，其余为某个颜色（"" 为默认颜色）。 */
export type StickyColorFilter = "all" | string;

const normalizedColor = (color?: string | null) =>
  color && STICKY_COLOR_OPTIONS.some(option => option.value === color) ? color : "";

export interface StickyListItem {
  note: StickyNoteSummary;
  state: StickyState;
  /** 内容的纯文本（去掉 Markdown 标记），用于预览与搜索。 */
  preview: string;
}

export const stickyState = (note: Pick<StickyNoteSummary, "isOpen" | "visible">): StickyState => {
  if (!note.isOpen) {
    return "closed";
  }
  return note.visible ? "showing" : "hidden";
};

/** 库里的时间有 `2026-09-30T08:00:00` 与 `2026-09-30 08:00:00` 两种写法。 */
const timeValue = (value?: string | null) => {
  if (!value) {
    return 0;
  }
  const time = Date.parse(value.replace(" ", "T"));
  return Number.isNaN(time) ? 0 : time;
};

const matchesFilter = (state: StickyState, filter: StickyFilter) => {
  if (filter === "open") {
    return state !== "closed";
  }
  if (filter === "closed") {
    return state === "closed";
  }
  return true;
};

/** 按筛选与关键词（标题或内容）过滤；显示中 → 已隐藏 → 已关闭，同组内最近修改的在前。 */
export const buildStickyList = (
  notes: StickyNoteSummary[],
  keyword = "",
  filter: StickyFilter = "all",
  color: StickyColorFilter = "all",
): StickyListItem[] => {
  const value = keyword.trim().toLowerCase();
  return notes
    .map(note => ({ note, state: stickyState(note), preview: markdownToPlainText(note.content) }))
    .filter(item => matchesFilter(item.state, filter))
    .filter(item => color === "all" || normalizedColor(item.note.color) === color)
    .filter(
      item =>
        !value || item.note.title.toLowerCase().includes(value) || item.preview.toLowerCase().includes(value),
    )
    .sort(
      (a, b) =>
        STATE_ORDER[a.state] - STATE_ORDER[b.state] || timeValue(b.note.updatedAt) - timeValue(a.note.updatedAt),
    );
};

export const countStickyStates = (notes: StickyNoteSummary[]): Record<StickyState, number> => {
  const counts: Record<StickyState, number> = { showing: 0, hidden: 0, closed: 0 };
  for (const note of notes) {
    counts[stickyState(note)] += 1;
  }
  return counts;
};
