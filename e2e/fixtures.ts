import { test as base, expect, type Page } from "@playwright/test";
import { fileURLToPath } from "node:url";

const MOCK_PATH = fileURLToPath(new URL("./tauri-mock.js", import.meta.url));

export interface InvokeCall {
  cmd: string;
  args: Record<string, unknown>;
}

/** 每个页面在脚本执行前注入 Tauri 运行时模拟。 */
export const test = base.extend<{ app: Page }>({
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

/** 读取模拟运行时记录的 invoke 调用（可按命令名过滤）。 */
export const invokeCalls = async (page: Page, cmd?: string): Promise<InvokeCall[]> => {
  const calls = await page.evaluate(() => (window as unknown as { __TAURI_MOCK__: { calls: InvokeCall[] } }).__TAURI_MOCK__.calls);
  return cmd ? calls.filter(call => call.cmd === cmd) : calls;
};

export { expect };
