import { describe, expect, it } from "vitest";
import { parseSyncState, syncStateLabel, syncStateTone } from "./syncStatus";

describe("syncStatus (v3.0.0 cloud states)", () => {
  it("maps known cloud states to labels and tones", () => {
    expect(parseSyncState("")).toBe("local");
    expect(syncStateLabel("local")).toBe("本地模式");
    expect(syncStateLabel("connected")).toBe("已连接");
    expect(syncStateLabel("syncing")).toBe("同步中");
    expect(syncStateLabel("offline")).toBe("已离线");
    expect(syncStateLabel("auth_required")).toBe("请重新登录");

    expect(syncStateTone("local")).toBe("idle");
    expect(syncStateTone("connected")).toBe("success");
    expect(syncStateTone("syncing")).toBe("syncing");
    expect(syncStateTone("offline")).toBe("error");
    expect(syncStateTone("auth_required")).toBe("error");
    expect(syncStateTone("connected", "连接超时")).toBe("error");
  });

  it("keeps unknown status strings verbatim", () => {
    expect(parseSyncState("custom_state")).toBeNull();
    expect(syncStateLabel("custom_state")).toBe("custom_state");
  });
});
