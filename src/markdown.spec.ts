import { describe, expect, it } from "vitest";
import { markdownToPlainText, markdownToPreviewText, stripLeadingListMarker } from "./markdown";

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
