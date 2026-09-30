import { describe, expect, it } from "vitest";
import { buildStickyList, countStickyStates, stickyState } from "./stickies";
import type { StickyNoteSummary } from "./types";

const note = (overrides: Partial<StickyNoteSummary>): StickyNoteSummary => ({
  taskId: "id",
  title: "便签",
  noteType: "TASK",
  content: "",
  posX: 0,
  posY: 0,
  width: 284,
  height: 280,
  isOpen: true,
  isPinned: false,
  createdAt: "2026-09-01T08:00:00",
  updatedAt: "2026-09-01T08:00:00",
  reminderTime: null,
  visible: true,
  ...overrides,
});

describe("stickyState", () => {
  it("distinguishes showing, hidden and closed", () => {
    expect(stickyState({ isOpen: true, visible: true })).toBe("showing");
    expect(stickyState({ isOpen: true, visible: false })).toBe("hidden");
    // 关闭后窗口可能还在（只是隐藏），以 isOpen 为准。
    expect(stickyState({ isOpen: false, visible: true })).toBe("closed");
  });
});

describe("buildStickyList", () => {
  const notes = [
    note({ taskId: "closed-new", isOpen: false, visible: false, updatedAt: "2026-09-30 09:00:00" }),
    note({ taskId: "showing-old", updatedAt: "2026-09-01T08:00:00" }),
    note({ taskId: "hidden", visible: false, updatedAt: "2026-09-29T08:00:00" }),
    note({ taskId: "showing-new", updatedAt: "2026-09-20T08:00:00" }),
  ];

  it("orders showing, hidden, closed and newest first within a group", () => {
    expect(buildStickyList(notes).map(item => item.note.taskId)).toEqual([
      "showing-new",
      "showing-old",
      "hidden",
      "closed-new",
    ]);
  });

  it("filters open and closed notes", () => {
    expect(buildStickyList(notes, "", "open").map(item => item.state)).toEqual(["showing", "showing", "hidden"]);
    expect(buildStickyList(notes, "", "closed").map(item => item.note.taskId)).toEqual(["closed-new"]);
  });

  it("searches title and markdown content as plain text", () => {
    const list = [
      note({ taskId: "a", title: "周会准备", content: "## 议题\n- **发布计划**" }),
      note({ taskId: "b", title: "购物", content: "- [ ] 牛奶" }),
    ];
    expect(buildStickyList(list, "周会").map(item => item.note.taskId)).toEqual(["a"]);
    // 内容里的 ** 已去掉，按纯文本匹配。
    expect(buildStickyList(list, "发布计划").map(item => item.note.taskId)).toEqual(["a"]);
    expect(buildStickyList(list, "  牛奶 ").map(item => item.note.taskId)).toEqual(["b"]);
    expect(buildStickyList(list, "议题")[0].preview).toBe("议题 发布计划");
  });
});

describe("countStickyStates", () => {
  it("counts each state", () => {
    expect(
      countStickyStates([
        note({}),
        note({ visible: false }),
        note({ isOpen: false, visible: false }),
        note({ isOpen: false, visible: false }),
      ]),
    ).toEqual({ showing: 1, hidden: 1, closed: 2 });
  });
});
