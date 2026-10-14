import { describe, expect, it } from "vitest";
import {
  ATTENTION_COLOR_PRESETS,
  ATTENTION_INTENSITY_OPTIONS,
  attentionColorLabel,
  attentionColorVars,
  attentionEdgeVars,
  attentionEffectHint,
  attentionEyebrow,
  attentionIntensityLabel,
  attentionRepeatHint,
  attentionTitle,
  formatScheduledClock,
  missedLabel,
  normalizeAttentionColor,
  normalizeAttentionIntensity,
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
      intensity: 2,
      color: "",
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
      intensity: 5,
      color: "#FF3B30",
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
    expect(payload?.intensity).toBe(5);
    expect(payload?.color).toBe("#ff3b30");
    expect(payload?.count).toBe(2);
    expect(payload?.popupDirection).toBe("left");
  });
});

describe("attention intensity", () => {
  it("keeps known levels and falls back to standard", () => {
    expect(ATTENTION_INTENSITY_OPTIONS.map(item => item.value)).toEqual([1, 2, 3, 4, 5]);
    expect(normalizeAttentionIntensity(4)).toBe(4);
    expect(normalizeAttentionIntensity(0)).toBe(2);
    expect(normalizeAttentionIntensity(9)).toBe(2);
    expect(normalizeAttentionIntensity(2.5)).toBe(2);
    expect(normalizeAttentionIntensity("5")).toBe(2);
    expect(attentionIntensityLabel(5)).toBe("最强");
    expect(attentionIntensityLabel(7)).toBe("标准");
  });

  it("keeps the 2.2.1 look at the standard level", () => {
    expect(attentionEdgeVars(2)).toEqual({
      "--attention-ring-width": "3px",
      "--attention-glow-blur": "48px",
      "--attention-glow-spread": "12px",
      "--attention-soft-blur": "180px",
      "--attention-soft-spread": "56px",
      "--attention-deep-blur": "0vmin",
      "--attention-deep-spread": "0vmin"
    });
  });

  it("reaches further toward the center at higher levels", () => {
    const reach = (level: number) => {
      const vars = attentionEdgeVars(level);
      return ["--attention-soft-blur", "--attention-soft-spread", "--attention-deep-blur", "--attention-deep-spread"].map(
        key => parseFloat(vars[key])
      );
    };
    for (let level = 2; level <= 5; level += 1) {
      const lower = reach(level - 1);
      const higher = reach(level);
      higher.forEach((value, index) => expect(value).toBeGreaterThanOrEqual(lower[index]));
      expect(higher[0]).toBeGreaterThan(lower[0]);
    }
    expect(parseFloat(attentionEdgeVars(5)["--attention-deep-spread"])).toBeGreaterThan(0);
  });
});

describe("attention color", () => {
  it("accepts #rrggbb only, like the backend", () => {
    expect(normalizeAttentionColor(" #1A2b3C ")).toBe("#1a2b3c");
    expect(normalizeAttentionColor("")).toBe("");
    expect(normalizeAttentionColor("red")).toBe("");
    expect(normalizeAttentionColor("#12345")).toBe("");
    expect(normalizeAttentionColor("#ffffff;x")).toBe("");
    expect(normalizeAttentionColor(null)).toBe("");
    expect(ATTENTION_COLOR_PRESETS.every(item => item.value === normalizeAttentionColor(item.value))).toBe(true);
  });

  it("names presets and custom colors", () => {
    expect(attentionColorLabel("")).toBe("默认蓝");
    expect(attentionColorLabel("#FF3B30")).toBe("红色");
    expect(attentionColorLabel("#123abc")).toBe("自定义 #123ABC");
    expect(attentionColorLabel("bad")).toBe("默认蓝");
  });

  it("keeps the theme tokens for the default color", () => {
    expect(attentionColorVars("", "dark")).toEqual({});
    expect(attentionColorVars("bad", "light")).toEqual({});
  });

  it("overrides the glow tokens with the chosen color per theme", () => {
    expect(attentionColorVars("#ff3b30", "dark")).toEqual({
      "--attention-ring": "rgba(255, 59, 48, 0.92)",
      "--attention-glow": "rgba(255, 59, 48, 0.6)",
      "--attention-glow-soft": "rgba(255, 59, 48, 0.24)",
      "--attention-banner-border": "rgba(255, 59, 48, 0.38)"
    });
    expect(attentionColorVars("#30D158", "light")["--attention-glow"]).toBe("rgba(48, 209, 88, 0.5)");
  });
});
