// 待办清单便签（v2.2.4）：列表顺序、时间文案与输入框新建待办的解析（纯函数，测试在 todoSticky.spec.ts）。
// v2.2.5：拖拽调整顺序（与待办页“手动顺序”共用 sortOrder）、加急标识。
import { taskAnchorTime } from "./due";
import { toLocalDateTimeString } from "./format";
import { parseQuickInput } from "./nlp";
import { buildRecurringPayload, formatTempTime, validateRecurringDraft } from "./recurring";
import { priorityOf } from "./tasks";
import type { Task } from "./types";

export type TodoTone = "overdue" | "today" | "later" | "none";

export interface TodoStickyItem {
  task: Task;
  /** 待办标题（多行时只取第一行）。 */
  title: string;
  /** 截止时间或提醒时间的简短文案（今天 / 明天 / 月日 + 钟点）；没有时为空字符串。 */
  when: string;
  tone: TodoTone;
  /** 加急（v2.2.5）。 */
  urgent: boolean;
}

const hasOrder = (task: Task) => typeof task.sortOrder === "number" && Number.isFinite(task.sortOrder);

const firstLine = (value: string) => value.split(/\r?\n/).find(line => line.trim())?.trim() ?? "";

const toneOf = (anchor: string | null, now: Date): TodoTone => {
  if (!anchor) return "none";
  const nowText = toLocalDateTimeString(now);
  if (anchor < nowText) return "overdue";
  return anchor.slice(0, 10) === nowText.slice(0, 10) ? "today" : "later";
};

type Anchored = { task: Task; anchor: string | null };

/** 没有手动顺序时的默认顺序：有时间的按先后，没有时间的按优先级从高到低，相同时按创建时间。 */
const compareDefault = (a: Anchored, b: Anchored) => {
  if (a.anchor && b.anchor) return a.anchor.localeCompare(b.anchor) || a.task.createdAt.localeCompare(b.task.createdAt);
  if (a.anchor) return -1;
  if (b.anchor) return 1;
  return priorityOf(b.task) - priorityOf(a.task) || a.task.createdAt.localeCompare(b.task.createdAt);
};

/**
 * 清单中的待办：只列未完成、未删除的。拖动排过的（有 sortOrder，与待办页“手动顺序”共用）按位置在前；
 * 其余（还没排过、或之后新建的）排在后面，按默认顺序：有时间的按时间先后（过期的自然在最前），
 * 没有时间的按优先级从高到低；相同时按创建时间。
 */
export const todoStickyItems = (tasks: ReadonlyArray<Task>, now = new Date()): TodoStickyItem[] =>
  tasks
    .filter(task => task.status !== "COMPLETED" && !task.deletedAt)
    .map(task => ({ task, anchor: taskAnchorTime(task) }))
    .sort((a, b) => {
      const ordered = Number(hasOrder(b.task)) - Number(hasOrder(a.task));
      if (ordered) return ordered;
      if (hasOrder(a.task)) return (a.task.sortOrder as number) - (b.task.sortOrder as number) || compareDefault(a, b);
      return compareDefault(a, b);
    })
    .map(({ task, anchor }) => ({
      task,
      title: firstLine(task.description) || "（无标题）",
      when: anchor ? formatTempTime(anchor, now) : "",
      tone: toneOf(anchor, now),
      urgent: Boolean(task.urgent),
    }));

/** 清单头部的计数文案：“3 项 · 1 项加急 · 1 项已过期”。 */
export const todoStickySummary = (items: ReadonlyArray<TodoStickyItem>) => {
  if (!items.length) return "没有待办";
  const parts = [`${items.length} 项`];
  const urgent = items.filter(item => item.urgent).length;
  if (urgent) parts.push(`${urgent} 项加急`);
  const overdue = items.filter(item => item.tone === "overdue").length;
  if (overdue) parts.push(`${overdue} 项已过期`);
  return parts.join(" · ");
};

/** 拖动时的插入位置（0 … 行数）：指针在某行上半部分时插到它前面，下半部分时插到它后面。 */
export const dropIndexAt = (rows: ReadonlyArray<{ top: number; height: number }>, y: number) => {
  for (let index = 0; index < rows.length; index += 1) {
    const row = rows[index];
    if (y < row.top + row.height / 2) return index;
  }
  return rows.length;
};

export type TodoInputResult =
  | { kind: "task"; payload: { description: string; reminderTime: string | null; dueAt: string | null; tags: string[]; project: string; priority: number }; hint: string }
  | { kind: "recurring"; payload: ReturnType<typeof buildRecurringPayload>; hint: string }
  | { kind: "error"; message: string };

/**
 * 解析输入框中的一行（与快速添加相同的自然语言识别）：识别出的时间、标签、项目、优先级从标题中去掉；
 * 识别出循环规则时新建循环提醒。截止时间算出的提醒时间已过时只记截止时间、不提醒。
 */
export const parseTodoInput = (input: string, now = new Date()): TodoInputResult | null => {
  const text = input.trim();
  if (!text) return null;
  const parsed = parseQuickInput(text, now);
  const title = parsed.title || text;
  if (parsed.recurring) {
    const draft = { ...parsed.recurring, description: title, tags: parsed.tags };
    const invalid = validateRecurringDraft(draft);
    if (invalid) return { kind: "error", message: invalid };
    return { kind: "recurring", payload: buildRecurringPayload(draft), hint: "已添加为循环提醒" };
  }
  let reminder = parsed.reminderTime;
  const due = parsed.dueTime;
  if (due && reminder && reminder.getTime() <= now.getTime() && due.getTime() > now.getTime()) {
    reminder = null;
  }
  if (reminder && reminder.getTime() <= now.getTime()) {
    return { kind: "error", message: "提醒时间需晚于当前时间" };
  }
  const reminderTime = reminder ? toLocalDateTimeString(reminder) : null;
  const dueAt = due ? toLocalDateTimeString(due) : null;
  const shown = dueAt ?? reminderTime;
  return {
    kind: "task",
    payload: {
      description: title,
      reminderTime,
      dueAt,
      tags: parsed.tags,
      project: parsed.project,
      priority: parsed.priority,
    },
    hint: shown ? `已添加，${formatTempTime(shown, now)}${dueAt ? " 截止" : " 提醒"}` : "已添加",
  };
};
