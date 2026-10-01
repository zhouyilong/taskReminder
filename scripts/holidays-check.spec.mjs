import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { checkHolidays, parseHolidays } from "./holidays-check.mjs";

const embedded = readFileSync(new URL("../src-tauri/data/holidays-cn.json", import.meta.url), "utf8");
const json = value => JSON.stringify(value);
const withYear = (year, value) => json({ ...JSON.parse(embedded), [year]: value });
const year2027 = { off: ["2027-01-01", ["2027-02-06", "2027-02-12"]], work: ["2027-02-14"] };

describe("parseHolidays", () => {
  it("accepts the embedded data and expands ranges", () => {
    const { years, errors } = parseHolidays(embedded);
    expect(errors).toEqual([]);
    expect(years.get("2026").off.has("2026-02-20")).toBe(true);
    expect(years.get("2026").work.has("2026-02-14")).toBe(true);
  });

  it("rejects malformed data", () => {
    expect(parseHolidays("{").errors[0]).toMatch("JSON");
    expect(parseHolidays("[]").errors).toEqual(["顶层应为对象"]);
    expect(parseHolidays(json({ _comment: "" })).errors).toEqual(["没有任何年份的数据"]);
    expect(parseHolidays(json({ 1999: year2027 })).errors[0]).toMatch("年份无效");
    expect(parseHolidays(json({ 2027: { off: [], work: [] } })).errors).toEqual(["2027 年没有放假日"]);
    expect(parseHolidays(json({ 2027: { off: ["2027-02-30"] } })).errors[0]).toMatch("日期无效");
    expect(parseHolidays(json({ 2027: { off: [["2027-02-12", "2027-02-06"]] } })).errors[0]).toMatch("倒置");
    expect(parseHolidays(json({ 2027: { off: ["2028-01-01"] } })).errors[0]).toMatch("不属于 2027 年");
    expect(parseHolidays(json({ 2027: { off: ["2027-01-01", ["2026-12-31", "2027-01-02"]] } })).errors).toEqual(
      expect.arrayContaining([expect.stringMatching("重复"), expect.stringMatching("不属于")])
    );
    expect(parseHolidays(json({ 2027: { off: ["2027-01-01"], work: ["2027-01-01"] } })).errors[0]).toMatch("同时出现");
  });
});

describe("checkHolidays", () => {
  it("allows appending a new year", () => {
    const result = checkHolidays(withYear(2027, year2027), embedded);
    expect(result.errors).toEqual([]);
    expect(result.notes).toEqual(["新增年份：2027"]);
  });

  it("only checks the format without a base", () => {
    expect(checkHolidays(embedded, null).errors).toEqual([]);
  });

  it("rejects removing a year or dates", () => {
    const data = JSON.parse(embedded);
    delete data["2025"];
    expect(checkHolidays(json(data), embedded).errors[0]).toMatch("删除了已发布的年份 2025");

    const shrunk = JSON.parse(embedded);
    shrunk["2026"].work = shrunk["2026"].work.slice(1);
    expect(checkHolidays(json(shrunk), embedded).errors[0]).toMatch("2026.work 删除了 2026-01-04");
  });

  it("requires an explicit flag for corrections to existing years", () => {
    const corrected = JSON.parse(embedded);
    corrected["2026"].work.push("2026-01-31");
    expect(checkHolidays(json(corrected), embedded).errors[0]).toMatch("holidays-correction");
    const allowed = checkHolidays(json(corrected), embedded, { allowCorrection: true });
    expect(allowed.errors).toEqual([]);
    expect(allowed.notes[0]).toMatch("2026.work 新增 2026-01-31");
  });
});
