import { describe, expect, it } from "vitest";
import { applyWebdavPreset, detectWebdavPreset, webdavUrlPlaceholderWarning } from "./webdavPresets";

const empty = { webdavUrl: "", webdavUsername: "", webdavRootPath: "" };

describe("applyWebdavPreset", () => {
  it("坚果云填入固定地址与默认路径", () => {
    expect(applyWebdavPreset("jianguoyun", empty)).toEqual({
      webdavUrl: "https://dav.jianguoyun.com/dav/",
      webdavUsername: "",
      webdavRootPath: "TaskReminder"
    });
  });

  it("已填写的远端路径保持不变", () => {
    const result = applyWebdavPreset("jianguoyun", { ...empty, webdavRootPath: "backup/todo" });
    expect(result.webdavRootPath).toBe("backup/todo");
  });

  it("Nextcloud 地址带入已填写的用户名", () => {
    const result = applyWebdavPreset("nextcloud", { ...empty, webdavUsername: "alice@example.com" });
    expect(result.webdavUrl).toBe("https://你的服务器地址/remote.php/dav/files/alice%40example.com/");
    expect(applyWebdavPreset("nextcloud", empty).webdavUrl).toContain("/files/用户名/");
  });

  it("自定义不改动字段", () => {
    const fields = { webdavUrl: "https://dav.example.com/", webdavUsername: "u", webdavRootPath: "" };
    expect(applyWebdavPreset("custom", fields)).toEqual(fields);
  });
});

describe("detectWebdavPreset", () => {
  it("按地址识别服务", () => {
    expect(detectWebdavPreset("https://dav.jianguoyun.com/dav/")).toBe("jianguoyun");
    expect(detectWebdavPreset("https://cloud.example.com/remote.php/dav/files/alice/")).toBe("nextcloud");
    expect(detectWebdavPreset("https://nas.local:5006/")).toBe("synology");
    expect(detectWebdavPreset("http://192.168.1.2:5005")).toBe("synology");
    expect(detectWebdavPreset("https://dav.example.com/")).toBe("custom");
    expect(detectWebdavPreset("")).toBe("custom");
  });
});

describe("webdavUrlPlaceholderWarning", () => {
  it("地址中还有占位文字时提示", () => {
    expect(webdavUrlPlaceholderWarning("https://你的服务器地址:5006/")).toContain("服务器地址");
    expect(webdavUrlPlaceholderWarning("https://cloud.example.com/remote.php/dav/files/用户名/")).toContain("用户名");
    expect(webdavUrlPlaceholderWarning("https://dav.jianguoyun.com/dav/")).toBeNull();
  });
});
