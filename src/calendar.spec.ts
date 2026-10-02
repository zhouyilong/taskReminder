import { describe, expect, it } from "vitest";
import {
  bucketCalendarItems,
  buildMonthGrid,
  buildWeekDays,
  defaultReminderForDay,
  formatWeekRange,
  groupItemsByHour,
  moveReminderToDay,
  moveReminderToSlot
} from "./calendar";
import { toLocalDateTimeString } from "./format";
import type { RecurringTask, ReminderRecord, Task } from "./types";

const NOW = new Date(2026, 8, 27, 10, 0, 0);

const task = (id: string, extra: Partial<Task> = {}): Task => ({
  id,
  description: id,
  type: "ONE_TIME",
  status: "PENDING",
  createdAt: "2026-09-01T00:00:00",
  ...extra,
});

const recurring: RecurringTask = {
  id: "r1",
  description: "喝水",
  type: "RECURRING",
  status: "PENDING",
  createdAt: "2026-09-01T00:00:00",
  intervalMinutes: 60,
  nextTrigger: "2026-09-27T11:00:00",
  isPaused: false,
  repeatMode: "INTERVAL_RANGE",
};

const record = (id: string, triggerTime: string): ReminderRecord => ({
  id,
  reminderId: "r1",
  description: "喝水",
  type: "RECURRING",
  triggerTime,
  action: "DISMISSED",
});

describe("buildMonthGrid", () => {
  it("starts on Monday and always has 6 weeks", () => {
    const grid = buildMonthGrid(2026, 8);
    expect(grid).toHaveLength(42);
    // 2026-09-01 是周二，网格从 8 月 31 日（周一）开始。
    expect(grid[0].key).toBe("2026-08-31");
    expect(grid[0].inMonth).toBe(false);
    expect(grid[1]).toMatchObject({ key: "2026-09-01", inMonth: true });
    expect(grid[5]).toMatchObject({ key: "2026-09-05", isWeekend: true });
    expect(grid[41].key).toBe("2026-10-11");
  });
});

describe("bucketCalendarItems", () => {
  it("buckets tasks, completed tasks and recurring occurrences by day", () => {
    const buckets = bucketCalendarItems({
      now: NOW,
      startKey: "2026-09-01",
      endKey: "2026-09-30",
      tasks: [
        task("overdue", { reminderTime: "2026-09-26T09:00:00" }),
        task("later", { reminderTime: "2026-09-27T15:00:00" }),
        task("outside", { reminderTime: "2026-10-02T09:00:00" }),
        task("no-reminder"),
      ],
      completedTasks: [
        task("done-with-reminder", { status: "COMPLETED", reminderTime: "2026-09-27T08:00:00", completedAt: "2026-09-27T08:05:00" }),
        task("done-plain", { status: "COMPLETED", completedAt: "2026-09-20T18:00:00" }),
      ],
      recurringTasks: [recurring],
      records: [record("a", "2026-09-27T08:00:00"), record("b", "2026-09-27T09:00:00")],
      previews: [{ taskId: "r1", times: ["2026-09-27T11:00:00", "2026-09-27T12:00:00", "2026-09-28T08:00:00"] }],
    });

    expect(buckets.get("2026-09-26")!.map(item => [item.key, item.state])).toEqual([["task-overdue", "overdue"]]);
    expect(buckets.get("2026-09-20")!.map(item => item.state)).toEqual(["done"]);
    expect(buckets.has("2026-10-02")).toBe(false);

    const today = buckets.get("2026-09-27")!;
    expect(today.map(item => [item.title, item.state, item.count, item.time.slice(11, 16)])).toEqual([
      ["喝水", "fired", 2, "08:00"],
      ["done-with-reminder", "done", 1, "08:00"],
      ["喝水", "upcoming", 2, "11:00"],
      ["later", "upcoming", 1, "15:00"],
    ]);
    expect(buckets.get("2026-09-28")![0]).toMatchObject({ kind: "recurring", count: 1, recurring });
  });

  it("skips paused recurring previews", () => {
    const buckets = bucketCalendarItems({
      now: NOW,
      startKey: "2026-09-01",
      endKey: "2026-09-30",
      tasks: [],
      completedTasks: [],
      recurringTasks: [{ ...recurring, isPaused: true }],
      records: [],
      previews: [{ taskId: "r1", times: ["2026-09-28T08:00:00"] }],
    });
    expect(buckets.size).toBe(0);
  });
});

describe("calendar reminders", () => {
  it("picks a default reminder for the selected day", () => {
    expect(toLocalDateTimeString(defaultReminderForDay(new Date(2026, 8, 29), NOW)!)).toBe("2026-09-29T09:00:00");
    expect(toLocalDateTimeString(defaultReminderForDay(new Date(2026, 8, 27), NOW)!)).toBe("2026-09-27T11:00:00");
    expect(defaultReminderForDay(new Date(2026, 8, 26), NOW)).toBeNull();
    expect(defaultReminderForDay(new Date(2026, 8, 27), new Date(2026, 8, 27, 23, 30))).toBeNull();
  });

  it("keeps the clock when moving a reminder to another day", () => {
    expect(moveReminderToDay("2026-09-27T15:30:00", new Date(2026, 9, 1))).toBe("2026-10-01T15:30:00");
    expect(moveReminderToDay(null, new Date(2026, 9, 1))).toBe("2026-10-01T09:00:00");
  });
});

describe("周视图", () => {
  it("从周一开始的 7 天，周日属于上一周", () => {
    const days = buildWeekDays(new Date(2026, 8, 27, 15, 0));
    expect(days.map(day => day.key)).toEqual([
      "2026-09-21",
      "2026-09-22",
      "2026-09-23",
      "2026-09-24",
      "2026-09-25",
      "2026-09-26",
      "2026-09-27"
    ]);
    expect(days[5].isWeekend && days[6].isWeekend).toBe(true);
    expect(buildWeekDays(new Date(2026, 8, 21))[0].key).toBe("2026-09-21");
  });

  it("跨月、跨年的周", () => {
    const crossMonth = buildWeekDays(new Date(2026, 9, 1));
    expect(crossMonth[0].key).toBe("2026-09-28");
    expect(crossMonth[6].key).toBe("2026-10-04");
    expect(formatWeekRange(crossMonth)).toBe("2026 年 9 月 28 日 – 10 月 4 日");

    const crossYear = buildWeekDays(new Date(2026, 11, 31));
    expect(crossYear[0].key).toBe("2026-12-28");
    expect(crossYear[6].key).toBe("2027-01-03");
    expect(formatWeekRange(crossYear)).toBe("2026 年 12 月 28 日 – 2027 年 1 月 3 日");

    expect(formatWeekRange(buildWeekDays(new Date(2026, 8, 23)))).toBe("2026 年 9 月 21 日 – 27 日");
  });

  it("按钟点分组", () => {
    const buckets = bucketCalendarItems({
      now: NOW,
      startKey: "2026-09-27",
      endKey: "2026-09-27",
      tasks: [
        task("a", { reminderTime: "2026-09-27T09:15:00" }),
        task("b", { reminderTime: "2026-09-27T09:45:00" }),
        task("c", { reminderTime: "2026-09-27T14:00:00" })
      ],
      completedTasks: [],
      recurringTasks: [],
      records: [],
      previews: []
    });
    const byHour = groupItemsByHour(buckets.get("2026-09-27") ?? []);
    expect(byHour.get(9)?.map(item => item.task?.id)).toEqual(["a", "b"]);
    expect(byHour.get(14)?.map(item => item.task?.id)).toEqual(["c"]);
    expect(byHour.has(10)).toBe(false);
  });

  it("周视图按小时合并循环提醒", () => {
    const input = {
      now: NOW,
      startKey: "2026-09-27",
      endKey: "2026-09-27",
      tasks: [],
      completedTasks: [],
      recurringTasks: [recurring],
      records: [],
      previews: [
        { taskId: "r1", times: ["2026-09-27T11:00:00", "2026-09-27T11:30:00", "2026-09-27T12:00:00"] }
      ]
    };
    const byDay = bucketCalendarItems(input).get("2026-09-27") ?? [];
    expect(byDay.map(item => item.count)).toEqual([3]);
    const byHour = bucketCalendarItems({ ...input, recurringGrouping: "hour" }).get("2026-09-27") ?? [];
    expect(byHour.map(item => [item.time, item.count])).toEqual([
      ["2026-09-27T11:00:00", 2],
      ["2026-09-27T12:00:00", 1]
    ]);
  });

  it("拖到某个时段保留分钟", () => {
    const day = new Date(2026, 9, 2);
    expect(moveReminderToSlot("2026-09-27T09:30:15", day, 14)).toBe("2026-10-02T14:30:15");
    expect(moveReminderToSlot(null, day, 8)).toBe("2026-10-02T08:00:00");
    expect(moveReminderToSlot("2026-09-27T09:30", day, 0)).toBe("2026-10-02T00:30:00");
  });
});

describe("due time placement (v2.1)", () => {
  it("places tasks on their due day and judges overdue by due time", () => {
    const buckets = bucketCalendarItems({
      now: NOW,
      startKey: "2026-09-01",
      endKey: "2026-09-30",
      tasks: [
        // 提醒在 26 日（已过），截止在 28 日：放在 28 日，尚未逾期。
        task("due-later", { reminderTime: "2026-09-26T18:00:00", dueAt: "2026-09-28T18:00:00" }),
        task("due-past", { dueAt: "2026-09-25T09:00:00" }),
      ],
      completedTasks: [],
      recurringTasks: [],
      records: [],
      previews: [],
    });
    expect(buckets.get("2026-09-26")).toBeUndefined();
    expect(buckets.get("2026-09-28")?.map(item => [item.key, item.state])).toEqual([["task-due-later", "upcoming"]]);
    expect(buckets.get("2026-09-25")?.map(item => item.state)).toEqual(["overdue"]);
  });
});
