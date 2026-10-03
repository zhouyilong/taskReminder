// 统计面板的数据计算：基于提醒记录（关闭 / 推迟 / 完成 / 待处理）与已完成待办。纯函数，便于测试。
import { addDays, dateKey } from "./format";
import type { RecurringTask, ReminderRecord, Task, UserAction } from "./types";

export const ACTION_ORDER: UserAction[] = ["COMPLETED", "DISMISSED", "SNOOZED", "PENDING"];

export type ActionCounts = Record<UserAction, number>;

/** 统计的时间粒度：近 7 / 30 天按天，近 12 周按周（周一开始）。 */
export type StatsGranularity = "day" | "week";

export interface StatsBucket {
  /** 这一格的第一天，YYYY-MM-DD（按周时为周一）。 */
  date: string;
  /** 这一格的最后一天（按天时与 date 相同；本周为今天）。 */
  endDate: string;
  counts: ActionCounts;
  total: number;
  /** 完成数：完成的待办（按完成时间）+ 循环提醒的完成记录。 */
  completions: number;
}

/** 按标签汇总的提醒（标签取自提醒对应的待办或循环提醒）。 */
export interface TagSummary {
  /** 标签；空字符串为“未加标签”（含来源已删除的提醒）。 */
  tag: string;
  total: number;
  counts: ActionCounts;
  /** 直接处理（完成或关闭）占比，0–1。 */
  handledRate: number;
}

export interface SnoozeRank {
  reminderId: string;
  description: string;
  type: ReminderRecord["type"];
  count: number;
}

export interface HabitStreak {
  task: RecurringTask;
  /** 当前连续打卡天数。 */
  current: number;
  /** 统计范围内最长连续打卡天数。 */
  best: number;
  /** 统计范围内打卡天数 / 有提醒的天数。 */
  checkedDays: number;
  remindedDays: number;
}

export interface StatsSummary {
  total: number;
  counts: ActionCounts;
  /** 直接处理（完成或关闭）占比，0–1；没有提醒时为 null。 */
  handledRate: number | null;
  completedTasks: number;
  granularity: StatsGranularity;
  buckets: StatsBucket[];
  topSnoozed: SnoozeRank[];
  habits: HabitStreak[];
  tags: TagSummary[];
}

const emptyCounts = (): ActionCounts => ({ COMPLETED: 0, DISMISSED: 0, SNOOZED: 0, PENDING: 0 });

/** “打卡”：当天有一次提醒被完成或关闭（知道了）。推迟和未处理不算。 */
const isCheckIn = (action: UserAction) => action === "COMPLETED" || action === "DISMISSED";

/** 从今天往前数 `days` 天（含今天）的日期键，按先后排列。 */
export const rangeDates = (now: Date, days: number) =>
  Array.from({ length: days }, (_, index) => dateKey(addDays(now, index - days + 1)));

/** 某天所在周的周一。 */
export const weekStart = (date: Date) => addDays(date, -((date.getDay() + 6) % 7));

/** 最近 `weeks` 周（含本周）每周周一的日期键，按先后排列。 */
export const rangeWeeks = (now: Date, weeks: number) => {
  const monday = weekStart(now);
  return Array.from({ length: weeks }, (_, index) => dateKey(addDays(monday, (index - weeks + 1) * 7)));
};

/**
 * 习惯连续天数：只看有提醒的日子（每周几次的提醒不会因为没排程的日子中断），
 * 某天的提醒全部被推迟或未处理即中断。今天还没打卡时不计入，也不中断。
 */
export const computeStreak = (dayChecks: Map<string, boolean>, today: string) => {
  const days = [...dayChecks.keys()].sort();
  let best = 0;
  let run = 0;
  for (const day of days) {
    run = dayChecks.get(day) ? run + 1 : 0;
    best = Math.max(best, run);
  }
  let current = 0;
  for (let index = days.length - 1; index >= 0; index -= 1) {
    const day = days[index];
    const checked = dayChecks.get(day);
    if (day === today && !checked) {
      continue;
    }
    if (!checked) {
      break;
    }
    current += 1;
  }
  return { current, best };
};

const UNTAGGED = "";

/**
 * 统计范围：按天时为最近 `days` 天；按周时为最近 `days / 7` 周（从那一周的周一开始，到今天为止）。
 */
export const computeStats = (input: {
  now: Date;
  days: number;
  granularity?: StatsGranularity;
  records: ReminderRecord[];
  completedTasks: Task[];
  recurringTasks: RecurringTask[];
  /** 进行中的待办，只用来查找提醒记录的标签。 */
  activeTasks?: Task[];
  topLimit?: number;
}): StatsSummary => {
  const granularity = input.granularity ?? "day";
  const today = dateKey(input.now);
  const starts = granularity === "week" ? rangeWeeks(input.now, Math.max(1, Math.round(input.days / 7))) : rangeDates(input.now, input.days);
  const start = `${starts[0]}T00:00:00`;
  const end = `${today}T23:59:59`;
  const inRange = (value?: string | null): value is string => !!value && value >= start && value <= end;
  const bucketKey = (value: string) =>
    granularity === "week" ? dateKey(weekStart(new Date(`${value.slice(0, 10)}T00:00:00`))) : value.slice(0, 10);

  const records = input.records.filter(record => inRange(record.triggerTime));
  const counts = emptyCounts();
  const buckets = new Map<string, StatsBucket>(
    starts.map((date, index) => {
      const endDate = granularity === "week" ? (index === starts.length - 1 ? today : dateKey(addDays(new Date(`${date}T00:00:00`), 6))) : date;
      return [date, { date, endDate, counts: emptyCounts(), total: 0, completions: 0 }];
    })
  );
  const snoozed = new Map<string, SnoozeRank>();
  const checksByTask = new Map<string, Map<string, boolean>>();

  const tagsById = new Map<string, string[]>();
  for (const task of [...(input.activeTasks ?? []), ...input.completedTasks, ...input.recurringTasks]) {
    tagsById.set(task.id, task.tags ?? []);
  }
  const byTag = new Map<string, TagSummary>();
  const addToTag = (tag: string, action: UserAction) => {
    const key = tag.toLowerCase();
    const summary = byTag.get(key) ?? { tag, total: 0, counts: emptyCounts(), handledRate: 0 };
    summary.total += 1;
    summary.counts[action] += 1;
    byTag.set(key, summary);
  };

  for (const task of input.completedTasks) {
    if (inRange(task.completedAt)) {
      const bucket = buckets.get(bucketKey(task.completedAt));
      if (bucket) {
        bucket.completions += 1;
      }
    }
  }

  for (const record of records) {
    const action = ACTION_ORDER.includes(record.action) ? record.action : "PENDING";
    counts[action] += 1;
    const bucket = buckets.get(bucketKey(record.triggerTime));
    if (bucket) {
      bucket.counts[action] += 1;
      bucket.total += 1;
      // 一次性待办在弹窗中完成时，已按完成时间计入完成的待办，这里只加循环提醒的完成。
      if (action === "COMPLETED" && record.type === "RECURRING") {
        bucket.completions += 1;
      }
    }
    const tags = tagsById.get(record.reminderId) ?? [];
    if (tags.length) {
      tags.forEach(tag => addToTag(tag, action));
    } else {
      addToTag(UNTAGGED, action);
    }
    if (action === "SNOOZED") {
      const rank = snoozed.get(record.reminderId);
      if (rank) {
        rank.count += 1;
      } else {
        snoozed.set(record.reminderId, {
          reminderId: record.reminderId,
          description: record.description,
          type: record.type,
          count: 1,
        });
      }
    }
    if (record.type === "RECURRING") {
      const checks = checksByTask.get(record.reminderId) ?? new Map<string, boolean>();
      const day = record.triggerTime.slice(0, 10);
      checks.set(day, (checks.get(day) ?? false) || isCheckIn(action));
      checksByTask.set(record.reminderId, checks);
    }
  }

  const recurringById = new Map(input.recurringTasks.map(task => [task.id, task]));
  const topSnoozed = [...snoozed.values()]
    .map(rank => ({ ...rank, description: recurringById.get(rank.reminderId)?.description ?? rank.description }))
    .sort((a, b) => b.count - a.count || a.description.localeCompare(b.description))
    .slice(0, input.topLimit ?? 5);

  const habits = input.recurringTasks
    .map<HabitStreak>(task => {
      const checks = checksByTask.get(task.id) ?? new Map<string, boolean>();
      const { current, best } = computeStreak(checks, today);
      return {
        task,
        current,
        best,
        checkedDays: [...checks.values()].filter(Boolean).length,
        remindedDays: checks.size,
      };
    })
    .filter(habit => habit.remindedDays > 0)
    .sort((a, b) => b.current - a.current || b.checkedDays - a.checkedDays);

  const tags = [...byTag.values()]
    .map(summary => ({ ...summary, handledRate: (summary.counts.COMPLETED + summary.counts.DISMISSED) / summary.total }))
    .sort((a, b) => {
      // “未加标签”排在最后。
      if ((a.tag === UNTAGGED) !== (b.tag === UNTAGGED)) {
        return a.tag === UNTAGGED ? 1 : -1;
      }
      return b.total - a.total || a.tag.localeCompare(b.tag);
    });

  const handled = counts.COMPLETED + counts.DISMISSED;
  return {
    total: records.length,
    counts,
    handledRate: records.length ? handled / records.length : null,
    completedTasks: input.completedTasks.filter(task => inRange(task.completedAt)).length,
    granularity,
    buckets: starts.map(date => buckets.get(date)!),
    topSnoozed,
    habits,
    tags,
  };
};

/** 习惯打卡按循环提醒的标签筛选（v2.1）；tag 为空时不筛选，不区分大小写。 */
export const filterHabitsByTag = (habits: HabitStreak[], tag: string) => {
  const value = tag.trim().toLowerCase();
  return value ? habits.filter(habit => (habit.task.tags ?? []).some(item => item.toLowerCase() === value)) : habits;
};
