// 检查节假日数据：格式合法，且与基准版本相比只追加年份（规则见 holidays-check.mjs）。
// 用法：node scripts/check-holidays.mjs [--base <git 引用>] [--allow-correction]
// 也可用环境变量 HOLIDAYS_BASE、HOLIDAYS_CORRECTION=true 指定；基准引用不存在时只检查格式。
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { checkHolidays } from "./holidays-check.mjs";

const FILE = "src-tauri/data/holidays-cn.json";
const args = process.argv.slice(2);
const option = name => {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
};

const baseRef = (option("--base") ?? process.env.HOLIDAYS_BASE ?? "").trim();
const allowCorrection = args.includes("--allow-correction") || process.env.HOLIDAYS_CORRECTION === "true";

let baseRaw = null;
if (baseRef && !/^0+$/.test(baseRef)) {
  try {
    baseRaw = execFileSync("git", ["show", `${baseRef}:${FILE}`], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"]
    });
  } catch {
    console.log(`基准 ${baseRef} 中没有 ${FILE}，只检查格式。`);
  }
}

const { errors, notes } = checkHolidays(readFileSync(resolve(process.cwd(), FILE), "utf8"), baseRaw, {
  allowCorrection
});
for (const note of notes) console.log(note);
if (errors.length > 0) {
  console.error(`${FILE} 检查未通过：`);
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}
console.log(`${FILE} 检查通过${baseRaw === null ? "（未与基准比较）" : `（基准 ${baseRef}）`}。`);
