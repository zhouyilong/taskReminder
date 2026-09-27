// 常见 WebDAV 服务预设：选择后填入地址模板，并提示需要准备的信息。

export type WebdavPresetId = "custom" | "jianguoyun" | "nextcloud" | "synology";

export interface WebdavPreset {
  id: WebdavPresetId;
  label: string;
  /** 地址模板；`{username}` 会替换为已填写的用户名。自定义时为空，不改动地址。 */
  urlTemplate: string;
  /** 远端路径为空时填入的默认值。 */
  defaultRootPath: string;
  hint: string;
}

/** 模板中需要用户替换的占位文字。 */
export const PLACEHOLDER_HOST = "你的服务器地址";
const PLACEHOLDER_USER = "用户名";

export const WEBDAV_PRESETS: WebdavPreset[] = [
  {
    id: "custom",
    label: "自定义",
    urlTemplate: "",
    defaultRootPath: "",
    hint: "填写 WebDAV 地址；远端路径需要先在网盘中创建。"
  },
  {
    id: "jianguoyun",
    label: "坚果云",
    urlTemplate: "https://dav.jianguoyun.com/dav/",
    defaultRootPath: "TaskReminder",
    hint: "用户名为坚果云账号（邮箱），密码需使用“第三方应用密码”（网页版 → 账户信息 → 安全选项 → 添加应用），不是登录密码。请先在坚果云中创建远端路径对应的文件夹。"
  },
  {
    id: "nextcloud",
    label: "Nextcloud",
    urlTemplate: `https://${PLACEHOLDER_HOST}/remote.php/dav/files/{username}/`,
    defaultRootPath: "TaskReminder",
    hint: "把地址中的“你的服务器地址”换成 Nextcloud 的域名；开启了两步验证时，密码需使用“设置 → 安全 → 应用密码”。请先在 Nextcloud 中创建远端路径对应的文件夹。"
  },
  {
    id: "synology",
    label: "群晖 NAS",
    urlTemplate: `https://${PLACEHOLDER_HOST}:5006/`,
    defaultRootPath: "/home/TaskReminder",
    hint: "需在套件中心安装并启用 WebDAV Server（HTTPS 默认端口 5006，HTTP 为 5005）。远端路径以共享文件夹开头，如 /home/TaskReminder，文件夹需先创建。"
  }
];

export const findWebdavPreset = (id: WebdavPresetId) =>
  WEBDAV_PRESETS.find(preset => preset.id === id) ?? WEBDAV_PRESETS[0];

/** 按地址识别预设，用于打开设置时回显。 */
export const detectWebdavPreset = (url: string): WebdavPresetId => {
  const value = url.trim().toLowerCase();
  if (value.includes("dav.jianguoyun.com")) {
    return "jianguoyun";
  }
  if (value.includes("/remote.php/dav") || value.includes("/remote.php/webdav")) {
    return "nextcloud";
  }
  if (/:500[56](\/|$)/.test(value)) {
    return "synology";
  }
  return "custom";
};

export interface WebdavFields {
  webdavUrl: string;
  webdavUsername: string;
  webdavRootPath: string;
}

/** 应用预设：替换地址；远端路径为空时填入默认值。自定义不改动任何字段。 */
export const applyWebdavPreset = (id: WebdavPresetId, fields: WebdavFields): WebdavFields => {
  const preset = findWebdavPreset(id);
  if (!preset.urlTemplate) {
    return { ...fields };
  }
  const username = fields.webdavUsername.trim();
  return {
    ...fields,
    webdavUrl: preset.urlTemplate.replace("{username}", encodeURIComponent(username) || PLACEHOLDER_USER),
    webdavRootPath: fields.webdavRootPath.trim() ? fields.webdavRootPath : preset.defaultRootPath
  };
};

/** 地址里还留着模板占位文字时返回提示。 */
export const webdavUrlPlaceholderWarning = (url: string): string | null => {
  if (url.includes(PLACEHOLDER_HOST)) {
    return "请把地址中的“你的服务器地址”替换为实际的域名或 IP";
  }
  if (url.includes(`/${PLACEHOLDER_USER}/`)) {
    return "请把地址中的“用户名”替换为实际的用户名";
  }
  return null;
};
