// 节假日数据（src-tauri/data/holidays-cn.json）的检查规则，供 check-holidays.mjs 与测试使用。
//
// 该文件同时是在线更新的数据源：推到 main 后，已发布的客户端会下载它。客户端只采用内置数据没有的年份，
// 且拒绝删减已下载年份的文件（整份拒收，之后的更新也收不到），因此这里要求：
// - 格式与客户端一致：年份 2000–2100；off / work 为日期或 [开始, 结束] 区间，日期落在所属年份内；
// - 每个年份都有放假日；同一天不能既放假又上班，也不能重复；
// - 与基准版本相比不能删除年份或删减日期；已有年份新增日期属于“更正”，需显式允许（只随版本发布生效）。

export const MIN_YEAR = 2000;
export const MAX_YEAR = 2100;

const DATE_RE = /^(\d{4})-(\d{2})-(\d{2})$/;

function parseDate(value) {
  if (typeof value !== "string") return null;
  const match = DATE_RE.exec(value);
  if (!match) return null;
  const [year, month, day] = match.slice(1).map(Number);
  const date = new Date(Date.UTC(year, month - 1, day));
  if (date.getUTCFullYear() !== year || date.getUTCMonth() !== month - 1 || date.getUTCDate() !== day) {
    return null;
  }
  return date;
}

const format = date => date.toISOString().slice(0, 10);

/** 展开 off / work 列表为日期字符串数组；格式错误写入 errors。 */
function expand(items, label, errors) {
  if (items === undefined) return [];
  if (!Array.isArray(items)) {
    errors.push(`${label} 应为数组`);
    return [];
  }
  const dates = [];
  for (const item of items) {
    if (Array.isArray(item)) {
      const [start, end] = item.map(parseDate);
      if (item.length !== 2 || !start || !end) {
        errors.push(`${label} 中的区间无效: ${JSON.stringify(item)}`);
        continue;
      }
      if (end < start) {
        errors.push(`${label} 中的区间倒置: ${item[0]} ~ ${item[1]}`);
        continue;
      }
      for (let day = start; day <= end; day = new Date(day.getTime() + 86400000)) {
        dates.push(format(day));
      }
    } else {
      const date = parseDate(item);
      if (!date) {
        errors.push(`${label} 中的日期无效: ${JSON.stringify(item)}`);
        continue;
      }
      dates.push(format(date));
    }
  }
  return dates;
}

/**
 * 解析并校验节假日 JSON。
 * @returns {{ years: Map<string, {off: Set<string>, work: Set<string>}>, errors: string[] }}
 */
export function parseHolidays(raw) {
  const errors = [];
  const years = new Map();
  let data;
  try {
    data = JSON.parse(raw);
  } catch (err) {
    return { years, errors: [`JSON 格式错误: ${err.message}`] };
  }
  if (!data || typeof data !== "object" || Array.isArray(data)) {
    return { years, errors: ["顶层应为对象"] };
  }
  for (const [key, value] of Object.entries(data)) {
    if (key.startsWith("_")) continue;
    const year = Number(key);
    if (!/^\d{4}$/.test(key) || year < MIN_YEAR || year > MAX_YEAR) {
      errors.push(`年份无效或超出范围（${MIN_YEAR}–${MAX_YEAR}）: ${key}`);
      continue;
    }
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      errors.push(`${key} 应为包含 off / work 的对象`);
      continue;
    }
    const off = expand(value.off, `${key}.off`, errors);
    const work = expand(value.work, `${key}.work`, errors);
    for (const [label, dates] of [["off", off], ["work", work]]) {
      const seen = new Set();
      for (const date of dates) {
        if (!date.startsWith(`${key}-`)) errors.push(`${key}.${label} 中的日期不属于 ${key} 年: ${date}`);
        if (seen.has(date)) errors.push(`${key}.${label} 中的日期重复: ${date}`);
        seen.add(date);
      }
    }
    const offSet = new Set(off);
    const workSet = new Set(work);
    if (offSet.size === 0) errors.push(`${key} 年没有放假日`);
    for (const date of workSet) {
      if (offSet.has(date)) errors.push(`${date} 同时出现在 ${key}.off 与 ${key}.work 中`);
    }
    years.set(key, { off: offSet, work: workSet });
  }
  if (years.size === 0 && errors.length === 0) errors.push("没有任何年份的数据");
  return { years, errors };
}

/**
 * 与基准版本比较。
 * @returns {{ errors: string[], corrections: string[], added: string[] }}
 *   corrections 为已有年份新增的日期（更正），是否允许由调用方决定。
 */
export function compareHolidays(base, current) {
  const errors = [];
  const corrections = [];
  for (const [year, before] of base) {
    const after = current.get(year);
    if (!after) {
      errors.push(`删除了已发布的年份 ${year}：客户端会拒收整份文件`);
      continue;
    }
    for (const kind of ["off", "work"]) {
      for (const date of before[kind]) {
        if (!after[kind].has(date)) errors.push(`${year}.${kind} 删除了 ${date}：客户端会拒收整份文件`);
      }
      for (const date of after[kind]) {
        if (!before[kind].has(date)) corrections.push(`${year}.${kind} 新增 ${date}`);
      }
    }
  }
  const added = [...current.keys()].filter(year => !base.has(year)).sort();
  return { errors, corrections, added };
}

/** 完整检查：格式 + 与基准比较。base 为 null 时只检查格式。 */
export function checkHolidays(currentRaw, baseRaw, { allowCorrection = false } = {}) {
  const current = parseHolidays(currentRaw);
  const errors = [...current.errors];
  const notes = [];
  if (baseRaw !== null && baseRaw !== undefined) {
    const base = parseHolidays(baseRaw);
    if (base.errors.length > 0) {
      notes.push(`基准版本本身不合格，跳过比较：${base.errors.join("；")}`);
    } else {
      const diff = compareHolidays(base.years, current.years);
      errors.push(...diff.errors);
      if (diff.corrections.length > 0) {
        const message = `修改了已有年份（${diff.corrections.join("，")}）。已发布的客户端以内置数据为准，更正只随应用版本发布生效`;
        if (allowCorrection) notes.push(message);
        else errors.push(`${message}；确认是更正时在提交信息或 PR 标题中加入 holidays-correction`);
      }
      if (diff.added.length > 0) notes.push(`新增年份：${diff.added.join("、")}`);
    }
  }
  return { errors, notes };
}
