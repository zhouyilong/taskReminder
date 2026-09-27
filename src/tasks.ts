// 待办列表的标签、优先级、筛选与排序。纯函数，便于测试。
import type { Task } from "./types";

export const PRIORITY_OPTIONS = [
  { value: 0, label: "无", short: "" },
  { value: 1, label: "低", short: "低" },
  { value: 2, label: "中", short: "中" },
  { value: 3, label: "高", short: "高" },
] as const;

export const priorityOf = (task: Pick<Task, "priority">) => Math.min(3, Math.max(0, task.priority ?? 0));

export const priorityLabel = (value: number) => PRIORITY_OPTIONS.find(item => item.value === value)?.label ?? "无";

/** 与后端 normalize_tags 一致：去掉前导 #、逗号，按不区分大小写去重。 */
export const normalizeTags = (tags: string[]) => {
  const result: string[] = [];
  for (const raw of tags) {
    const cleaned = raw
      .trim()
      .replace(/^[#＃]+/, "")
      .replace(/[,，]/g, "")
      .slice(0, 24)
      .trim();
    if (cleaned && !result.some(item => item.toLowerCase() === cleaned.toLowerCase())) {
      result.push(cleaned);
    }
    if (result.length >= 10) {
      break;
    }
  }
  return result;
};

/** 所有标签及使用次数，按次数降序、再按名称排序。 */
export const collectTags = (tasks: Task[]) => {
  const counts = new Map<string, { tag: string; count: number }>();
  for (const task of tasks) {
    for (const tag of task.tags ?? []) {
      const key = tag.toLowerCase();
      const entry = counts.get(key);
      if (entry) {
        entry.count += 1;
      } else {
        counts.set(key, { tag, count: 1 });
      }
    }
  }
  return [...counts.values()].sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag, "zh-CN"));
};

export type TaskSortKey = "created" | "reminder" | "priority";

export const TASK_SORT_OPTIONS: { value: TaskSortKey; label: string }[] = [
  { value: "created", label: "创建时间" },
  { value: "reminder", label: "提醒时间" },
  { value: "priority", label: "优先级" },
];

export interface TaskFilter {
  query: string;
  /** 为空表示全部标签。 */
  tag: string;
  /** -1 表示全部优先级。 */
  priority: number;
}

export const matchesTaskFilter = (task: Task, filter: TaskFilter) => {
  if (filter.tag && !(task.tags ?? []).some(tag => tag.toLowerCase() === filter.tag.toLowerCase())) {
    return false;
  }
  if (filter.priority >= 0 && priorityOf(task) !== filter.priority) {
    return false;
  }
  const query = filter.query.trim().toLowerCase();
  if (!query) {
    return true;
  }
  const haystack = [task.description, task.stickyContent ?? "", ...(task.tags ?? []).map(tag => `#${tag}`)]
    .join("\n")
    .toLowerCase();
  return query.split(/\s+/).every(part => haystack.includes(part));
};

const compareOptionalTime = (a?: string | null, b?: string | null) => {
  if (a && b) {
    return a.localeCompare(b);
  }
  if (a) {
    return -1;
  }
  if (b) {
    return 1;
  }
  return 0;
};

/** 排序：提醒时间升序（未设置的排在最后），优先级降序；相同则按创建时间升序。 */
export const sortTasks = (tasks: Task[], key: TaskSortKey) => {
  const sorted = [...tasks];
  sorted.sort((a, b) => {
    let result = 0;
    if (key === "reminder") {
      result = compareOptionalTime(a.reminderTime, b.reminderTime);
    } else if (key === "priority") {
      result = priorityOf(b) - priorityOf(a) || compareOptionalTime(a.reminderTime, b.reminderTime);
    }
    return result || a.createdAt.localeCompare(b.createdAt);
  });
  return sorted;
};

export const filterAndSortTasks = (tasks: Task[], filter: TaskFilter, sort: TaskSortKey) =>
  sortTasks(
    tasks.filter(task => matchesTaskFilter(task, filter)),
    sort
  );
