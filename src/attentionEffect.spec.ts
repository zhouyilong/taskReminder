import { describe, expect, it } from "vitest";
import {
  attentionEffectHint,
  attentionEyebrow,
  attentionRepeatHint,
  attentionTitle,
  formatScheduledClock,
  missedLabel,
  normalizeAttentionPayload,
  popupHint,
  resolveAttentionTheme
} from "./attentionEffect";

const now = new Date(2026, 9, 13, 10, 0, 0);

describe("attention effect texts", () => {
  it("describes each effect and duration in settings", () => {
    expect(attentionEffectHint("off", false)).toBe("到点时只弹出提醒窗口");
    expect(attentionEffectHint("edge", false)).toBe("每块屏幕边缘亮起柔光，约 6 秒后淡出；不挡操作、不抢焦点，勿扰时段内不播放");
    expect(attentionEffectHint("banner", true)).toContain("顶部显示提醒内容，直到处理完提醒");
  });

  it("describes repeat reminders", () => {
    expect(attentionRepeatHint(0, "banner")).toBe("提醒弹出后不再重复");
    expect(attentionRepeatHint(5, "banner")).toBe(
      "提醒没有处理时，每 5 分钟把弹窗移到鼠标所在屏幕、重播全屏效果，开启提示音时再响一次"
    );
    expect(attentionRepeatHint(2, "off")).not.toContain("全屏效果");
  });

  it("marks missed reminders with the scheduled time", () => {
    expect(missedLabel(null, now)).toBe("");
    expect(missedLabel("not a time", now)).toBe("");
    expect(missedLabel("2026-10-13T09:59:00", now)).toBe("");
    expect(missedLabel("2026-10-13T09:58:00", now)).toBe("原定 09:58");
    expect(missedLabel("2026-10-12T21:05:00", now)).toBe("原定 10月12日 21:05");
    expect(formatScheduledClock(new Date(2026, 9, 13, 8, 5), now)).toBe("08:05");
  });

  it("picks the eyebrow by preview, repeat, missed and kind", () => {
    const base = { preview: false, repeat: false, reminderType: "TASK", scheduledTime: "2026-10-13T10:00:00" };
    expect(attentionEyebrow(base, now)).toBe("任务提醒");
    expect(attentionEyebrow({ ...base, reminderType: "RECURRING" }, now)).toBe("循环提醒");
    expect(attentionEyebrow({ ...base, scheduledTime: "2026-10-13T08:30:00" }, now)).toBe("错过的提醒 · 原定 08:30");
    expect(attentionEyebrow({ ...base, repeat: true, scheduledTime: "2026-10-13T08:30:00" }, now)).toBe(
      "任务提醒 · 还没有处理"
    );
    expect(attentionEyebrow({ ...base, preview: true, repeat: true }, now)).toBe("效果预览");
  });

  it("uses plain text for the title", () => {
    expect(attentionTitle("- **开会** 讨论 [方案](https://example.com)")).toBe("开会 讨论 方案");
    expect(attentionTitle("")).toBe("提醒");
    expect(attentionTitle(null)).toBe("提醒");
  });

  it("points to the screen with the popup", () => {
    expect(popupHint("here", 1)).toBe("在右下角处理");
    expect(popupHint("left", 1)).toBe("提醒窗口在左侧屏幕");
    expect(popupHint("down", 3)).toBe("提醒窗口在下方屏幕 · 共 3 条");
    expect(popupHint("elsewhere", 2)).toBe("在右下角处理 · 共 2 条");
  });
});

describe("resolveAttentionTheme", () => {
  it("follows explicit, app and system themes", () => {
    expect(resolveAttentionTheme("light", "dark", false)).toBe("light");
    expect(resolveAttentionTheme("dark", "light", true)).toBe("dark");
    expect(resolveAttentionTheme("app", "light", false)).toBe("light");
    expect(resolveAttentionTheme("app", null, true)).toBe("light");
    expect(resolveAttentionTheme("system", "dark", true)).toBe("light");
    expect(resolveAttentionTheme(undefined, undefined, false)).toBe("dark");
  });
});

describe("normalizeAttentionPayload", () => {
  it("fills defaults for missing or invalid fields", () => {
    expect(normalizeAttentionPayload(undefined)).toBeNull();
    expect(normalizeAttentionPayload("x")).toBeNull();
    expect(normalizeAttentionPayload({ effect: "fireworks", count: 0 })).toEqual({
      effect: "banner",
      untilHandled: false,
      reminderType: "TASK",
      description: "",
      scheduledTime: null,
      count: 1,
      repeat: false,
      preview: false,
      theme: "app",
      popupDirection: "here"
    });
    const payload = normalizeAttentionPayload({
      effect: "edge",
      untilHandled: true,
      reminderType: "RECURRING",
      description: "喝水",
      scheduledTime: "2026-10-13T10:00:00",
      count: 2,
      repeat: true,
      preview: false,
      theme: "dark",
      popupDirection: "left"
    });
    expect(payload?.effect).toBe("edge");
    expect(payload?.untilHandled).toBe(true);
    expect(payload?.count).toBe(2);
    expect(payload?.popupDirection).toBe("left");
  });
});
