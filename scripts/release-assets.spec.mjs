import { describe, expect, it } from "vitest";
import { findBuiltMsi, releaseAssetName } from "./release-assets.mjs";

describe("发布产物文件名", () => {
  it("发布名称保持 ASCII", () => {
    expect(releaseAssetName("2.2.4")).toBe("TaskReminderApp_2.2.4_x64_zh-CN.msi");
  });

  it("找出本版本带签名的 MSI", () => {
    const files = ["任务提醒_2.2.4_x64_zh-CN.msi", "任务提醒_2.2.4_x64_zh-CN.msi.sig", "TaskReminderApp_2.2.3_x64_zh-CN.msi"];
    expect(findBuiltMsi(files, "2.2.4")).toBe("任务提醒_2.2.4_x64_zh-CN.msi");
    expect(findBuiltMsi(files, "2.2.5")).toBeNull();
  });

  it("已复制过发布名称时仍取 Tauri 生成的原文件；只有发布名称时取它", () => {
    const files = [
      "TaskReminderApp_2.2.4_x64_zh-CN.msi",
      "TaskReminderApp_2.2.4_x64_zh-CN.msi.sig",
      "任务提醒_2.2.4_x64_zh-CN.msi",
      "任务提醒_2.2.4_x64_zh-CN.msi.sig"
    ];
    expect(findBuiltMsi(files, "2.2.4")).toBe("任务提醒_2.2.4_x64_zh-CN.msi");
    expect(findBuiltMsi(files.slice(0, 2), "2.2.4")).toBe("TaskReminderApp_2.2.4_x64_zh-CN.msi");
  });
});
