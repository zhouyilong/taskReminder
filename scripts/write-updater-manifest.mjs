import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const projectRoot = process.cwd();
const packageJsonPath = resolve(projectRoot, "package.json");
const bundleDir = resolve(projectRoot, "src-tauri/target/release/bundle/msi");
const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
const version = process.env.RELEASE_VERSION?.trim() || packageJson.version;
const assetName = `TaskReminderApp_${version}_x64_zh-CN.msi`;
const signaturePath = resolve(bundleDir, `${assetName}.sig`);
const manifestPath = resolve(bundleDir, "latest.json");

if (!existsSync(signaturePath)) {
  throw new Error(`Missing updater signature: ${signaturePath}`);
}

const signature = readFileSync(signaturePath, "utf8").trim();
const notes = process.env.RELEASE_NOTES?.trim() || `TaskReminderApp ${version}`;
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
