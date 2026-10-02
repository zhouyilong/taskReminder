// 自然语言输入解析：从“明天下午3点 交周报 #工作 !高”这类文本中识别提醒时间、
// 循环规则、标签与优先级，剩余文本作为标题。纯函数，规则 + 正则覆盖常见中文表达。
import { createRecurringDraft, type RecurringDraft } from "./recurring";
import { WEEKDAY_MASK_ALL, WEEKDAY_MASK_WEEKEND, formatWeekdayMask, weekdayBit } from "./weekdays";

export type Priority = 0 | 1 | 2 | 3;

export interface ParsedInput {
  /** 去掉识别出的时间、标签、优先级后的标题。 */
  title: string;
  /** 一次性提醒时间；识别出循环规则时为 null。 */
  reminderTime: Date | null;
  /** 循环规则（描述取 title）；为 null 表示一次性待办。 */
  recurring: RecurringDraft | null;
  tags: string[];
  priority: Priority;
  /** 被识别并从标题中移除的原文片段，按出现顺序。 */
  matches: string[];
}

const CN_DIGITS: Record<string, number> = {
  零: 0,
  〇: 0,
  一: 1,
  二: 2,
  两: 2,
  三: 3,
  四: 4,
  五: 5,
  六: 6,
  七: 7,
  八: 8,
  九: 9,
};

/** 解析阿拉伯数字或 99 以内的中文数字（十五、二十三、两）。 */
export const parseNumber = (raw: string): number | null => {
  const text = raw.trim();
  if (!text) {
    return null;
  }
  if (/^\d+$/.test(text)) {
    return Number(text);
  }
  if (!/^[零〇一二两三四五六七八九十]+$/.test(text)) {
    return null;
  }
  const tenIndex = text.indexOf("十");
  if (tenIndex === -1) {
    return text.length === 1 ? CN_DIGITS[text] : null;
  }
  const tensText = text.slice(0, tenIndex);
  const onesText = text.slice(tenIndex + 1);
  if (tensText.length > 1 || onesText.length > 1) {
    return null;
  }
  const tens = tensText ? CN_DIGITS[tensText] : 1;
  const ones = onesText ? CN_DIGITS[onesText] : 0;
  if (tens === undefined || ones === undefined) {
    return null;
  }
  return tens * 10 + ones;
};

const NUM = "(?:\\d{1,2}|[零〇一二两三四五六七八九十]{1,3})";
const WEEKDAY_CHAR = "[一二三四五六日天七1-7]";
const WEEK_WORD = "(?:周|星期|礼拜)";

const weekdayFromChar = (value: string): number | null => {
  if (value === "日" || value === "天" || value === "七" || value === "7") {
    return 7;
  }
  const parsed = parseNumber(value);
  return parsed && parsed >= 1 && parsed <= 7 ? parsed : null;
};

type Period = "dawn" | "morning" | "noon" | "afternoon" | "evening" | "night";

const PERIOD_WORDS: Record<string, Period> = {
  凌晨: "dawn",
  早上: "morning",
  早晨: "morning",
  上午: "morning",
  中午: "noon",
  下午: "afternoon",
  傍晚: "evening",
  晚上: "night",
  晚间: "night",
  夜里: "night",
  夜间: "night",
};

/** 只说了时段、没说几点时的默认时间。 */
const PERIOD_DEFAULT_HOUR: Record<Period, number> = {
  dawn: 6,
  morning: 9,
  noon: 12,
  afternoon: 15,
  evening: 18,
  night: 20,
};

const applyPeriod = (hour: number, period: Period | null): number => {
  switch (period) {
    case "afternoon":
    case "evening":
    case "night":
      return hour < 12 ? hour + 12 : hour;
    case "noon":
      return hour <= 2 ? hour + 12 : hour;
    case "dawn":
    case "morning":
      return hour === 12 ? 0 : hour;
    default:
      // 没有时段：1–6 点多半指下午（“3点开会”），7 点及以后按字面。
      return hour >= 1 && hour <= 6 ? hour + 12 : hour;
  }
};

interface Scanner {
  text: string;
  matches: { index: number; value: string }[];
}

/** 找到第一处匹配后从文本中抹掉（替换为空格，保持其他位置不变）。 */
const take = (scanner: Scanner, pattern: RegExp): RegExpExecArray | null => {
  const match = pattern.exec(scanner.text);
  if (!match) {
    return null;
  }
  const value = match[0];
  scanner.matches.push({ index: match.index, value: value.trim() });
  scanner.text =
    scanner.text.slice(0, match.index) + " ".repeat(value.length) + scanner.text.slice(match.index + value.length);
  return match;
};

const startOfDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate());

const addDays = (date: Date, days: number) => {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
};

/** 周几（1 = 周一 … 7 = 周日）。 */
const isoWeekday = (date: Date) => (date.getDay() === 0 ? 7 : date.getDay());

const pad = (value: number) => String(value).padStart(2, "0");

const parseTags = (scanner: Scanner): string[] => {
  const tags: string[] = [];
  const pattern = /(^|\s)[#＃]([^\s#＃,，]{1,24})/;
  let match = take(scanner, pattern);
  while (match) {
    const tag = match[2];
    if (!tags.some(existing => existing.toLowerCase() === tag.toLowerCase())) {
      tags.push(tag);
    }
    match = take(scanner, pattern);
  }
  return tags;
};

const parsePriority = (scanner: Scanner): Priority => {
  const match = take(scanner, /(^|\s)[!！](高|中|低|[123]|[!！]{1,2})(?=\s|$)/);
  if (!match) {
    return 0;
  }
  switch (match[2]) {
    case "高":
    case "3":
      return 3;
    case "中":
    case "2":
      return 2;
    case "低":
    case "1":
      return 1;
    default:
      // “!!” → 中，“!!!” → 高。
      return match[2].length >= 2 ? 3 : 2;
  }
};

interface ClockTime {
  hour: number;
  minute: number;
}

/** 识别时段与钟点，如“下午3点半”“晚上8:30”“9点”“中午”。 */
const parseClock = (scanner: Scanner): { time: ClockTime | null; period: Period | null } => {
  const periodPattern = Object.keys(PERIOD_WORDS).join("|");
  const colon = take(scanner, new RegExp(`(${periodPattern})?\\s*(\\d{1,2})\\s*[:：]\\s*(\\d{2})(?!\\d)`));
  if (colon) {
    const period = colon[1] ? PERIOD_WORDS[colon[1]] : null;
    const hour = Number(colon[2]);
    const minute = Number(colon[3]);
    if (hour <= 23 && minute <= 59) {
      // 冒号写法按 24 小时制；只有带时段时才换算（“下午3:30”）。
      return { time: { hour: period ? applyPeriod(hour, period) : hour, minute }, period };
    }
  }
  const spoken = take(
    scanner,
    new RegExp(`(${periodPattern})?\\s*(${NUM})\\s*[点时](?:\\s*(半|一刻|三刻|(${NUM})\\s*分?))?`)
  );
  if (spoken) {
    const period = spoken[1] ? PERIOD_WORDS[spoken[1]] : null;
    const hour = parseNumber(spoken[2]);
    let minute = 0;
    if (spoken[3] === "半") {
      minute = 30;
    } else if (spoken[3] === "一刻") {
      minute = 15;
    } else if (spoken[3] === "三刻") {
      minute = 45;
    } else if (spoken[4]) {
      minute = parseNumber(spoken[4]) ?? 0;
    }
    if (hour !== null && hour <= 24 && minute <= 59) {
      return { time: { hour: applyPeriod(hour % 24, period), minute }, period };
    }
  }
  const periodOnly = take(scanner, new RegExp(`(${periodPattern})`));
  if (periodOnly) {
    const period = PERIOD_WORDS[periodOnly[1]];
    return { time: { hour: PERIOD_DEFAULT_HOUR[period], minute: 0 }, period };
  }
  return { time: null, period: null };
};

/** 识别循环规则；返回草稿（时间稍后补上）。 */
const parseRecurring = (scanner: Scanner): RecurringDraft | null => {
  const draft = createRecurringDraft();

  const interval = take(scanner, new RegExp(`每(?:隔)?\\s*(${NUM}|半)?\\s*(?:个)?\\s*(分钟|小时|钟头)`));
  if (interval) {
    const amount = interval[1] === "半" ? 0.5 : interval[1] ? parseNumber(interval[1]) : 1;
    if (amount && amount > 0) {
      draft.mode = "INTERVAL_RANGE";
      // 时间窗沿用表单默认值（08:00–17:30），可在编辑中调整。
      draft.intervalMinutes = Math.round(interval[2] === "分钟" ? amount : amount * 60);
      return draft;
    }
  }

  // “每月最后一个工作日”要在“每月最后一天”之前识别。
  if (take(scanner, /每个?月(?:的)?(?:最后一个?|末(?:的)?|底(?:的)?)工作日/)) {
    draft.mode = "MONTHLY_LAST_WORKDAY";
    return draft;
  }

  if (take(scanner, /每个?月(?:的)?(?:最后一[天日]|月?末|月?底)/)) {
    draft.mode = "MONTHLY_LAST_DAY";
    return draft;
  }

  if (take(scanner, /每个?工作日|(?:^|\s)工作日(?:每天)?/)) {
    draft.mode = "WORKDAY";
    return draft;
  }

  if (take(scanner, /每个?周末/)) {
    draft.mode = "WEEKLY";
    draft.scheduleWeekdays = WEEKDAY_MASK_WEEKEND;
    return draft;
  }

  const weekly = take(
    scanner,
    // 后续的数字必须带分隔符（“每周1、3”），避免把“每周五 17:30”里的 1 当成周一。
    new RegExp(
      `每个?${WEEK_WORD}(${WEEKDAY_CHAR}(?:(?:[、,，和及至到~\\-]|${WEEK_WORD})+${WEEKDAY_CHAR}|[一二三四五六日天七])*)`
    )
  );
  if (weekly) {
    let mask = 0;
    const body = weekly[1].replace(new RegExp(WEEK_WORD, "g"), "");
    const rangePattern = new RegExp(`(${WEEKDAY_CHAR})\\s*[至到~\\-]\\s*(${WEEKDAY_CHAR})`, "g");
    const withoutRanges = body.replace(rangePattern, (_, from: string, to: string) => {
      const start = weekdayFromChar(from);
      const end = weekdayFromChar(to);
      if (start && end) {
        for (let day = start; day <= end; day += 1) {
          mask |= weekdayBit(day);
        }
      }
      return " ";
    });
    for (const char of withoutRanges.match(new RegExp(WEEKDAY_CHAR, "g")) ?? []) {
      const day = weekdayFromChar(char);
      if (day) {
        mask |= weekdayBit(day);
      }
    }
    if (mask & WEEKDAY_MASK_ALL) {
      draft.mode = "WEEKLY";
      draft.scheduleWeekdays = mask & WEEKDAY_MASK_ALL;
      return draft;
    }
  }

  const monthly = take(scanner, new RegExp(`每个?月\\s*(${NUM})\\s*[日号]`));
  if (monthly) {
    const day = parseNumber(monthly[1]);
    if (day && day >= 1 && day <= 31) {
      draft.mode = "MONTHLY";
      draft.scheduleDay = day;
      return draft;
    }
  }

  if (take(scanner, /每(?:天|日)|天天/)) {
    draft.mode = "DAILY";
    return draft;
  }
  return null;
};

interface DayResult {
  date: Date;
  /** 由“今晚”“明早”等词带出的默认时段。 */
  period: Period | null;
}

const DAY_WORDS: [string, number, Period | null][] = [
  ["大后天", 3, null],
  ["后天", 2, null],
  ["明天", 1, null],
  ["明日", 1, null],
  ["明早", 1, "morning"],
  ["明晨", 1, "morning"],
  ["明晚", 1, "night"],
  ["明夜", 1, "night"],
  ["今天", 0, null],
  ["今日", 0, null],
  ["今早", 0, "morning"],
  ["今晚", 0, "night"],
  ["今夜", 0, "night"],
];

const parseDay = (scanner: Scanner, now: Date): DayResult | null => {
  const today = startOfDay(now);

  const full = take(scanner, /(\d{4})\s*[-/.年]\s*(\d{1,2})\s*[-/.月]\s*(\d{1,2})\s*[日号]?/);
  if (full) {
    const date = new Date(Number(full[1]), Number(full[2]) - 1, Number(full[3]));
    if (date.getMonth() === Number(full[2]) - 1) {
      return { date, period: null };
    }
  }

  const monthDay = take(scanner, new RegExp(`(下个?月|本月|这个?月)?\\s*(${NUM})\\s*月\\s*(${NUM})\\s*[日号]`));
  if (monthDay) {
    const month = parseNumber(monthDay[2]);
    const day = parseNumber(monthDay[3]);
    if (month && day && month <= 12 && day <= 31) {
      let date = new Date(today.getFullYear(), month - 1, day);
      // 今年的日期已过去则指明年。
      if (date < today) {
        date = new Date(today.getFullYear() + 1, month - 1, day);
      }
      if (date.getDate() === day) {
        return { date, period: null };
      }
    }
  }

  // “下个月5号”“本月20号”“15号”（单独的“N号”要求后面紧跟空白、时段或钟点，避免误识别“3号楼”）。
  const dayOfMonth = take(
    scanner,
    new RegExp(
      `(?:(下个?月|本月|这个?月)\\s*|(?<![\\d月]))(${NUM})\\s*[日号](?=\\s|$|[上下中早晚凌傍夜]|\\d|${NUM}\\s*[点时:：])`
    )
  );
  if (dayOfMonth) {
    const day = parseNumber(dayOfMonth[2]);
    if (day && day >= 1 && day <= 31) {
      const monthOffset = dayOfMonth[1]?.startsWith("下") ? 1 : 0;
      let date = new Date(today.getFullYear(), today.getMonth() + monthOffset, day);
      if (!dayOfMonth[1] && date < today) {
        date = new Date(today.getFullYear(), today.getMonth() + 1, day);
      }
      if (date.getDate() === day) {
        return { date, period: null };
      }
    }
  }

  const relative = take(scanner, new RegExp(`(${NUM})\\s*(?:个)?\\s*(天|周|星期|礼拜)(?:以)?后`));
  if (relative) {
    const amount = parseNumber(relative[1]);
    if (amount !== null) {
      return { date: addDays(today, relative[2] === "天" ? amount : amount * 7), period: null };
    }
  }

  const weekday = take(scanner, new RegExp(`(下下|下个?|这个?|本)?\\s*${WEEK_WORD}(${WEEKDAY_CHAR})`));
  if (weekday) {
    const target = weekdayFromChar(weekday[2]);
    if (target) {
      const current = isoWeekday(today);
      const prefix = weekday[1] ?? "";
      let offset: number;
      if (prefix.startsWith("下下")) {
        offset = 14 - current + target;
      } else if (prefix.startsWith("下")) {
        offset = 7 - current + target;
      } else if (prefix) {
        offset = target - current;
      } else {
        // 单说“周五”：本周还没到就是本周，否则下周。
        offset = (target - current + 7) % 7;
      }
      return { date: addDays(today, offset), period: null };
    }
  }

  for (const [word, offset, period] of DAY_WORDS) {
    if (take(scanner, new RegExp(word))) {
      return { date: addDays(today, offset), period };
    }
  }
  return null;
};

const parseRelativeTime = (scanner: Scanner, now: Date): Date | null => {
  const match = take(scanner, new RegExp(`(${NUM}|半)\\s*(?:个)?\\s*(半)?\\s*(分钟|小时|钟头)(?:以)?后`));
  if (!match) {
    return null;
  }
  const base = match[1] === "半" ? 0.5 : parseNumber(match[1]);
  if (base === null) {
    return null;
  }
  const amount = base + (match[2] ? 0.5 : 0);
  const minutes = match[3] === "分钟" ? amount : amount * 60;
  const target = new Date(now.getTime() + Math.round(minutes) * 60 * 1000);
  target.setSeconds(0, 0);
  return target;
};

const cleanTitle = (text: string) =>
  text
    .replace(/\s+/g, " ")
    .replace(/^[\s,，、。:：;；\-—]+|[\s,，、。:：;；\-—]+$/g, "")
    .replace(/^(?:提醒我|提醒|记得)\s*/, "")
    .trim();

export const parseQuickInput = (input: string, now: Date = new Date()): ParsedInput => {
  const scanner: Scanner = { text: input, matches: [] };
  const tags = parseTags(scanner);
  const priority = parsePriority(scanner);

  const recurring = parseRecurring(scanner);
  let reminderTime: Date | null = null;

  if (recurring) {
    const { time } = parseClock(scanner);
    if (time) {
      recurring.scheduleTime = `${pad(time.hour)}:${pad(time.minute)}`;
    }
  } else {
    reminderTime = parseRelativeTime(scanner, now);
    if (!reminderTime) {
      const day = parseDay(scanner, now);
      const { time, period } = parseClock(scanner);
      if (day || time) {
        const base = day ? day.date : startOfDay(now);
        const resolved =
          time ??
          (day?.period ? { hour: PERIOD_DEFAULT_HOUR[day.period], minute: 0 } : { hour: 9, minute: 0 });
        let hour = resolved.hour;
        // “今晚8点”“明早7点”：钟点没有自带时段时用日期词的时段换算。
        if (time && !period && day?.period) {
          hour = applyPeriod(time.hour > 12 ? time.hour - 12 : time.hour, day.period);
        }
        reminderTime = new Date(base.getFullYear(), base.getMonth(), base.getDate(), hour, resolved.minute);
        // 只说了钟点且今天已过：顺延到明天。
        if (!day && reminderTime.getTime() <= now.getTime()) {
          reminderTime = addDays(reminderTime, 1);
        }
      }
    }
  }

  const title = cleanTitle(scanner.text);
  if (recurring) {
    recurring.description = title;
  }
  return {
    title,
    reminderTime,
    recurring,
    tags,
    priority,
    matches: scanner.matches.sort((a, b) => a.index - b.index).map(item => item.value),
  };
};

const WEEKDAY_NAMES = ["日", "一", "二", "三", "四", "五", "六"];

/** 识别结果的简短说明，如“明天 15:00”“每周一、三 09:00”。 */
export const describeParsedSchedule = (parsed: ParsedInput, now: Date = new Date()): string => {
  if (parsed.recurring) {
    const draft = parsed.recurring;
    switch (draft.mode) {
      case "INTERVAL_RANGE":
        return `每 ${draft.intervalMinutes} 分钟（${draft.startTime} - ${draft.endTime}）`;
      case "DAILY":
        return `每天 ${draft.scheduleTime}`;
      case "WORKDAY":
        return `法定工作日 ${draft.scheduleTime}`;
      case "WEEKLY":
        return `${formatWeekdayMask(draft.scheduleWeekdays)} ${draft.scheduleTime}`;
      case "MONTHLY":
        return `每月 ${draft.scheduleDay} 日 ${draft.scheduleTime}`;
      case "MONTHLY_LAST_DAY":
        return `每月最后一天 ${draft.scheduleTime}`;
      case "MONTHLY_LAST_WORKDAY":
        return `每月最后一个工作日 ${draft.scheduleTime}`;
      default:
        return "";
    }
  }
  const target = parsed.reminderTime;
  if (!target) {
    return "";
  }
  const time = `${pad(target.getHours())}:${pad(target.getMinutes())}`;
  const diff = Math.round((startOfDay(target).getTime() - startOfDay(now).getTime()) / 86400000);
  if (diff === 0) {
    return `今天 ${time}`;
  }
  if (diff === 1) {
    return `明天 ${time}`;
  }
  if (diff === 2) {
    return `后天 ${time}`;
  }
  const sameYear = target.getFullYear() === now.getFullYear();
  const date = `${sameYear ? "" : `${target.getFullYear()}年`}${target.getMonth() + 1}月${target.getDate()}日`;
  return `${date} 周${WEEKDAY_NAMES[target.getDay()]} ${time}`;
};

/** 是否识别出了任何结构化信息（用于决定是否展示识别提示）。 */
export const hasParsedMeta = (parsed: ParsedInput) =>
  Boolean(parsed.reminderTime || parsed.recurring || parsed.tags.length || parsed.priority);

export interface RescheduleParse {
  time: Date | null;
  error: string | null;
}

/**
 * 解析“改到什么时候”的输入（提醒弹窗自定义稍后提醒、批量改提醒时间）：
 * 只取一次性时间，循环规则、没有时间或时间已过都给出原因。
 */
export const parseRescheduleTime = (input: string, now: Date = new Date()): RescheduleParse => {
  const text = input.trim();
  if (!text) return { time: null, error: null };
  const parsed = parseQuickInput(text, now);
  if (parsed.recurring) return { time: null, error: "请输入一个具体时间，而不是循环规则" };
  if (!parsed.reminderTime) return { time: null, error: "没有识别出时间，可以试试“明天下午3点”“30分钟后”" };
  if (parsed.reminderTime.getTime() <= now.getTime()) return { time: null, error: "这个时间已经过去了" };
  return { time: parsed.reminderTime, error: null };
};
