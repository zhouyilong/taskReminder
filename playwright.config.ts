import { defineConfig, devices } from "@playwright/test";

// 端到端测试：在浏览器中运行主窗口，Tauri 运行时由 e2e/tauri-mock.js 模拟（见 e2e/fixtures.ts）。
const PORT = 5199;

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    locale: "zh-CN",
    timezoneId: "Asia/Shanghai",
    viewport: { width: 1200, height: 800 },
    trace: "retain-on-failure"
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1200, height: 800 } } }],
  webServer: {
    command: `pnpm exec vite --port ${PORT} --strictPort --host 127.0.0.1`,
    url: `http://127.0.0.1:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000
  }
});
