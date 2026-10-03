import { describe, expect, it } from "vitest";
import { checklistProgress, markdownToPlainText, markdownToPreviewText, stripLeadingListMarker } from "./markdown";

describe("markdown helpers", () => {
  it("flattens markdown to plain text", () => {
    expect(markdownToPlainText("# 标题\n- [x] **完成** [链接](https://a.b)\n`code`")).toBe("标题 完成 链接 code");
    expect(markdownToPlainText(null)).toBe("");
  });

  it("falls back for empty previews", () => {
    expect(markdownToPreviewText("  ")).toBe("-");
    expect(markdownToPreviewText("", "")).toBe("");
  });

  it("strips leading list markers including zero-width characters", () => {
    expect(stripLeadingListMarker("​- [ ] 交电费")).toBe("交电费");
    expect(stripLeadingListMarker("• 喝水")).toBe("喝水");
    expect(stripLeadingListMarker("1. 第一步")).toBe("第一步");
  });
});

describe("checklistProgress", () => {
  it("counts checked and unchecked task items", () => {
    expect(checklistProgress("- [x] 初稿\n- [ ] 审阅\n- [X] 排版")).toEqual({ done: 2, total: 3 });
  });

  it("accepts *, + and ordered markers, nested items and empty items", () => {
    const md = ["* [ ] a", "+ [x] b", "1. [x] c", "2) [ ] d", "  - [x] nested", "- [ ]"].join("\n");
    expect(checklistProgress(md)).toEqual({ done: 3, total: 6 });
  });

  it("ignores plain lists, links and code blocks", () => {
    const md = ["- 普通列表", "- [链接](http://a)", "```", "- [ ] 代码里的", "```", "~~~md", "- [x] 也是代码", "~~~", "- [ ] 真的"].join("\n");
    expect(checklistProgress(md)).toEqual({ done: 0, total: 1 });
  });

  it("returns null without task items", () => {
    expect(checklistProgress("")).toBeNull();
    expect(checklistProgress(null)).toBeNull();
    expect(checklistProgress("只是文字 [x] 不是任务项")).toBeNull();
  });

  it("handles Windows line endings", () => {
    expect(checklistProgress("- [x] a\r\n- [ ] b")).toEqual({ done: 1, total: 2 });
  });
});
