import { describe, expect, it } from "vitest";
import { monthsAgoCutoff, recordsBefore } from "./records";

describe("monthsAgoCutoff", () => {
  it("取半年前当天的零点", () => {
    expect(monthsAgoCutoff(new Date(2026, 10, 6, 15, 30))).toBe("2026-05-06T00:00:00");
    expect(monthsAgoCutoff(new Date(2026, 2, 15, 8))).toBe("2025-09-15T00:00:00");
  });

  it("目标月份没有这一天时取月末", () => {
    expect(monthsAgoCutoff(new Date(2026, 7, 31))).toBe("2026-02-28T00:00:00");
    expect(monthsAgoCutoff(new Date(2028, 7, 31))).toBe("2028-02-29T00:00:00");
    expect(monthsAgoCutoff(new Date(2026, 11, 31), 3)).toBe("2026-09-30T00:00:00");
  });
});

describe("recordsBefore", () => {
  it("只保留触发时间早于截止时间的记录", () => {
    const records = [
      { id: "a", triggerTime: "2026-05-05T23:59:59" },
      { id: "b", triggerTime: "2026-05-06T00:00:00" },
      { id: "c", triggerTime: "2026-01-01T08:00:00.123" }
    ];
    expect(recordsBefore(records, "2026-05-06T00:00:00").map(record => record.id)).toEqual(["a", "c"]);
  });
});
