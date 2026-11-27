// 云端连接/同步状态码与界面文案（v3.0.0）。

export type SyncStateCode = "local" | "connected" | "syncing" | "offline" | "auth_required";
export type SyncTone = "idle" | "syncing" | "success" | "error";

const LABELS: Record<SyncStateCode, string> = {
  local: "本地模式",
  connected: "已连接",
  syncing: "同步中",
  offline: "已离线",
  auth_required: "请重新登录"
};

export const parseSyncState = (value?: string | null): SyncStateCode | null => {
  const raw = (value ?? "").trim();
  if (!raw) return "local";
  if (raw in LABELS) return raw as SyncStateCode;
  return null;
};

export const syncStateLabel = (value?: string | null) => {
  const state = parseSyncState(value);
  return state ? LABELS[state] : (value ?? "").trim();
};

export const syncStateTone = (value?: string | null, error?: string | null): SyncTone => {
  const state = parseSyncState(value);
  if (state === "offline" || state === "auth_required" || error) {
    return "error";
  }
  if (state === "syncing") {
    return "syncing";
  }
  if (state === "connected") {
    return "success";
  }
  return "idle";
};
