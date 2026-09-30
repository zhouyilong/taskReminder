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
): StickyListItem[] => {
  const value = keyword.trim().toLowerCase();
  return notes
    .map(note => ({ note, state: stickyState(note), preview: markdownToPlainText(note.content) }))
    .filter(item => matchesFilter(item.state, filter))
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
