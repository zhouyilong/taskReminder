import { describe, expect, it } from "vitest";
import { parseSyncState, syncStateLabel, syncStateTone } from "./syncStatus";

describe("parseSyncState", () => {
  it("识别状态码", () => {
    expect(parseSyncState("success")).toBe("success");
    expect(parseSyncState("first_sync")).toBe("first_sync");
    expect(parseSyncState("lock_busy")).toBe("lock_busy");
  });

  it("兼容旧版中文文案", () => {
    expect(parseSyncState("同步成功")).toBe("success");
    expect(parseSyncState("首次同步完成")).toBe("first_sync");
    expect(parseSyncState("同步失败")).toBe("failed");
    expect(parseSyncState("同步中")).toBe("syncing");
  });

  it("空值视为未同步，未知值返回 null", () => {
    expect(parseSyncState(null)).toBe("never");
    expect(parseSyncState("")).toBe("never");
    expect(parseSyncState("something")).toBeNull();
  });
});

describe("syncStateLabel", () => {
  it("状态码映射为文案，未知值原样显示", () => {
    expect(syncStateLabel("success")).toBe("同步成功");
    expect(syncStateLabel("never")).toBe("未同步");
    expect(syncStateLabel(undefined)).toBe("未同步");
    expect(syncStateLabel("同步成功")).toBe("同步成功");
    expect(syncStateLabel("something")).toBe("something");
  });
});

describe("syncStateTone", () => {
  it("按状态码判断色调", () => {
    expect(syncStateTone("success")).toBe("success");
    expect(syncStateTone("first_sync")).toBe("success");
    expect(syncStateTone("syncing")).toBe("syncing");
    expect(syncStateTone("failed")).toBe("error");
    expect(syncStateTone("lock_busy")).toBe("idle");
    expect(syncStateTone("never")).toBe("idle");
  });

  it("带错误信息时为 error", () => {
    expect(syncStateTone("success", "网络错误")).toBe("error");
  });

  it("兼容旧版中文文案", () => {
    expect(syncStateTone("同步成功")).toBe("success");
    expect(syncStateTone("同步失败")).toBe("error");
  });
});
