import { describe, expect, it } from "vitest";
import { describeParsedSchedule, parseNumber, parseQuickInput } from "./nlp";
import { toLocalDateTimeString } from "./format";

// 2026-09-27 是周日。
const NOW = new Date(2026, 8, 27, 10, 0, 0);

const at = (input: string, now = NOW) => {
  const parsed = parseQuickInput(input, now);
  return parsed.reminderTime ? toLocalDateTimeString(parsed.reminderTime) : null;
};

describe("parseNumber", () => {
  it("parses arabic and chinese numerals", () => {
    expect(parseNumber("15")).toBe(15);
    expect(parseNumber("两")).toBe(2);
    expect(parseNumber("十")).toBe(10);
    expect(parseNumber("十五")).toBe(15);
    expect(parseNumber("二十三")).toBe(23);
    expect(parseNumber("三十")).toBe(30);
    expect(parseNumber("一二三")).toBeNull();
    expect(parseNumber("abc")).toBeNull();
  });
});

describe("parseQuickInput one-time reminders", () => {
  it("parses day words with period and clock", () => {
    const parsed = parseQuickInput("明天下午3点 交周报", NOW);
    expect(parsed.title).toBe("交周报");
    expect(toLocalDateTimeString(parsed.reminderTime!)).toBe("2026-09-28T15:00:00");
    expect(parsed.recurring).toBeNull();
    expect(parsed.matches).toEqual(["明天", "下午3点"]);
  });

  it("handles half hours, quarters and minutes", () => {
    expect(at("后天上午9点半 体检")).toBe("2026-09-29T09:30:00");
    expect(at("今天晚上8点一刻 看书")).toBe("2026-09-27T20:15:00");
    expect(at("明天10点20分 开会")).toBe("2026-09-28T10:20:00");
    expect(at("明天 17:45 下班打卡")).toBe("2026-09-28T17:45:00");
    expect(at("明天下午3:30 评审")).toBe("2026-09-28T15:30:00");
  });

  it("uses period defaults and day-word periods", () => {
    expect(at("今晚 给妈妈打电话")).toBe("2026-09-27T20:00:00");
    expect(at("明早 跑步")).toBe("2026-09-28T09:00:00");
    expect(at("今晚8点 追剧")).toBe("2026-09-27T20:00:00");
    expect(at("明早7点 赶飞机")).toBe("2026-09-28T07:00:00");
    expect(at("明天 买菜")).toBe("2026-09-28T09:00:00");
    expect(at("明天中午 吃饭")).toBe("2026-09-28T12:00:00");
  });

  it("treats bare 1–6 o'clock as afternoon and rolls past times to tomorrow", () => {
    expect(at("3点 开会")).toBe("2026-09-27T15:00:00");
    expect(at("9点 开会")).toBe("2026-09-28T09:00:00");
    expect(at("凌晨2点 看比赛")).toBe("2026-09-28T02:00:00");
  });

  it("parses relative times", () => {
    expect(at("30分钟后 关火")).toBe("2026-09-27T10:30:00");
    expect(at("半小时后 取快递")).toBe("2026-09-27T10:30:00");
    expect(at("两个小时后 喝水")).toBe("2026-09-27T12:00:00");
    expect(at("一个半小时后 出门")).toBe("2026-09-27T11:30:00");
    expect(at("3天后 还书")).toBe("2026-09-30T09:00:00");
  });

  it("parses weekdays relative to today (Sunday)", () => {
    expect(at("周五 交报告")).toBe("2026-10-02T09:00:00");
    expect(at("星期一下午2点 周会")).toBe("2026-09-28T14:00:00");
    expect(at("下周三 复盘")).toBe("2026-09-30T09:00:00");
    expect(at("周日晚上 总结")).toBe("2026-09-27T20:00:00");
    // 周三：本周三已过，“周一”指下周一；“下周一”同样是下周一。
    const wednesday = new Date(2026, 8, 30, 10, 0, 0);
    expect(at("周一 例会", wednesday)).toBe("2026-10-05T09:00:00");
    expect(at("下周一 例会", wednesday)).toBe("2026-10-05T09:00:00");
    expect(at("这周五 例会", wednesday)).toBe("2026-10-02T09:00:00");
  });

  it("parses explicit dates", () => {
    expect(at("10月1日 国庆出游")).toBe("2026-10-01T09:00:00");
    expect(at("9月1日 开学")).toBe("2027-09-01T09:00:00");
    expect(at("2026-12-31 20:00 跨年")).toBe("2026-12-31T20:00:00");
    expect(at("15号 下午 交房租")).toBe("2026-10-15T15:00:00");
    expect(at("下个月5号 还信用卡")).toBe("2026-10-05T09:00:00");
    expect(at("30号 交水电费")).toBe("2026-09-30T09:00:00");
  });

  it("does not treat unrelated numbers as dates", () => {
    const parsed = parseQuickInput("去3号楼 取资料", NOW);
    expect(parsed.reminderTime).toBeNull();
    expect(parsed.title).toBe("去3号楼 取资料");
  });

  it("leaves plain text untouched", () => {
    const parsed = parseQuickInput("  写周报  ", NOW);
    expect(parsed).toMatchObject({ title: "写周报", reminderTime: null, recurring: null, tags: [], priority: 0 });
  });

  it("strips leading 提醒我", () => {
    expect(parseQuickInput("明天9点提醒我交电费", NOW).title).toBe("交电费");
  });
});

describe("parseQuickInput tags and priority", () => {
  it("extracts tags and priority markers", () => {
    const parsed = parseQuickInput("交周报 #工作 #周报 #工作 !高", NOW);
    expect(parsed.title).toBe("交周报");
    expect(parsed.tags).toEqual(["工作", "周报"]);
    expect(parsed.priority).toBe(3);
  });

  it("supports !! / !!! / numeric priority and full-width marks", () => {
    expect(parseQuickInput("a !!", NOW).priority).toBe(2);
    expect(parseQuickInput("a !!!", NOW).priority).toBe(3);
    expect(parseQuickInput("a !1", NOW).priority).toBe(1);
    expect(parseQuickInput("a ！中 ＃生活", NOW)).toMatchObject({ priority: 2, tags: ["生活"], title: "a" });
  });

  it("ignores exclamation inside words and # without a leading space", () => {
    const parsed = parseQuickInput("Hurry! issue#12", NOW);
    expect(parsed.priority).toBe(0);
    expect(parsed.tags).toEqual([]);
    expect(parsed.title).toBe("Hurry! issue#12");
  });
});

describe("parseQuickInput recurring rules", () => {
  it("parses daily and workday rules", () => {
    const daily = parseQuickInput("每天早上8点 吃药", NOW);
    expect(daily.reminderTime).toBeNull();
    expect(daily.recurring).toMatchObject({ mode: "DAILY", scheduleTime: "08:00", description: "吃药" });

    const workday = parseQuickInput("工作日 9:00 打卡", NOW);
    expect(workday.recurring).toMatchObject({ mode: "WORKDAY", scheduleTime: "09:00", description: "打卡" });
  });

  it("parses weekly rules with lists and ranges", () => {
    expect(parseQuickInput("每周五 17:30 写周报", NOW).recurring).toMatchObject({
      mode: "WEEKLY",
      scheduleWeekdays: 0b0010000,
      scheduleTime: "17:30",
      description: "写周报"
    });
    expect(parseQuickInput("每周一三五 晚上7点 健身", NOW).recurring).toMatchObject({
      scheduleWeekdays: 0b0010101,
      scheduleTime: "19:00"
    });
    expect(parseQuickInput("每周一到周五 站会", NOW).recurring).toMatchObject({
      scheduleWeekdays: 0b0011111,
      scheduleTime: "09:00"
    });
    expect(parseQuickInput("每周末 大扫除", NOW).recurring).toMatchObject({ scheduleWeekdays: 0b1100000 });
    expect(parseQuickInput("每周日 备份", NOW).recurring).toMatchObject({ scheduleWeekdays: 0b1000000 });
  });

  it("parses monthly and interval rules", () => {
    expect(parseQuickInput("每月15号 交房租", NOW).recurring).toMatchObject({
      mode: "MONTHLY",
      scheduleDay: 15,
      description: "交房租"
    });
    expect(parseQuickInput("每隔45分钟 起来活动", NOW).recurring).toMatchObject({
      mode: "INTERVAL_RANGE",
      intervalMinutes: 45
    });
    expect(parseQuickInput("每小时 喝水", NOW).recurring).toMatchObject({ intervalMinutes: 60 });
    expect(parseQuickInput("每半小时 看一眼", NOW).recurring).toMatchObject({ intervalMinutes: 30 });
  });
});

describe("describeParsedSchedule", () => {
  it("describes one-time and recurring results", () => {
    expect(describeParsedSchedule(parseQuickInput("明天下午3点 a", NOW), NOW)).toBe("明天 15:00");
    expect(describeParsedSchedule(parseQuickInput("30分钟后 a", NOW), NOW)).toBe("今天 10:30");
    expect(describeParsedSchedule(parseQuickInput("10月1日 a", NOW), NOW)).toBe("10月1日 周四 09:00");
    expect(describeParsedSchedule(parseQuickInput("每周一三五 19:00 a", NOW), NOW)).toBe("周一、三、五 19:00");
    expect(describeParsedSchedule(parseQuickInput("a", NOW), NOW)).toBe("");
  });
});
