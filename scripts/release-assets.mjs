// 发布产物的文件名（write-updater-manifest.mjs 使用，测试在 release-assets.spec.mjs）。
//
// 2.2.4 起 productName 为“任务提醒”，Tauri 生成的安装包是 `任务提醒_{版本}_x64_zh-CN.msi`。
// GitHub Release 会改写含非 ASCII 字符的附件名，latest.json 中的下载地址就对不上了，
// 所以上传前统一复制为 ASCII 名称 `TaskReminderApp_{版本}_x64_zh-CN.msi`（签名只与文件内容有关，改名不影响）。

export const RELEASE_ASSET_PREFIX = "TaskReminderApp";

export const releaseAssetName = version => `${RELEASE_ASSET_PREFIX}_${version}_x64_zh-CN.msi`;

/** 在打包目录的文件名中找出本版本的 MSI（有签名文件的优先，已是发布名称的也算）。 */
export const findBuiltMsi = (fileNames, version) => {
  const suffix = `_${version}_x64_zh-CN.msi`;
  const candidates = fileNames.filter(name => name.endsWith(suffix));
  const signed = candidates.filter(name => fileNames.includes(`${name}.sig`));
  const pick = list => list.find(name => name !== releaseAssetName(version)) ?? list[0];
  return pick(signed) ?? pick(candidates) ?? null;
};
