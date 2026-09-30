import { describe, expect, it } from "vitest";
import { isUserMarkdownChange } from "./markdownBaseline";

describe("isUserMarkdownChange", () => {
  const baseline = "## 议题\n\n* 发布计划\n";

  it("ignores updates fired while the editor is loading", () => {
    expect(isUserMarkdownChange({ ready: false, baseline: null, userEdited: false }, baseline)).toBe(false);
  });

  it("ignores the editor re-serializing loaded content", () => {
    expect(isUserMarkdownChange({ ready: true, baseline, userEdited: false }, baseline)).toBe(false);
  });

  it("emits real edits", () => {
    expect(isUserMarkdownChange({ ready: true, baseline, userEdited: false }, `${baseline}* 新的一行\n`)).toBe(true);
  });

  it("emits a revert to the original after the user edited", () => {
    expect(isUserMarkdownChange({ ready: true, baseline, userEdited: true }, baseline)).toBe(true);
  });
});
