// 日历视图的数据整理：生成月份网格与周视图，并把待办、已完成待办、循环提醒（已触发记录与预估）
// 按天归类。纯函数，便于测试。
import { addDays, dateKey, toLocalDateTimeString } from "./format";
import type { RecurringPreview, RecurringTask, ReminderRecord, Task } from "./types";
import { taskAnchorTime } from "./due";

export interface CalendarDay {
  date: Date;
  key: string;
  inMonth: boolean;
  isWeekend: boolean;
}

/** 月份网格：从包含 1 号的那一周的周一开始，固定 6 周 42 天，避免切换月份时高度跳动。 */
export const buildMonthGrid = (year: number, month: number): CalendarDay[] => {
  const first = new Date(year, month, 1);
  const offset = (first.getDay() + 6) % 7;
  const start = addDays(first, -offset);
  return Array.from({ length: 42 }, (_, index) => {
    const date = addDays(start, index);
    return {
      date,
      key: dateKey(date),
      inMonth: date.getMonth() === month,
      isWeekend: date.getDay() === 0 || date.getDay() === 6,
    };
  });
};

/** 周视图：包含 `anchor` 的那一周，周一到周日。 */
export const buildWeekDays = (anchor: Date): CalendarDay[] => {
  const day = new Date(anchor.getFullYear(), anchor.getMonth(), anchor.getDate());
  const start = addDays(day, -((day.getDay() + 6) % 7));
  return Array.from({ length: 7 }, (_, index) => {
    const date = addDays(start, index);
    return {
      date,
      key: dateKey(date),
      inMonth: true,
      isWeekend: date.getDay() === 0 || date.getDay() === 6,
    };
  });
};

/** 周视图标题，如“2026 年 9 月 21 日 – 27 日”；跨月、跨年时补全月份与年份。 */
export const formatWeekRange = (days: CalendarDay[]) => {
  const first = days[0].date;
  const last = days[days.length - 1].date;
  const head = `${first.getFullYear()} 年 ${first.getMonth() + 1} 月 ${first.getDate()} 日`;
  if (first.getFullYear() !== last.getFullYear()) {
    return `${head} – ${last.getFullYear()} 年 ${last.getMonth() + 1} 月 ${last.getDate()} 日`;
  }
  if (first.getMonth() !== last.getMonth()) {
    return `${head} – ${last.getMonth() + 1} 月 ${last.getDate()} 日`;
  }
  return `${head} – ${last.getDate()} 日`;
};

/** 按钟点（0–23）分组，供周视图按时段展示。 */
export const groupItemsByHour = (items: CalendarItem[]): Map<number, CalendarItem[]> => {
  const groups = new Map<number, CalendarItem[]>();
  for (const item of items) {
    const hour = Number(item.time.slice(11, 13)) || 0;
    const list = groups.get(hour);
    if (list) {
      list.push(item);
    } else {
      groups.set(hour, [item]);
    }
  }
  return groups;
};

export type CalendarItemState = "upcoming" | "overdue" | "done" | "fired";

export interface CalendarItem {
  key: string;
  kind: "task" | "recurring";
  /** 本地时间 YYYY-MM-DDTHH:mm:ss；同一天多次的循环提醒取最早一次。 */
  time: string;
  title: string;
  state: CalendarItemState;
  /** 同一循环提醒当天的次数。 */
  count: number;
  task?: Task;
  recurring?: RecurringTask;
}

export interface CalendarInput {
  now: Date;
  /** 含首尾两天的日期键范围 YYYY-MM-DD。 */
  startKey: string;
  endKey: string;
  tasks: Task[];
  completedTasks: Task[];
  recurringTasks: RecurringTask[];
  records: ReminderRecord[];
  previews: RecurringPreview[];
  /** 循环提醒的合并粒度：月视图按天合并，周视图按小时合并。默认按天。 */
  recurringGrouping?: "day" | "hour";
}

const STATE_ORDER: Record<CalendarItemState, number> = { overdue: 0, upcoming: 1, fired: 2, done: 3 };

export const bucketCalendarItems = (input: CalendarInput): Map<string, CalendarItem[]> => {
  const now = toLocalDateTimeString(input.now);
  const inRange = (value?: string | null): value is string => {
    if (!value) {
      return false;
    }
    const key = value.slice(0, 10);
    return key >= input.startKey && key <= input.endKey;
  };
  const buckets = new Map<string, CalendarItem[]>();
  const push = (item: CalendarItem) => {
    const key = item.time.slice(0, 10);
    const list = buckets.get(key);
    if (list) {
      list.push(item);
    } else {
      buckets.set(key, [item]);
    }
  };

  for (const task of input.tasks) {
    // 有截止时间时按截止时间（v2.1），否则按提醒时间。
    const time = taskAnchorTime(task);
    if (!inRange(time)) {
      continue;
    }
    push({
      key: `task-${task.id}`,
      kind: "task",
      time,
      title: task.description,
      state: time <= now ? "overdue" : "upcoming",
      count: 1,
      task,
    });
  }

  for (const task of input.completedTasks) {
    // 有截止 / 提醒时间的按该日期，否则按完成日期。
    const time = taskAnchorTime(task) || task.completedAt;
    if (!inRange(time)) {
      continue;
    }
    push({ key: `task-${task.id}`, kind: "task", time, title: task.description, state: "done", count: 1, task });
  }

  const recurringById = new Map(input.recurringTasks.map(task => [task.id, task]));
  // 循环提醒按“任务 + 日期”合并：已触发的来自提醒记录，未来的来自预估。
  const recurringGroups = new Map<string, CalendarItem>();
  const addRecurring = (taskId: string, time: string, title: string, state: CalendarItemState) => {
    const period = input.recurringGrouping === "hour" ? time.slice(0, 13) : time.slice(0, 10);
    const groupKey = `${taskId}|${period}|${state === "fired" ? "fired" : "upcoming"}`;
    const existing = recurringGroups.get(groupKey);
    if (existing) {
      existing.count += 1;
      if (time < existing.time) {
        existing.time = time;
      }
      return;
    }
    recurringGroups.set(groupKey, {
      key: `recurring-${groupKey}`,
      kind: "recurring",
      time,
      title,
      state,
      count: 1,
      recurring: recurringById.get(taskId),
    });
  };

  for (const record of input.records) {
    if (record.type !== "RECURRING" || !inRange(record.triggerTime) || record.triggerTime > now) {
      continue;
    }
    const title = recurringById.get(record.reminderId)?.description ?? record.description;
    addRecurring(record.reminderId, record.triggerTime, title, "fired");
  }

  for (const preview of input.previews) {
    const recurring = recurringById.get(preview.taskId);
    if (!recurring || recurring.isPaused) {
      continue;
    }
    for (const time of preview.times) {
      if (inRange(time) && time > now) {
        addRecurring(recurring.id, time, recurring.description, "upcoming");
      }
    }
  }

  recurringGroups.forEach(push);

  for (const list of buckets.values()) {
    list.sort((a, b) => a.time.localeCompare(b.time) || STATE_ORDER[a.state] - STATE_ORDER[b.state]);
  }
  return buckets;
};

/** 在日历某天新建待办时的默认提醒：该天 9:00；若已过去（今天）则取下一个整点，更早的日期不设提醒。 */
export const defaultReminderForDay = (day: Date, now: Date): Date | null => {
  const nine = new Date(day.getFullYear(), day.getMonth(), day.getDate(), 9, 0, 0);
  if (nine.getTime() > now.getTime()) {
    return nine;
  }
  if (dateKey(day) !== dateKey(now)) {
    return null;
  }
  const nextHour = new Date(now);
  nextHour.setHours(now.getHours() + 1, 0, 0, 0);
  return dateKey(nextHour) === dateKey(now) ? nextHour : null;
};

/** 把待办拖到另一天：保留原来的钟点（没有提醒时用 9:00）。 */
export const moveReminderToDay = (reminderTime: string | null | undefined, day: Date): string => {
  const clock = reminderTime ? reminderTime.slice(11, 19) : "09:00:00";
  return `${dateKey(day)}T${clock.length === 8 ? clock : `${clock.slice(0, 5)}:00`}`;
};

/** 把待办拖到周视图的某个时段：日期与钟点取目标，分钟保留原值（没有提醒时为整点）。 */
export const moveReminderToSlot = (reminderTime: string | null | undefined, day: Date, hour: number): string => {
  const rest = reminderTime ? reminderTime.slice(14, 19) : "00:00";
  const minutesSeconds = rest.length === 5 ? rest : `${rest.slice(0, 2)}:00`;
  return `${dateKey(day)}T${String(hour).padStart(2, "0")}:${minutesSeconds}`;
};
