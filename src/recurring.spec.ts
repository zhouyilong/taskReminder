import { describe, expect, it } from "vitest";
import {
  buildRecurringPayload,
  createRecurringDraft,
  draftFromRecurringTask,
  formatRecurringRule,
  validateRecurringDraft
} from "./recurring";
import type { RecurringTask } from "./types";

const base: RecurringTask = {
  id: "r",
  description: "  喝水  ",
  type: "RECURRING",
  status: "PENDING",
  createdAt: "2026-09-01T00:00:00",
  intervalMinutes: 45,
  nextTrigger: "2026-09-26T10:00:00",
  isPaused: false,
  repeatMode: "WEEKLY",
  scheduleTime: "09:30",
  scheduleWeekday: 1,
  scheduleWeekdays: 0b0010101,
};

describe("recurring drafts", () => {
  it("round-trips a weekly task and keeps the legacy first weekday", () => {
    const draft = draftFromRecurringTask(base);
    const payload = buildRecurringPayload(draft);
    expect(payload).toMatchObject({
      description: "喝水",
      repeatMode: "WEEKLY",
      scheduleTime: "09:30",
      scheduleWeekdays: 0b0010101,
      scheduleWeekday: 1,
      startTime: null,
      cronExpression: null,
    });
    expect(formatRecurringRule(base)).toBe("周一、三、五 09:30");
  });

  it("validates per mode", () => {
    const draft = createRecurringDraft();
    expect(validateRecurringDraft(draft)).toBeNull();
    expect(validateRecurringDraft({ ...draft, startTime: "18:00", endTime: "08:00" })).toMatch("开始时间");
    expect(validateRecurringDraft({ ...draft, mode: "WEEKLY", scheduleWeekdays: 0 })).toMatch("至少");
    expect(validateRecurringDraft({ ...draft, mode: "MONTHLY", scheduleDay: 32 })).toMatch("1 到 31");
    expect(validateRecurringDraft({ ...draft, mode: "CRON", cronExpression: " " })).toMatch("Cron");
  });

  it("only sends the fields of the chosen mode", () => {
    const payload = buildRecurringPayload({ ...createRecurringDraft(), mode: "WORKDAY", scheduleTime: "08:30" });
    expect(payload).toMatchObject({ repeatMode: "WORKDAY", scheduleTime: "08:30", startTime: null, endTime: null });
  });
});
