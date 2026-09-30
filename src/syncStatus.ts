// 云同步状态码与界面文案。后端存库与广播的都是状态码（见 `sync.rs` 的 `SyncState`），
// 2.0.1 之前的版本直接存中文文案，这里一并兼容。

export type SyncStateCode =
  | "never"
  | "syncing"
  | "success"
  | "first_sync"
  | "lock_busy"
  | "failed"
  | "passphrase_mismatch";
export type SyncTone = "idle" | "syncing" | "success" | "error";

const LABELS: Record<SyncStateCode, string> = {
  never: "未同步",
  syncing: "同步中",
  success: "同步成功",
  first_sync: "首次同步完成",
  lock_busy: "锁被占用，稍后重试",
  failed: "同步失败",
  // 云端数据无法用本机的同步密码解密，通常是在其他设备上更换了密码；自动同步暂停。
  passphrase_mismatch: "同步密码不匹配"
};

// 旧版只存过前六种文案。
const LEGACY: Record<string, SyncStateCode> = Object.fromEntries(
  Object.entries(LABELS)
    .filter(([code]) => code !== "passphrase_mismatch")
    .map(([code, label]) => [label, code as SyncStateCode])
);

/** 解析状态码或旧版中文文案；空值视为未同步，无法识别时返回 null。 */
export const parseSyncState = (value?: string | null): SyncStateCode | null => {
  const raw = (value ?? "").trim();
  if (!raw) {
    return "never";
  }
  if (raw in LABELS) {
    return raw as SyncStateCode;
  }
  return LEGACY[raw] ?? null;
};

/** 状态文案；无法识别的值原样显示。 */
export const syncStateLabel = (value?: string | null) => {
  const state = parseSyncState(value);
  return state ? LABELS[state] : (value ?? "").trim();
};

export const syncStateTone = (value?: string | null, error?: string | null): SyncTone => {
  const state = parseSyncState(value);
  if (state === "failed" || state === "passphrase_mismatch" || error) {
    return "error";
  }
  if (state === "syncing") {
    return "syncing";
  }
  if (state === "success" || state === "first_sync") {
    return "success";
  }
  return "idle";
};
