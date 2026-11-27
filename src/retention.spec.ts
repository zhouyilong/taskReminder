import { describe, expect, it } from "vitest";
import {
  COMPLETED_RETENTION_OPTIONS,
  completedRetentionHint,
  completedRetentionLabel,
  normalizeAutoBackupKeep,
  retentionCoversRange
} from "./retention";

describe("retention helpers", () => {
  it("covers all supported options", () => {
    expect(COMPLETED_RETENTION_OPTIONS.map(o => o.value)).toEqual([30, 90, 365, 0]);
    expect(completedRetentionLabel(30)).toBe("30 天");
    expect(normalizeAutoBackupKeep(14)).toBe(14);
    expect(normalizeAutoBackupKeep(999)).toBe(7);
  });

  it("explains local vs cloud sync behavior", () => {
    expect(completedRetentionHint(30, false)).toContain("100 条");
    expect(completedRetentionHint(90, true)).toContain("同步到云端");
    expect(completedRetentionHint(0, true)).toContain("不会自动清理");
  });

  it("checks range coverage accurately", () => {
    expect(retentionCoversRange(30, 30)).toBe(true);
    expect(retentionCoversRange(30, 90)).toBe(false);
    expect(retentionCoversRange(0, 999)).toBe(true);
  });
});
