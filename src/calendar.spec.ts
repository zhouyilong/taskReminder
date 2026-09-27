import { describe, expect, it } from "vitest";
import { bucketCalendarItems, buildMonthGrid, defaultReminderForDay, moveReminderToDay } from "./calendar";
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
