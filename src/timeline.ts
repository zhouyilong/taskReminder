// “今天”视图的数据整理：把今日的一次性提醒、已触发的循环提醒记录与即将到来的
// 循环提醒预估合并成一条按时间排列的时间线。纯函数，便于测试。
import { dateKey, toLocalDateTimeString } from "./format";
import type { RecurringPreview, RecurringTask, ReminderRecord, Task, UserAction } from "./types";
import { taskAnchorTime } from "./due";

/** 同一循环提醒当天超过这么多次（如每 30 分钟一次）时折叠为一条。 */
export const MAX_OCCURRENCES_PER_TASK = 3;

export type TimelineState = "upcoming" | "overdue" | "done" | UserAction;

export interface TimelineEntry {
  key: string;
  /** 本地时间 YYYY-MM-DDTHH:mm:ss。 */
  time: string;
  kind: "task" | "recurring";
  title: string;
  state: TimelineState;
  /** 是否早于当前时间。 */
  isPast: boolean;
  task?: Task;
  recurring?: RecurringTask;
  record?: ReminderRecord;
  /** 折叠掉的同一任务的其他次数。 */
  collapsedCount: number;
}

export interface TodayData {
  entries: TimelineEntry[];
  /** 提醒时间早于今天、仍未完成的待办。 */
  overdueTasks: Task[];
  summary: {
    total: number;
    upcoming: number;
    overdue: number;
    completedToday: number;
  };
}

export interface TodayInput {
  now: Date;
  tasks: Task[];
  completedTasks: Task[];
  recurringTasks: RecurringTask[];
  records: ReminderRecord[];
  previews: RecurringPreview[];
}

const byTime = (a: { time: string }, b: { time: string }) => a.time.localeCompare(b.time);

/** 同一任务超过上限时只保留一条：已发生的保留最近一次，未来的保留最近将要发生的一次。 */
const collapse = (entries: TimelineEntry[], keep: "last" | "first") => {
  if (entries.length <= MAX_OCCURRENCES_PER_TASK) {
    return entries;
  }
  const sorted = [...entries].sort(byTime);
  const kept = keep === "last" ? sorted[sorted.length - 1] : sorted[0];
  return [{ ...kept, collapsedCount: entries.length - 1 }];
};

const groupBy = <T>(items: T[], key: (item: T) => string) => {
  const groups = new Map<string, T[]>();
  for (const item of items) {
    const id = key(item);
    const group = groups.get(id);
    if (group) {
      group.push(item);
    } else {
      groups.set(id, [item]);
    }
  }
  return groups;
};

export const buildTodayData = (input: TodayInput): TodayData => {
  const day = dateKey(input.now);
  const dayStart = `${day}T00:00:00`;
  const dayEnd = `${day}T23:59:59`;
  const now = toLocalDateTimeString(input.now);
  const isToday = (value?: string | null): value is string => !!value && value >= dayStart && value <= dayEnd;
  const recurringById = new Map(input.recurringTasks.map(task => [task.id, task]));

  const entries: TimelineEntry[] = [];

  for (const task of input.tasks) {
    const time = taskAnchorTime(task);
    if (!isToday(time)) {
      continue;
    }
    const isPast = time <= now;
    entries.push({
      key: `task-${task.id}`,
      time,
      kind: "task",
      title: task.description,
      state: isPast ? "overdue" : "upcoming",
      isPast,
      task,
      collapsedCount: 0,
    });
  }

  for (const task of input.completedTasks) {
    const time = taskAnchorTime(task);
    if (!isToday(time)) {
      continue;
    }
    entries.push({
      key: `task-${task.id}`,
      time,
      kind: "task",
      title: task.description,
      state: "done",
      isPast: time <= now,
      task,
      collapsedCount: 0,
    });
  }

  const firedToday = input.records.filter(
    record => record.type === "RECURRING" && isToday(record.triggerTime) && record.triggerTime <= now
  );
  for (const group of groupBy(firedToday, record => record.reminderId).values()) {
    const fired = group.map<TimelineEntry>(record => {
      const recurring = recurringById.get(record.reminderId);
      return {
        key: `record-${record.id}`,
        time: record.triggerTime,
        kind: "recurring",
        title: recurring?.description ?? record.description,
        state: record.action,
        isPast: true,
        recurring,
        record,
        collapsedCount: 0,
      };
    });
    entries.push(...collapse(fired, "last"));
  }

  for (const preview of input.previews) {
    const recurring = recurringById.get(preview.taskId);
    if (!recurring || recurring.isPaused) {
      continue;
    }
    const upcoming = preview.times
      .filter(time => time >= dayStart && time <= dayEnd)
      .map<TimelineEntry>(time => ({
        key: `preview-${recurring.id}-${time}`,
        time,
        kind: "recurring",
        title: recurring.description,
        state: "upcoming",
        // 已到点但尚未触发（巡检会在 30 秒内补发）仍按“待提醒”展示，只是排在“现在”之前。
        isPast: time <= now,
        recurring,
        collapsedCount: 0,
      }));
    entries.push(...collapse(upcoming, "first"));
  }

  entries.sort((a, b) => byTime(a, b) || (a.kind === b.kind ? 0 : a.kind === "task" ? -1 : 1));

  const overdueTasks = input.tasks
    .filter(task => {
      const time = taskAnchorTime(task);
      return !!time && time < dayStart;
    })
    .sort((a, b) => (taskAnchorTime(a) ?? "").localeCompare(taskAnchorTime(b) ?? ""));

  const count = (entry: TimelineEntry) => 1 + entry.collapsedCount;
  return {
    entries,
    overdueTasks,
    summary: {
      total: entries.reduce((sum, entry) => sum + count(entry), 0),
      upcoming: entries.filter(entry => entry.state === "upcoming").reduce((sum, entry) => sum + count(entry), 0),
      overdue: overdueTasks.length + entries.filter(entry => entry.state === "overdue").length,
      completedToday: input.completedTasks.filter(task => isToday(task.completedAt)).length,
    },
  };
};

/** 时间线中“现在”分隔线应插入的位置：第一条晚于当前时间的条目之前。 */
export const nowMarkerIndex = (entries: TimelineEntry[], now: Date) => {
  const current = toLocalDateTimeString(now);
  const index = entries.findIndex(entry => entry.time > current);
  return index === -1 ? entries.length : index;
};
