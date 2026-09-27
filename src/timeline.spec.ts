import { describe, expect, it } from "vitest";
import { buildTodayData, MAX_OCCURRENCES_PER_TASK, nowMarkerIndex } from "./timeline";
import type { RecurringTask, ReminderRecord, Task } from "./types";

// 2026-09-26（周六）14:00，本地时间。
const NOW = new Date(2026, 8, 26, 14, 0, 0);

const task = (id: string, reminderTime: string | null, extra: Partial<Task> = {}): Task => ({
  id,
  description: `task ${id}`,
  type: "ONE_TIME",
  status: "PENDING",
  createdAt: "2026-09-20T09:00:00",
  reminderTime,
  ...extra,
});

const recurring = (id: string, extra: Partial<RecurringTask> = {}): RecurringTask => ({
  id,
  description: `recurring ${id}`,
  type: "RECURRING",
  status: "PENDING",
  createdAt: "2026-09-01T09:00:00",
  intervalMinutes: 30,
  nextTrigger: "2026-09-26T15:00:00",
  isPaused: false,
  repeatMode: "INTERVAL_RANGE",
  ...extra,
});

const record = (id: string, reminderId: string, triggerTime: string, action: ReminderRecord["action"] = "DISMISSED"): ReminderRecord => ({
  id,
  reminderId,
  description: `record ${reminderId}`,
  type: "RECURRING",
  triggerTime,
  action,
});

const build = (input: Partial<Parameters<typeof buildTodayData>[0]>) =>
  buildTodayData({
    now: NOW,
    tasks: [],
    completedTasks: [],
    recurringTasks: [],
    records: [],
    previews: [],
    ...input,
  });

describe("buildTodayData", () => {
  it("merges today's tasks, fired records and previews in time order", () => {
    const data = build({
      tasks: [task("a", "2026-09-26T16:00:00"), task("b", "2026-09-26T09:00:00"), task("c", "2026-09-27T09:00:00")],
      completedTasks: [task("d", "2026-09-26T08:00:00", { status: "COMPLETED", completedAt: "2026-09-26T08:10:00" })],
      recurringTasks: [recurring("r")],
      records: [record("x", "r", "2026-09-26T10:00:00")],
      previews: [{ taskId: "r", times: ["2026-09-26T15:00:00"] }],
    });
    expect(data.entries.map(entry => [entry.time.slice(11, 16), entry.state])).toEqual([
      ["08:00", "done"],
      ["09:00", "overdue"],
      ["10:00", "DISMISSED"],
      ["15:00", "upcoming"],
      ["16:00", "upcoming"],
    ]);
    expect(data.summary).toEqual({ total: 5, upcoming: 2, overdue: 1, completedToday: 1 });
    expect(nowMarkerIndex(data.entries, NOW)).toBe(3);
  });

  it("lists tasks overdue from earlier days separately", () => {
    const data = build({ tasks: [task("old", "2026-09-24T10:00:00"), task("none", null)] });
    expect(data.overdueTasks.map(item => item.id)).toEqual(["old"]);
    expect(data.entries).toHaveLength(0);
    expect(data.summary.overdue).toBe(1);
  });

  it("collapses frequent recurring reminders", () => {
    const times = ["15:00", "15:30", "16:00", "16:30", "17:00"].map(time => `2026-09-26T${time}:00`);
    const fired = ["10:00", "10:30", "11:00", "11:30"].map((time, index) => record(`f${index}`, "r", `2026-09-26T${time}:00`));
    const data = build({
      recurringTasks: [recurring("r")],
      records: fired,
      previews: [{ taskId: "r", times }],
    });
    expect(times.length).toBeGreaterThan(MAX_OCCURRENCES_PER_TASK);
    expect(data.entries.map(entry => [entry.time.slice(11, 16), entry.collapsedCount])).toEqual([
      ["11:30", 3],
      ["15:00", 4],
    ]);
    expect(data.summary.total).toBe(9);
    expect(data.summary.upcoming).toBe(5);
  });

  it("skips paused and deleted recurring previews but keeps fired records", () => {
    const data = build({
      recurringTasks: [recurring("paused", { isPaused: true })],
      records: [record("x", "gone", "2026-09-26T09:00:00", "SNOOZED")],
      previews: [
        { taskId: "paused", times: ["2026-09-26T15:00:00"] },
        { taskId: "gone", times: ["2026-09-26T15:00:00"] },
      ],
    });
    expect(data.entries).toHaveLength(1);
    expect(data.entries[0].title).toBe("record gone");
  });

  it("places a due-but-not-fired preview before the now marker", () => {
    const data = build({
      recurringTasks: [recurring("r")],
      previews: [{ taskId: "r", times: ["2026-09-26T13:59:50", "2026-09-26T18:00:00"] }],
    });
    expect(data.entries[0].isPast).toBe(true);
    expect(data.entries[0].state).toBe("upcoming");
    expect(nowMarkerIndex(data.entries, NOW)).toBe(1);
  });
});
