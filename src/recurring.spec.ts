import { describe, expect, it } from "vitest";
import {
  formatWorkdayHint,
  formatYearRange,
  canSkipRecurring,
  isSupportedRecurringMode,
  buildRecurringPayload,
  createRecurringDraft,
  draftFromRecurringTask,
  formatRecurringMode,
  formatRecurringRule,
  validateRecurringDraft,
  firstUncoveredHolidayYear,
  formatRecurringEnd,
  isRecurringEnded,
  recurringStatus,
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

describe("unsupported recurring modes", () => {
  // 来自更新版本设备的模式：后端原样保留，界面标为需升级。
  const future = { ...base, repeatMode: "BIWEEKLY" } as unknown as RecurringTask;

  it("detects modes this version does not know", () => {
    expect(isSupportedRecurringMode("WORKDAY")).toBe(true);
    expect(isSupportedRecurringMode("BIWEEKLY")).toBe(false);
    expect(isSupportedRecurringMode(undefined)).toBe(false);
  });

  it("does not describe them as interval reminders", () => {
    expect(formatRecurringMode("BIWEEKLY")).toBe("不支持的模式");
    expect(formatRecurringMode("WORKDAY")).toBe("法定工作日");
    expect(formatRecurringMode(null)).toBe("区间间隔");
    expect(formatRecurringRule(future)).toBe("不支持的循环模式（BIWEEKLY），请升级应用");
    expect(formatRecurringRule({ ...base, repeatMode: "INTERVAL_RANGE", startTime: "08:00", endTime: "18:00" })).toBe(
      "每 45 分钟（08:00 - 18:00）"
    );
  });
});

describe("canSkipRecurring", () => {
  const base = { isPaused: false, repeatMode: "DAILY" as const, nextTrigger: "2026-10-02T09:00:00" };
  it("allows running tasks with a known mode", () => {
    expect(canSkipRecurring(base)).toBe(true);
  });
  it("rejects paused, unknown-mode or unscheduled tasks", () => {
    expect(canSkipRecurring({ ...base, isPaused: true })).toBe(false);
    expect(canSkipRecurring({ ...base, repeatMode: "BIWEEKLY" as never })).toBe(false);
    expect(canSkipRecurring({ ...base, nextTrigger: "" })).toBe(false);
  });
});

describe("month-end modes and tags", () => {
  it("builds payloads for month-end modes with normalized tags", () => {
    const draft = { ...createRecurringDraft(), description: "交房租", mode: "MONTHLY_LAST_DAY" as const, scheduleTime: "20:00", tags: ["#生活", "生活", "账单"] };
    expect(validateRecurringDraft(draft)).toBeNull();
    const payload = buildRecurringPayload(draft);
    expect(payload).toMatchObject({ repeatMode: "MONTHLY_LAST_DAY", scheduleTime: "20:00", scheduleDay: null, tags: ["生活", "账单"] });
    expect(validateRecurringDraft({ ...draft, mode: "MONTHLY_LAST_WORKDAY", scheduleTime: "" })).not.toBeNull();
  });

  it("describes and warns for month-end workday reminders", () => {
    const task = { repeatMode: "MONTHLY_LAST_WORKDAY", scheduleTime: "17:00", isPaused: false, nextTrigger: "2026-12-31T17:00:00" } as never;
    expect(formatRecurringRule(task)).toBe("每月最后一个工作日 17:00");
    expect(workdayHolidayWarning(task, [2025, 2026], new Date(2026, 11, 20))).toMatch("2027");
  });
});

describe("recurring end condition", () => {
  const NOW = new Date(2026, 9, 3, 10, 0, 0);

  it("round-trips the end date and count through drafts and payloads", () => {
    const task: RecurringTask = { ...base, endsOn: "2026-12-31", remainingCount: 3 };
    const draft = draftFromRecurringTask(task);
    expect([draft.endsOn, draft.count]).toEqual(["2026-12-31", 3]);
    expect(buildRecurringPayload(draft)).toMatchObject({ endsOn: "2026-12-31", remainingCount: 3 });
    // 输入框清空时 v-model.number 给出空字符串。
    const cleared = { ...draft, endsOn: "", count: "" as unknown as number };
    expect(buildRecurringPayload(cleared)).toMatchObject({ endsOn: null, remainingCount: null });
    expect(buildRecurringPayload(createRecurringDraft())).toMatchObject({ endsOn: null, remainingCount: null });
  });

  it("validates the count and date", () => {
    const draft = createRecurringDraft();
    expect(validateRecurringDraft({ ...draft, count: 0 })).toBeNull();
    expect(validateRecurringDraft({ ...draft, count: 1.5 })).toMatch("整数");
    expect(validateRecurringDraft({ ...draft, count: -1 })).toMatch("整数");
    expect(validateRecurringDraft({ ...draft, count: 10000 })).toMatch("整数");
    expect(validateRecurringDraft({ ...draft, endsOn: "12/31" })).toMatch("结束日期");
  });

  it("matches the backend's ended rule and labels the status", () => {
    const running = { ...base, endsOn: "2026-09-26", remainingCount: null };
    expect(isRecurringEnded(running)).toBe(false);
    expect(isRecurringEnded({ ...running, nextTrigger: "2026-09-27T09:30:00" })).toBe(true);
    expect(isRecurringEnded({ ...base, remainingCount: 0 })).toBe(true);
    expect(isRecurringEnded({ ...base, remainingCount: 1 })).toBe(false);

    expect(recurringStatus({ ...base, remainingCount: 0, isPaused: true })).toBe("ended");
    expect(recurringStatus({ ...base, remainingCount: 2, isPaused: true })).toBe("paused");
    expect(recurringStatus({ ...base, repeatMode: "BIWEEKLY" as never })).toBe("unsupported");
    expect(recurringStatus(base)).toBe("running");
  });

  it("describes the end condition briefly", () => {
    expect(formatRecurringEnd(base, NOW)).toBe("");
    expect(formatRecurringEnd({ endsOn: "2026-12-31" }, NOW)).toBe("到 12月31日");
    expect(formatRecurringEnd({ endsOn: "2027-01-31", remainingCount: 3 }, NOW)).toBe("到 2027年1月31日，剩 3 次");
    expect(formatRecurringEnd({ remainingCount: -1 }, NOW)).toBe("剩 0 次");
  });
});
