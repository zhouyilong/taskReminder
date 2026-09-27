// 发布前检查：三处版本号一致，且与发布的 tag（v{version}）相符。
// 用法：node scripts/check-release-version.mjs [v2.0.1]；不传参数时读取 GITHUB_REF_NAME。
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const read = path => readFileSync(resolve(root, path), "utf8");

const versions = {
  "package.json": JSON.parse(read("package.json")).version,
  "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version,
  "src-tauri/Cargo.toml": /^version\s*=\s*"([^"]+)"/m.exec(read("src-tauri/Cargo.toml"))?.[1]
};

const errors = [];
const distinct = new Set(Object.values(versions));
if (distinct.size !== 1 || distinct.has(undefined)) {
  errors.push(
    `三处版本号不一致：${Object.entries(versions)
      .map(([file, version]) => `${file}=${version ?? "未找到"}`)
      .join("，")}`
  );
}

const version = versions["package.json"];
const tag = (process.argv[2] || process.env.GITHUB_REF_NAME || "").trim();
if (tag && tag !== `v${version}`) {
  errors.push(`tag ${tag} 与版本号 ${version} 不符，应为 v${version}`);
}

if (errors.length) {
  for (const error of errors) {
    console.error(`[release] ${error}`);
  }
  process.exit(1);
}
console.log(`[release] 版本号一致：${version}${tag ? `，tag ${tag}` : ""}`);
