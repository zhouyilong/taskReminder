// 统计面板的数据计算：基于提醒记录（关闭 / 推迟 / 完成 / 待处理）与已完成待办。纯函数，便于测试。
import { addDays, dateKey } from "./format";
import type { RecurringTask, ReminderRecord, Task, UserAction } from "./types";

export const ACTION_ORDER: UserAction[] = ["COMPLETED", "DISMISSED", "SNOOZED", "PENDING"];

export type ActionCounts = Record<UserAction, number>;

export interface DailyBucket {
  /** YYYY-MM-DD */
  date: string;
  counts: ActionCounts;
  total: number;
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
  daily: DailyBucket[];
  topSnoozed: SnoozeRank[];
  habits: HabitStreak[];
}

const emptyCounts = (): ActionCounts => ({ COMPLETED: 0, DISMISSED: 0, SNOOZED: 0, PENDING: 0 });

/** “打卡”：当天有一次提醒被完成或关闭（知道了）。推迟和未处理不算。 */
const isCheckIn = (action: UserAction) => action === "COMPLETED" || action === "DISMISSED";

/** 从今天往前数 `days` 天（含今天）的日期键，按先后排列。 */
export const rangeDates = (now: Date, days: number) =>
  Array.from({ length: days }, (_, index) => dateKey(addDays(now, index - days + 1)));

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

export const computeStats = (input: {
  now: Date;
  days: number;
  records: ReminderRecord[];
  completedTasks: Task[];
  recurringTasks: RecurringTask[];
  topLimit?: number;
}): StatsSummary => {
  const dates = rangeDates(input.now, input.days);
  const start = `${dates[0]}T00:00:00`;
  const today = dates[dates.length - 1];
  const end = `${today}T23:59:59`;
  const inRange = (value?: string | null): value is string => !!value && value >= start && value <= end;

  const records = input.records.filter(record => inRange(record.triggerTime));
  const counts = emptyCounts();
  const daily = new Map<string, DailyBucket>(
    dates.map(date => [date, { date, counts: emptyCounts(), total: 0 }])
  );
  const snoozed = new Map<string, SnoozeRank>();
  const checksByTask = new Map<string, Map<string, boolean>>();

  for (const record of records) {
    const action = ACTION_ORDER.includes(record.action) ? record.action : "PENDING";
    counts[action] += 1;
    const bucket = daily.get(record.triggerTime.slice(0, 10));
    if (bucket) {
      bucket.counts[action] += 1;
      bucket.total += 1;
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

  const handled = counts.COMPLETED + counts.DISMISSED;
  return {
    total: records.length,
    counts,
    handledRate: records.length ? handled / records.length : null,
    completedTasks: input.completedTasks.filter(task => inRange(task.completedAt)).length,
    daily: dates.map(date => daily.get(date)!),
    topSnoozed,
    habits,
  };
};

/** 习惯打卡按循环提醒的标签筛选（v2.1）；tag 为空时不筛选，不区分大小写。 */
export const filterHabitsByTag = (habits: HabitStreak[], tag: string) => {
  const value = tag.trim().toLowerCase();
  return value ? habits.filter(habit => (habit.task.tags ?? []).some(item => item.toLowerCase() === value)) : habits;
};
