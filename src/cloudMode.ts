// 运行模式与云端服务地址的纯函数工具（测试在 cloudMode.spec.ts）。
import type { BusinessDataCounts, CloudStatus } from "./types";

export interface ModeBadgeView {
  label: string;
  tone: "local" | "connected" | "syncing" | "offline";
  actionable: boolean;
  title: string;
}

/** 判断一个 IPv4 地址是否属于本机或局域网私有网段。 */
function isPrivateIpv4(host: string): boolean {
  const parts = host.split(".").map(Number);
  if (parts.length !== 4 || parts.some(n => !Number.isInteger(n) || n < 0 || n > 255)) {
    return false;
  }
  const [a, b] = parts;
  if (a === 127 || a === 10) return true;
  if (a === 192 && b === 168) return true;
  if (a === 172 && b >= 16 && b <= 31) return true;
  return false;
}

/**
 * 填写非 `https://` 且非本机/局域网地址时返回 true，用于在界面给出明文传输提示。
 */
export function isInsecureRemoteUrl(rawUrl: string): boolean {
  const trimmed = rawUrl.trim();
  if (!trimmed) return false;
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return false;
  }
  if (parsed.protocol !== "http:") {
    return false;
  }
  const host = parsed.hostname.replace(/^\[|\]$/g, "").toLowerCase();
  if (host === "localhost" || host === "::1" || host.endsWith(".local")) {
    return false;
  }
  if (isPrivateIpv4(host)) {
    return false;
  }
  return true;
}

export function totalBusinessCount(counts?: BusinessDataCounts | null): number {
  if (!counts) return 0;
  return (
    (counts.activeTasks || 0) +
    (counts.completedTasks || 0) +
    (counts.recurringTasks || 0) +
    (counts.reminderRecords || 0)
  );
}

export function formatBusinessCountsSummary(counts?: BusinessDataCounts | null): string {
  if (!counts) return "0 条数据";
  const parts: string[] = [];
  if (counts.activeTasks > 0) parts.push(`${counts.activeTasks} 条未完成待办`);
  if (counts.completedTasks > 0) parts.push(`${counts.completedTasks} 条已办`);
  if (counts.recurringTasks > 0) parts.push(`${counts.recurringTasks} 条循环提醒`);
  if (counts.reminderRecords > 0) parts.push(`${counts.reminderRecords} 条提醒记录`);
  return parts.length ? parts.join("、") : "暂无业务数据";
}

/** 标题栏运行模式与云端连接状态徽标。 */
export function buildModeBadge(status: CloudStatus | null): ModeBadgeView {
  if (!status || status.mode === "local") {
    return {
      label: "本地",
      tone: "local",
      actionable: true,
      title: "当前为本地模式，点击打开设置可切换至云端模式"
    };
  }
  const displayName = (status.nickname || status.username || "未登录").trim();
  if (!status.loggedIn || status.state === "auth_required") {
    return {
      label: "云端 · 请登录",
      tone: "offline",
      actionable: true,
      title: status.error || "云端登录态已失效，点击重新登录或切换至本地模式"
    };
  }
  if (status.state === "offline") {
    const suffix = status.pendingCount > 0 ? `（${status.pendingCount} 条待同步）` : "";
    return {
      label: `云端 · 已离线${suffix}`,
      tone: "offline",
      actionable: true,
      title: `${status.error ? `${status.error}\n` : ""}点击可一键切换至本地模式`
    };
  }
  if (status.state === "syncing") {
    return {
      label: `云端 · ${displayName}`,
      tone: "syncing",
      actionable: false,
      title: `正在与云端同步（${status.serverUrl}）`
    };
  }
  return {
    label: `云端 · ${displayName}`,
    tone: "connected",
    actionable: false,
    title: `云端已连接：${displayName}（${status.serverUrl}）`
  };
}

/** 生成 16 位随机强密码（不含易混淆字符），供管理员建号/重置密码使用。 */
export function generateStrongPassword(length = 16): string {
  const alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*";
  const result: string[] = [];
  if (typeof crypto !== "undefined" && crypto.getRandomValues) {
    const buf = new Uint32Array(length);
    crypto.getRandomValues(buf);
    for (let i = 0; i < length; i += 1) {
      result.push(alphabet[buf[i] % alphabet.length]);
    }
  } else {
    for (let i = 0; i < length; i += 1) {
      result.push(alphabet[Math.floor(Math.random() * alphabet.length)]);
    }
  }
  return result.join("");
}
