import { describe, expect, it } from "vitest";
import {
  formatWorkdayHint,
  formatYearRange,
  buildRecurringPayload,
  createRecurringDraft,
  draftFromRecurringTask,
  formatRecurringRule,
  validateRecurringDraft,
  firstUncoveredHolidayYear,
  workdayDraftWarning,
  workdayHolidayWarning
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

describe("节假日数据覆盖提示", () => {
  const years = [2025, 2026];

  it("30 天内都已覆盖时不提示", () => {
    expect(firstUncoveredHolidayYear(new Date(2026, 8, 27), years)).toBeNull();
    expect(firstUncoveredHolidayYear(new Date(2026, 11, 1), years)).toBeNull();
  });

  it("30 天内跨入未覆盖的年份时返回该年份", () => {
    expect(firstUncoveredHolidayYear(new Date(2026, 11, 5), years)).toBe(2027);
    expect(firstUncoveredHolidayYear(new Date(2027, 5, 1), years)).toBe(2027);
    expect(firstUncoveredHolidayYear(new Date(2024, 5, 1), years)).toBe(2024);
  });

  it("数据未加载时不提示", () => {
    expect(firstUncoveredHolidayYear(new Date(2030, 0, 1), [])).toBeNull();
  });

  it("只提示运行中的法定工作日提醒，从下次触发起算", () => {
    const now = new Date(2026, 8, 27, 10, 0);
    const task = { repeatMode: "WORKDAY" as const, isPaused: false, nextTrigger: "2026-12-10T09:00:00" };
    expect(workdayHolidayWarning(task, years, now)).toBe("2027 年节假日安排尚未内置，暂按周一至周五计算");
    expect(workdayHolidayWarning({ ...task, nextTrigger: "2026-09-28T09:00:00" }, years, now)).toBeNull();
    expect(workdayHolidayWarning({ ...task, isPaused: true }, years, now)).toBeNull();
    expect(workdayHolidayWarning({ ...task, repeatMode: "DAILY" }, years, now)).toBeNull();
  });

  it("下次触发已过期时从现在起算", () => {
    const now = new Date(2026, 11, 20, 10, 0);
    const task = { repeatMode: "WORKDAY" as const, isPaused: false, nextTrigger: "2026-09-01T09:00:00" };
    expect(workdayHolidayWarning(task, years, now)).toContain("2027");
  });

  it("新建表单按当前时间判断", () => {
    expect(workdayDraftWarning(years, new Date(2026, 8, 27))).toBeNull();
    expect(workdayDraftWarning(years, new Date(2026, 11, 20))).toContain("2027");
  });
});

describe("formatYearRange", () => {
  it("formats the covered years", () => {
    expect(formatYearRange([])).toBe("暂无");
    expect(formatYearRange([2026])).toBe("2026 年");
    expect(formatYearRange([2027, 2025, 2026])).toBe("2025–2027 年");
  });

  it("is used by the workday hint", () => {
    expect(formatWorkdayHint([2025, 2026])).toContain("已有 2025–2026 年安排");
    expect(formatWorkdayHint([])).toContain("暂无节假日数据");
  });
});
