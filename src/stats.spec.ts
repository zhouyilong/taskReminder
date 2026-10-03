import { describe, expect, it } from "vitest";
import { computeStats, computeStreak, rangeDates, filterHabitsByTag } from "./stats";
import type { RecurringTask, ReminderRecord, Task } from "./types";

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
    expect(stats.granularity).toBe("day");
    expect(stats.buckets).toHaveLength(7);
    expect(stats.buckets[6]).toMatchObject({ date: "2026-09-26", endDate: "2026-09-26", total: 1, completions: 0 });
    expect(stats.buckets[5].counts).toEqual({ COMPLETED: 1, DISMISSED: 0, SNOOZED: 2, PENDING: 0 });
    // 9 月 25 日：完成的待办 c + 循环提醒 r1 的完成记录。
    expect(stats.buckets[5].completions).toBe(2);
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
    expect(stats.buckets.every(day => day.total === 0)).toBe(true);
    expect(stats.tags).toEqual([]);
  });

  const task = (id: string, tags: string[], completedAt?: string): Task => ({
    id,
    description: id,
    type: "ONE_TIME",
    status: completedAt ? "COMPLETED" : "PENDING",
    createdAt: "2026-09-01T00:00:00",
    completedAt,
    tags,
  });

  it("summarizes reminders by the tags of their source", () => {
    const records = [
      record("1", "r1", "2026-09-26T09:00:00", "COMPLETED"),
      record("2", "r1", "2026-09-25T09:00:00", "SNOOZED"),
      record("3", "t1", "2026-09-25T18:00:00", "DISMISSED", "TASK"),
      record("4", "done", "2026-09-24T18:00:00", "COMPLETED", "TASK"),
      record("5", "gone", "2026-09-24T19:00:00", "SNOOZED", "TASK"),
      record("6", "plain", "2026-09-24T20:00:00", "DISMISSED", "TASK"),
      record("7", "t1", "2026-09-26T18:00:00", "PENDING", "TASK"),
    ];
    const stats = computeStats({
      now: NOW,
      days: 7,
      records,
      activeTasks: [task("t1", ["工作", "周报"]), task("plain", [])],
      completedTasks: [task("done", ["工作"], "2026-09-24T18:00:00")],
      recurringTasks: [{ ...habit("r1"), tags: ["健康"] }],
    });
    expect(stats.tags.map(item => [item.tag, item.total, item.counts.COMPLETED, item.counts.SNOOZED])).toEqual([
      ["工作", 3, 1, 0],
      ["健康", 2, 1, 1],
      ["周报", 2, 0, 0],
      // 没有标签的待办与来源已删除的提醒。
      ["", 2, 0, 1],
    ]);
    expect(stats.tags[0].handledRate).toBeCloseTo(2 / 3);
    expect(stats.tags[1].handledRate).toBe(0.5);
  });

  it("matches tags case-insensitively", () => {
    const stats = computeStats({
      now: NOW,
      days: 7,
      records: [record("1", "a", "2026-09-26T09:00:00", "DISMISSED", "TASK"), record("2", "b", "2026-09-26T10:00:00", "DISMISSED", "TASK")],
      activeTasks: [task("a", ["Work"]), task("b", ["work"])],
      completedTasks: [],
      recurringTasks: [],
    });
    expect(stats.tags).toHaveLength(1);
    expect(stats.tags[0]).toMatchObject({ tag: "Work", total: 2 });
  });

  it("groups by week for the 12-week range, ending today", () => {
    // NOW 为 2026-09-26（周六）；最近 12 周从 2026-07-06（周一）开始。
    const stats = computeStats({
      now: NOW,
      days: 84,
      granularity: "week",
      records: [
        record("1", "r1", "2026-09-21T09:00:00", "COMPLETED"),
        record("2", "r1", "2026-09-26T09:00:00", "COMPLETED"),
        record("3", "r1", "2026-09-20T09:00:00", "SNOOZED"),
        record("4", "r1", "2026-07-05T09:00:00", "COMPLETED"),
      ],
      completedTasks: [task("a", [], "2026-07-06T08:00:00"), task("b", [], "2026-09-22T08:00:00")],
      recurringTasks: [habit("r1")],
    });
    expect(stats.granularity).toBe("week");
    expect(stats.buckets).toHaveLength(12);
    expect(stats.buckets[0]).toMatchObject({ date: "2026-07-06", endDate: "2026-07-12", completions: 1, total: 0 });
    expect(stats.buckets[10]).toMatchObject({ date: "2026-09-14", endDate: "2026-09-20", total: 1, completions: 0 });
    expect(stats.buckets[11]).toMatchObject({ date: "2026-09-21", endDate: "2026-09-26", total: 2, completions: 3 });
    expect(stats.total).toBe(3);
  });

  it("splits weeks across the new year", () => {
    const stats = computeStats({
      now: new Date(2027, 0, 3, 12),
      days: 14,
      granularity: "week",
      records: [record("1", "r1", "2026-12-31T09:00:00", "DISMISSED"), record("2", "r1", "2026-12-27T09:00:00", "DISMISSED")],
      completedTasks: [],
      recurringTasks: [habit("r1")],
    });
    expect(stats.buckets.map(bucket => [bucket.date, bucket.endDate, bucket.total])).toEqual([
      ["2026-12-21", "2026-12-27", 1],
      ["2026-12-28", "2027-01-03", 1],
    ]);
  });
});

describe("filterHabitsByTag", () => {
  const habit = (id: string, tags: string[]) =>
    ({ task: { id, tags }, current: 0, best: 0, checkedDays: 0, remindedDays: 1 }) as never;
  it("keeps habits whose recurring task has the tag", () => {
    const habits = [habit("a", ["健康"]), habit("b", ["工作"]), habit("c", [])];
    expect(filterHabitsByTag(habits, "健康").map((item: { task: { id: string } }) => item.task.id)).toEqual(["a"]);
    expect(filterHabitsByTag(habits, "")).toHaveLength(3);
  });
});
