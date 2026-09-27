import { describe, expect, it } from "vitest";
import { formatQuietHoursHint } from "./quietHours";

describe("formatQuietHoursHint", () => {
  it("同一天内的时段", () => {
    expect(formatQuietHoursHint("12:00", "14:00")).toBe("12:00 至 14:00：提醒照常记录但不弹窗，结束后一次弹出");
  });

  it("跨午夜的时段标注次日", () => {
    expect(formatQuietHoursHint("22:00", "08:00")).toContain("22:00 至次日 08:00");
  });

  it("开始等于结束或时间无效时提示不生效", () => {
    expect(formatQuietHoursHint("09:00", "09:00")).toContain("不会生效");
    expect(formatQuietHoursHint("", "08:00")).toContain("不会生效");
  });
});
