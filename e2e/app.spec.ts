import { expect, invokeCalls, test } from "./fixtures";

// 主窗口的关键流程：只断言界面与 invoke 参数（Tauri 运行时由 tauri-mock.js 模拟）。

const pad = (value: number) => String(value).padStart(2, "0");
const dateKey = (date: Date) => `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
const tomorrow = () => {
  const date = new Date();
  date.setDate(date.getDate() + 1);
  return dateKey(date);
};

const taskRow = (app: import("@playwright/test").Page, title: string) =>
  app.locator(".table-row", { has: app.locator(".task-title-text", { hasText: title }) });

test("自然语言新建待办", async ({ app }) => {
  const input = app.locator(".composer-input");
  await input.fill("明天下午3点 交周报 #工作 !高");
  await expect(app.locator(".composer-parse")).toContainText("15:00");
  await input.press("Enter");

  await expect(taskRow(app, "交周报")).toBeVisible();
  const [call] = await invokeCalls(app, "create_task");
  expect(call.args.payload).toMatchObject({
    description: "交周报",
    tags: ["工作"],
    priority: 3,
    reminderTime: `${tomorrow()}T15:00:00`
  });
});

test("多选后批量完成", async ({ app }) => {
  await app.getByRole("button", { name: "多选" }).click();
  await taskRow(app, "买菜").click();
  await taskRow(app, "预约牙医").click();
  await app.locator(".batch-bar, .task-batch-bar").getByRole("button", { name: "完成", exact: true }).click();

  await expect(taskRow(app, "买菜")).toHaveCount(0);
  await expect(taskRow(app, "预约牙医")).toHaveCount(0);
  await expect(taskRow(app, "写季度报告")).toBeVisible();
  const [call] = await invokeCalls(app, "batch_update_tasks");
  expect(call.args).toEqual({ ids: ["t-groceries", "t-dentist"], payload: { action: "complete" } });
});

test("Ctrl+K 搜索便签正文并打开编辑", async ({ app }) => {
  await app.locator(".sidebar").getByText("今天").click();
  await app.keyboard.press("Control+k");
  const palette = app.getByRole("dialog", { name: "全局搜索" });
  await expect(palette).toBeVisible();
  await app.keyboard.type("审阅");

  const option = palette.getByRole("option");
  await expect(option).toHaveCount(1);
  await expect(option).toContainText("写季度报告");
  await expect(option.locator(".search-palette-snippet mark")).toHaveText("审阅");

  await app.keyboard.press("Enter");
  await expect(palette).toBeHidden();
  await expect(app.locator(".modal-header", { hasText: "编辑任务" })).toBeVisible();
  await expect(app.locator(".modal .input").first()).toHaveValue("写季度报告");
  // 跳到了待办页。
  await expect(app.locator(".section-title", { hasText: "待办事项" })).toBeVisible();

  // Esc 关闭编辑弹窗。
  await app.keyboard.press("Escape");
  await expect(app.locator(".modal-header", { hasText: "编辑任务" })).toBeHidden();
});

test("Ctrl+K 中 #标签 只匹配标签，Esc 关闭", async ({ app }) => {
  await app.keyboard.press("Control+k");
  const palette = app.getByRole("dialog", { name: "全局搜索" });
  await app.keyboard.type("#健康");
  await expect(palette.locator(".search-palette-group-title")).toHaveText([/待办/, /循环提醒/]);
  await expect(palette.getByRole("option")).toHaveText([/预约牙医/, /喝水/]);
  await app.keyboard.press("Escape");
  await expect(palette).toBeHidden();
});

test("编辑截止时间的提前量，只改提醒时间", async ({ app }) => {
  await taskRow(app, "写季度报告").dblclick();
  const modal = app.locator(".modal", { has: app.locator(".modal-header", { hasText: "编辑任务" }) });
  await modal.locator("select.select").first().selectOption("1440");
  await modal.getByRole("button", { name: "确认" }).click();
  await expect(modal).toBeHidden();

  const [call] = await invokeCalls(app, "update_task");
  const task = call.args.task as { dueAt: string; reminderTime: string; tags: string[]; priority: number };
  const state = await app.evaluate(() => (window as unknown as { __TAURI_MOCK__: { state: { tasks: Array<{ id: string; dueAt: string }> } } }).__TAURI_MOCK__.state.tasks.find(item => item.id === "t-report"));
  expect(task.dueAt).toBe(state!.dueAt);
  const due = new Date(task.dueAt);
  const reminder = new Date(task.reminderTime);
  expect(due.getTime() - reminder.getTime()).toBe(24 * 60 * 60 * 1000);
  expect(task.tags).toEqual(["工作"]);
  expect(task.priority).toBe(3);
});

test("便签清单进度", async ({ app }) => {
  await expect(taskRow(app, "写季度报告").locator(".checklist-chip")).toHaveText("2/4");
  await expect(taskRow(app, "买菜").locator(".checklist-chip")).toHaveText("0/3");
  await expect(taskRow(app, "预约牙医").locator(".checklist-chip")).toHaveCount(0);

  // 在新建待办的描述中写一个清单：Crepe 保存为 GFM 任务项，列表中显示进度。
  await app.locator(".composer-input").fill("周末大扫除");
  const editor = app.locator(".composer-editor .ProseMirror");
  await editor.click();
  await app.keyboard.type("- [ ] 擦窗户");
  await app.keyboard.press("Enter");
  await app.keyboard.type("拖地");
  // Milkdown 的内容变更事件有 200ms 防抖，等它把内容同步到输入框的 v-model。
  await app.waitForTimeout(400);
  await app.getByRole("button", { name: "添加任务" }).click();

  await expect(taskRow(app, "周末大扫除").locator(".checklist-chip")).toHaveText("0/2");
  const [call] = await invokeCalls(app, "create_task");
  expect((call.args.payload as { stickyContent: string }).stickyContent).toMatch(/[-*] \[ \] 擦窗户/);
});

test("统计：近 12 周按周显示，并按标签汇总", async ({ app }) => {
  await app.locator(".sidebar").getByText("统计").click();
  await app.getByRole("radio", { name: "近 12 周" }).click();
  await expect(app.locator(".stats-card-title", { hasText: "每周提醒" })).toBeVisible();
  await expect(app.locator(".stats-card-title", { hasText: "完成趋势" })).toBeVisible();
  await expect(app.locator(".daily-chart").first().locator(".chart-column")).toHaveCount(12);
  // 默认只保留 30 天的已完成待办，提示可调整。
  await expect(app.locator(".stat-tile", { hasText: "完成待办" })).toContainText("已完成只保留 30 天");

  const tagRows = app.locator(".tag-stats-table tbody tr");
  await expect(tagRows.first()).toContainText("#健康");
  await expect(app.locator(".tag-stats-table")).toContainText("#工作");
});

test("设置已完成待办的保留期", async ({ app }) => {
  await app.getByRole("button", { name: "设置", exact: true }).click();
  const modal = app.locator(".modal", { has: app.locator(".modal-header", { hasText: "应用设置" }) });
  const row = modal.locator(".form-row", { hasText: "已完成待办保留" });
  await row.locator("select").selectOption({ label: "1 年" });
  await expect(row).toContainText("完成超过 1 年的待办自动移入回收站");
  await modal.getByRole("button", { name: "确认" }).click();

  const calls = await invokeCalls(app, "save_settings");
  expect((calls.at(-1)!.args.settings as { completedRetentionDays: number }).completedRetentionDays).toBe(365);
});
