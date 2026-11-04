import { test as base, expect, type Page } from "@playwright/test";
import { fileURLToPath } from "node:url";

const MOCK_PATH = fileURLToPath(new URL("./tauri-mock.js", import.meta.url));

export interface InvokeCall {
  cmd: string;
  args: Record<string, unknown>;
}

/** 每个页面在脚本执行前注入 Tauri 运行时模拟。`todoSticky` 打开待办清单便签窗口（v2.2.4）。 */
export const test = base.extend<{ app: Page; todoSticky: Page }>({
  todoSticky: async ({ page }, use) => {
    await page.addInitScript({ path: MOCK_PATH });
    await page.goto("/sticky-todo-list.html");
    await expect(page.locator(".todo-sticky")).toBeVisible();
    await use(page);
  },
  app: async ({ page }, use) => {
    await page.addInitScript({ path: MOCK_PATH });
    await page.addInitScript(() => {
      try {
        localStorage.setItem("activeTab", "tasks");
      } catch {
        // 忽略
      }
    });
    await page.goto("/");
    await expect(page.locator(".app-shell, #app > *").first()).toBeVisible();
    await use(page);
  }
});

/** 打开单张便签窗口（`sticky-note-item.html?noteId=...`，v2.2.6），等编辑器就绪。 */
export const openStickyNote = async (page: Page, noteId: string) => {
  await page.addInitScript({ path: MOCK_PATH });
  await page.goto(`/sticky-note-item.html?noteId=${encodeURIComponent(noteId)}`);
  await expect(page.locator(".paper-note-window .ProseMirror")).toBeVisible();
};

/** 读取模拟运行时记录的 invoke 调用（可按命令名过滤）。 */
export const invokeCalls = async (page: Page, cmd?: string): Promise<InvokeCall[]> => {
  const calls = await page.evaluate(() => (window as unknown as { __TAURI_MOCK__: { calls: InvokeCall[] } }).__TAURI_MOCK__.calls);
  return cmd ? calls.filter(call => call.cmd === cmd) : calls;
};

export { expect };
