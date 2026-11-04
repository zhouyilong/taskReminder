import { copyFileSync, existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { findBuiltMsi, releaseAssetName } from "./release-assets.mjs";

const projectRoot = process.cwd();
const packageJsonPath = resolve(projectRoot, "package.json");
const bundleDir = resolve(projectRoot, "src-tauri/target/release/bundle/msi");
const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
const version = process.env.RELEASE_VERSION?.trim() || packageJson.version;
const assetName = releaseAssetName(version);
const signaturePath = resolve(bundleDir, `${assetName}.sig`);

// 安装包名随 productName（“任务提醒”）变化，复制为 ASCII 的发布名称再上传（见 release-assets.mjs）。
const builtMsi = existsSync(bundleDir) ? findBuiltMsi(readdirSync(bundleDir), version) : null;
if (builtMsi && builtMsi !== assetName) {
  copyFileSync(resolve(bundleDir, builtMsi), resolve(bundleDir, assetName));
  if (existsSync(resolve(bundleDir, `${builtMsi}.sig`))) {
    copyFileSync(resolve(bundleDir, `${builtMsi}.sig`), signaturePath);
  }
  console.log(`[updater] ${builtMsi} -> ${assetName}`);
}
const manifestPath = resolve(bundleDir, "latest.json");

if (!existsSync(signaturePath)) {
  throw new Error(`Missing updater signature: ${signaturePath}`);
}

const signature = readFileSync(signaturePath, "utf8").trim();
const notes = process.env.RELEASE_NOTES?.trim() || `任务提醒 ${version}`;
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature,
      url: `https://github.com/zhouyilong/taskReminder/releases/download/v${version}/${assetName}`
    }
  }
};

writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
console.log(`[updater] latest.json written to ${manifestPath}`);
