import { describe, expect, it } from "vitest";
import { completedRetentionHint, normalizeCompletedRetention, retentionCoversRange } from "./retention";

describe("已完成待办保留期", () => {
  it("不认识的值按 30 天处理", () => {
    expect(normalizeCompletedRetention(90)).toBe(90);
    expect(normalizeCompletedRetention(0)).toBe(0);
    expect(normalizeCompletedRetention(45)).toBe(30);
    expect(normalizeCompletedRetention(undefined)).toBe(30);
  });

  it("说明文案区分默认档位、永久与同步", () => {
    expect(completedRetentionHint(30, false)).toBe("完成超过 30 天的待办并只保留最近 100 条，自动移入回收站");
    expect(completedRetentionHint(365, false)).toBe("完成超过 1 年的待办自动移入回收站");
    expect(completedRetentionHint(0, false)).toBe("已完成的待办不会自动清理");
    expect(completedRetentionHint(90, true)).toContain("以所有设备中最短的为准");
  });

  it("判断统计范围是否在保留期内", () => {
    expect(retentionCoversRange(30, 30)).toBe(true);
    expect(retentionCoversRange(30, 84)).toBe(false);
    expect(retentionCoversRange(90, 84)).toBe(true);
    expect(retentionCoversRange(0, 365)).toBe(true);
  });
});
