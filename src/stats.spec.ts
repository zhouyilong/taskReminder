import { describe, expect, it } from "vitest";
import { computeStats, computeStreak, rangeDates } from "./stats";
import type { RecurringTask, ReminderRecord } from "./types";

const NOW = new Date(2026, 8, 26, 12, 0, 0);

const record = (
  id: string,
  reminderId: string,
  triggerTime: string,
  action: ReminderRecord["action"],
  type: ReminderRecord["type"] = "RECURRING"
): ReminderRecord => ({ id, reminderId, description: reminderId, type, triggerTime, action });

const habit = (id: string): RecurringTask => ({
  id,
  description: `habit ${id}`,
  type: "RECURRING",
  status: "PENDING",
  createdAt: "2026-09-01T00:00:00",
  intervalMinutes: 60,
  nextTrigger: "2026-09-27T09:00:00",
  isPaused: false,
  repeatMode: "DAILY",
});

describe("rangeDates", () => {
  it("ends with today", () => {
    expect(rangeDates(NOW, 3)).toEqual(["2026-09-24", "2026-09-25", "2026-09-26"]);
  });
});

describe("computeStreak", () => {
  const checks = (entries: [string, boolean][]) => new Map(entries);

  it("counts consecutive reminded days and ignores an unchecked today", () => {
    const result = computeStreak(
      checks([
        ["2026-09-20", true],
        ["2026-09-22", false],
        ["2026-09-23", true],
        ["2026-09-25", true],
        ["2026-09-26", false],
      ]),
      "2026-09-26"
    );
    // 24 日没有提醒（如每周几次），不中断；26 日（今天）尚未打卡，不计入也不中断。
    expect(result).toEqual({ current: 2, best: 2 });
  });

  it("breaks on a missed day before today", () => {
    expect(
      computeStreak(
        checks([
          ["2026-09-23", true],
          ["2026-09-24", true],
          ["2026-09-25", false],
        ]),
        "2026-09-26"
      )
    ).toEqual({ current: 0, best: 2 });
  });
});

describe("computeStats", () => {
  it("aggregates actions, daily buckets, snoozes and habits within the range", () => {
    const records = [
      record("1", "r1", "2026-09-26T09:00:00", "DISMISSED"),
      record("2", "r1", "2026-09-25T09:00:00", "SNOOZED"),
      record("3", "r1", "2026-09-25T09:10:00", "COMPLETED"),
      record("4", "t1", "2026-09-25T18:00:00", "SNOOZED", "TASK"),
      record("5", "t1", "2026-09-24T18:00:00", "SNOOZED", "TASK"),
      record("6", "r1", "2026-09-10T09:00:00", "DISMISSED"),
    ];
    const stats = computeStats({
      now: NOW,
      days: 7,
      records,
      completedTasks: [
        { id: "c", description: "c", type: "ONE_TIME", status: "COMPLETED", createdAt: "2026-09-01T00:00:00", completedAt: "2026-09-25T10:00:00" },
        { id: "old", description: "old", type: "ONE_TIME", status: "COMPLETED", createdAt: "2026-09-01T00:00:00", completedAt: "2026-09-01T10:00:00" },
      ],
      recurringTasks: [habit("r1"), habit("idle")],
    });

    expect(stats.total).toBe(5);
    expect(stats.counts).toEqual({ COMPLETED: 1, DISMISSED: 1, SNOOZED: 3, PENDING: 0 });
    expect(stats.handledRate).toBeCloseTo(2 / 5);
    expect(stats.completedTasks).toBe(1);
    expect(stats.daily).toHaveLength(7);
    expect(stats.daily[6]).toMatchObject({ date: "2026-09-26", total: 1 });
    expect(stats.daily[5].counts).toEqual({ COMPLETED: 1, DISMISSED: 0, SNOOZED: 2, PENDING: 0 });
    expect(stats.topSnoozed.map(item => [item.reminderId, item.count])).toEqual([
      ["t1", 2],
      ["r1", 1],
    ]);
    expect(stats.habits).toHaveLength(1);
    expect(stats.habits[0]).toMatchObject({ current: 2, best: 2, checkedDays: 2, remindedDays: 2 });
  });

  it("reports no rate without records", () => {
    const stats = computeStats({ now: NOW, days: 7, records: [], completedTasks: [], recurringTasks: [] });
    expect(stats.handledRate).toBeNull();
    expect(stats.daily.every(day => day.total === 0)).toBe(true);
  });
});
