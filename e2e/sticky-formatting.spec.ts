import type { Locator } from "@playwright/test";
import { expect, invokeCalls, openStickyNote, test } from "./fixtures";

async function selectText(locator: Locator, start = 0, end?: number) {
  await locator.evaluate((element, offsets) => {
    element.closest<HTMLElement>(".ProseMirror")!.focus();
    const range = document.createRange();
    if (offsets.end === undefined && offsets.start === 0) {
      range.selectNodeContents(element);
    } else {
      range.setStart(element.firstChild!, offsets.start);
      range.setEnd(element.firstChild!, offsets.end!);
    }
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
  }, { start, end });
}

test("便签文字颜色可保存、重新打开和清除", async ({ page, context }) => {
  await page.setViewportSize({ width: 220, height: 180 });
  await openStickyNote(page, "note", "正文前 **加粗文字** 正文后");
  const editor = page.locator(".ProseMirror");
  const trigger = page.getByRole("button", { name: "文字颜色", exact: true });
  await expect(trigger).toBeDisabled();
  await selectText(editor.locator("strong"));
  await trigger.click();
  const palette = page.getByRole("group", { name: "文字颜色选项" });
  await expect(palette).toBeInViewport({ ratio: 1 });
  await palette.getByRole("button", { name: "红色" }).click();
  await expect(editor.locator('[data-note-color="red"]')).toHaveText("加粗文字");

  await page.getByTitle("关闭", { exact: true }).click();
  await expect.poll(async () => (await invokeCalls(page, "save_sticky_note_content")).length).toBe(1);
  const [save] = await invokeCalls(page, "save_sticky_note_content");
  const { content } = save.args.payload as { content: string };
  expect(content).toContain('data-note-color="red"');
  expect(content).toContain("**加粗文字**");

  const reopened = await context.newPage();
  await openStickyNote(reopened, "note", content);
  const colored = reopened.locator('.ProseMirror [data-note-color="red"]');
  await expect(colored).toHaveText("加粗文字");
  await expect(reopened.locator(".ProseMirror strong")).toHaveText("加粗文字");
  await selectText(colored);
  await reopened.getByRole("button", { name: "文字颜色", exact: true }).click();
  await reopened.getByRole("button", { name: "默认文字颜色" }).click();
  await expect(colored).toHaveCount(0);
  await expect(reopened.locator(".ProseMirror strong")).toHaveText("加粗文字");
});

test("整行删除线移至便签末尾并可一次撤销，局部删除线保留原位置", async ({ page }) => {
  await openStickyNote(page, "note", "~~已有删除线~~\n\n计划整理资料\n\n后续事项");
  const editor = page.locator(".ProseMirror");
  const rows = editor.locator(":scope > p");
  await expect(rows).toHaveText(["已有删除线", "计划整理资料", "后续事项"]);
  const plan = rows.filter({ hasText: "计划整理资料" });
  await selectText(plan, 0, 2);
  await page.keyboard.press("Control+Alt+x");
  await expect(plan.locator("del")).toHaveText("计划");
  await expect(rows).toHaveText(["已有删除线", "计划整理资料", "后续事项"]);

  await page.keyboard.press("Control+z");
  await expect(plan.locator("del")).toHaveCount(0);
  await selectText(plan);
  await page.locator(".milkdown-toolbar button").nth(2).click();
  await expect(plan.locator("del")).toHaveText("计划整理资料");
  await expect(rows).toHaveText(["已有删除线", "后续事项", "计划整理资料"]);

  await page.keyboard.press("Control+z");
  await expect(rows).toHaveText(["已有删除线", "计划整理资料", "后续事项"]);
  await expect(plan.locator("del")).toHaveCount(0);
  await page.keyboard.press("Control+y");
  await expect(rows).toHaveText(["已有删除线", "后续事项", "计划整理资料"]);
  await page.getByTitle("关闭", { exact: true }).click();
  await expect.poll(async () => (await invokeCalls(page, "save_sticky_note_content")).length).toBe(1);
  const [save] = await invokeCalls(page, "save_sticky_note_content");
  expect(save.args.payload).toMatchObject({ content: expect.stringMatching(/后续事项\n+~~计划整理资料~~/) });
});

test("清单项移至正文末尾并保留勾选状态", async ({ page }) => {
  await openStickyNote(page, "note", "- [x] 收集资料\n- [ ] 写初稿\n\n补充说明");
  const editor = page.locator(".ProseMirror");
  await selectText(editor.getByText("收集资料", { exact: true }));
  await page.keyboard.press("Control+Alt+x");
  await expect(editor.locator(":scope > :is(ul, p)")).toHaveText(["写初稿", "补充说明", "收集资料", ""]);
  await expect(editor.locator("li").last().locator("del")).toHaveText("收集资料");
  await expect(page.locator(".paper-note-progress-text")).toHaveText("1/2");
  await page.getByTitle("关闭", { exact: true }).click();
  await expect.poll(async () => (await invokeCalls(page, "save_sticky_note_content")).length).toBe(1);
  const [save] = await invokeCalls(page, "save_sticky_note_content");
  expect(save.args.payload).toMatchObject({ content: expect.stringMatching(/补充说明\n+\* \[x\] ~~收集资料~~/) });
});

test("手动换行的便签正文可只移动其中一行", async ({ page }) => {
  await openStickyNote(page, "note", "先处理\\\n接着处理\\\n最后处理");
  const paragraph = page.locator(".ProseMirror > p");
  await expect(paragraph.locator("br[data-type='hardbreak']")).toHaveCount(2);
  await selectText(paragraph, 0, 3);
  await page.keyboard.press("Control+Alt+x");
  await expect(paragraph.locator("del")).toHaveText("先处理");
  await expect(paragraph).toHaveText("接着处理最后处理先处理");
  await expect(paragraph.locator("br[data-type='hardbreak']")).toHaveCount(2);
});
