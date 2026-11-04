// 待办清单便签（v2.2.4）：列表顺序、时间文案与输入框新建待办的解析（纯函数，测试在 todoSticky.spec.ts）。
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
}

const firstLine = (value: string) => value.split(/\r?\n/).find(line => line.trim())?.trim() ?? "";

const toneOf = (anchor: string | null, now: Date): TodoTone => {
  if (!anchor) return "none";
  const nowText = toLocalDateTimeString(now);
  if (anchor < nowText) return "overdue";
  return anchor.slice(0, 10) === nowText.slice(0, 10) ? "today" : "later";
};

/**
 * 清单中的待办：只列未完成、未删除的；有时间的按时间先后（过期的自然在最前），
 * 没有时间的排在后面，按优先级从高到低；相同时按创建时间。
 */
export const todoStickyItems = (tasks: ReadonlyArray<Task>, now = new Date()): TodoStickyItem[] =>
  tasks
    .filter(task => task.status !== "COMPLETED" && !task.deletedAt)
    .map(task => ({ task, anchor: taskAnchorTime(task) }))
    .sort((a, b) => {
      if (a.anchor && b.anchor) return a.anchor.localeCompare(b.anchor) || a.task.createdAt.localeCompare(b.task.createdAt);
      if (a.anchor) return -1;
      if (b.anchor) return 1;
      return priorityOf(b.task) - priorityOf(a.task) || a.task.createdAt.localeCompare(b.task.createdAt);
    })
    .map(({ task, anchor }) => ({
      task,
      title: firstLine(task.description) || "（无标题）",
      when: anchor ? formatTempTime(anchor, now) : "",
      tone: toneOf(anchor, now),
    }));

/** 清单头部的计数文案：“3 项 · 1 项已过期”。 */
export const todoStickySummary = (items: ReadonlyArray<TodoStickyItem>) => {
  if (!items.length) return "没有待办";
  const overdue = items.filter(item => item.tone === "overdue").length;
  return overdue ? `${items.length} 项 · ${overdue} 项已过期` : `${items.length} 项`;
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
