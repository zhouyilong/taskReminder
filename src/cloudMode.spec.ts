import { describe, expect, it } from "vitest";
import {
  buildModeBadge,
  formatBusinessCountsSummary,
  generateStrongPassword,
  isInsecureRemoteUrl,
  totalBusinessCount
} from "./cloudMode";
import type { CloudStatus } from "./types";

const baseStatus = (patch: Partial<CloudStatus> = {}): CloudStatus => ({
  mode: "local",
  state: "local",
  loggedIn: false,
  userId: "",
  username: "",
  nickname: "",
  role: "",
  mustChangePassword: false,
  serverUrl: "http://104.168.101.151:8080",
  defaultServerUrl: "http://104.168.101.151:8080",
  pendingCount: 0,
  lastSyncTime: null,
  error: null,
  ...patch
});

describe("isInsecureRemoteUrl", () => {
  it("flags public http urls as insecure", () => {
    expect(isInsecureRemoteUrl("http://tasks.example.com")).toBe(true);
    expect(isInsecureRemoteUrl("http://8.8.8.8:8080")).toBe(true);
  });

  it("allows https, localhost and private LAN http urls", () => {
    expect(isInsecureRemoteUrl("")).toBe(false);
    expect(isInsecureRemoteUrl("https://tasks.example.com")).toBe(false);
    expect(isInsecureRemoteUrl("http://localhost:8080")).toBe(false);
    expect(isInsecureRemoteUrl("http://127.0.0.1:8080")).toBe(false);
    expect(isInsecureRemoteUrl("http://192.168.1.10:8080")).toBe(false);
    expect(isInsecureRemoteUrl("http://10.0.0.5:8080")).toBe(false);
    expect(isInsecureRemoteUrl("http://172.16.0.9:8080")).toBe(false);
    expect(isInsecureRemoteUrl("http://nas.local:8080")).toBe(false);
  });
});

describe("buildModeBadge", () => {
  it("renders local badge in local mode", () => {
    const badge = buildModeBadge(baseStatus());
    expect(badge.label).toBe("本地");
    expect(badge.tone).toBe("local");
  });

  it("renders connected badge with user display name in cloud mode", () => {
    const badge = buildModeBadge(
      baseStatus({
        mode: "cloud",
        state: "connected",
        loggedIn: true,
        username: "alice",
        nickname: "爱丽丝"
      })
    );
    expect(badge.label).toBe("云端 · 爱丽丝");
    expect(badge.tone).toBe("connected");
  });

  it("renders offline warning badge with pending count", () => {
    const badge = buildModeBadge(
      baseStatus({
        mode: "cloud",
        state: "offline",
        loggedIn: true,
        username: "alice",
        pendingCount: 3
      })
    );
    expect(badge.label).toBe("云端 · 已离线（3 条待同步）");
    expect(badge.tone).toBe("offline");
    expect(badge.actionable).toBe(true);
  });
});

describe("business counts and password generator", () => {
  it("summarizes business counts cleanly", () => {
    const counts = {
      activeTasks: 2,
      completedTasks: 1,
      recurringTasks: 3,
      reminderRecords: 0
    };
    expect(totalBusinessCount(counts)).toBe(6);
    expect(formatBusinessCountsSummary(counts)).toBe("2 条未完成待办、1 条已办、3 条循环提醒");
    expect(formatBusinessCountsSummary(null)).toBe("0 条数据");
  });

  it("generates password of requested length", () => {
    expect(generateStrongPassword(16)).toHaveLength(16);
  });
});
