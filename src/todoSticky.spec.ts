import { describe, expect, it } from "vitest";
import { dropIndexAt, parseTodoInput, todoStickyItems, todoStickySummary } from "./todoSticky";
import type { Task } from "./types";

const task = (id: string, extra: Partial<Task> = {}): Task => ({
  id,
  description: id,
  type: "ONE_TIME",
  status: "PENDING",
  createdAt: `2026-11-01T0${id.length}:00:00`,
  ...extra,
});

describe("待办清单便签", () => {
  const now = new Date(2026, 10, 4, 10, 0);

  it("有时间的按先后排在前面，没有时间的按优先级", () => {
    const items = todoStickyItems(
      [
        task("a"),
        task("bb", { priority: 3 }),
        task("ccc", { reminderTime: "2026-11-05T09:00:00" }),
        task("dddd", { dueAt: "2026-11-03T18:00:00", reminderTime: "2026-11-03T17:00:00" }),
        task("eeeee", { reminderTime: "2026-11-04T15:30:00" }),
        task("done", { status: "COMPLETED" }),
        task("gone", { deletedAt: "2026-11-02T00:00:00" }),
      ],
      now
    );
    expect(items.map(item => item.task.id)).toEqual(["dddd", "eeeee", "ccc", "bb", "a"]);
    expect(items.map(item => item.tone)).toEqual(["overdue", "today", "later", "none", "none"]);
    expect(items[0].when).toBe("11月3日 18:00");
    expect(items[1].when).toBe("今天 15:30");
    expect(items[2].when).toBe("明天 09:00");
    expect(items[3].when).toBe("");
    expect(todoStickySummary(items)).toBe("5 项 · 1 项已过期");
    expect(todoStickySummary([])).toBe("没有待办");
  });

  it("拖动排过的按位置在前，之后新建的按默认顺序排在后面", () => {
    const items = todoStickyItems(
      [
        task("a", { sortOrder: 2048 }),
        task("bb", { sortOrder: 1024 }),
        task("ccc", { reminderTime: "2026-11-05T09:00:00" }),
        task("dddd"),
        task("eeeee", { reminderTime: "2026-11-04T15:30:00", sortOrder: Number.NaN }),
      ],
      now
    );
    expect(items.map(item => item.task.id)).toEqual(["bb", "a", "eeeee", "ccc", "dddd"]);
  });

  it("加急的待办带标识并计入头部文案", () => {
    const items = todoStickyItems(
      [task("a", { urgent: true }), task("bb", { dueAt: "2026-11-03T18:00:00", urgent: true }), task("ccc")],
      now
    );
    expect(items.map(item => item.urgent)).toEqual([true, true, false]);
    expect(todoStickySummary(items)).toBe("3 项 · 2 项加急 · 1 项已过期");
    expect(todoStickySummary(items.slice(2))).toBe("1 项");
  });

  it("按指针位置计算插入位置", () => {
    const rows = [
      { top: 0, height: 32 },
      { top: 32, height: 32 },
      { top: 64, height: 32 },
    ];
    expect(dropIndexAt(rows, -5)).toBe(0);
    expect(dropIndexAt(rows, 15)).toBe(0);
    expect(dropIndexAt(rows, 17)).toBe(1);
    expect(dropIndexAt(rows, 60)).toBe(2);
    expect(dropIndexAt(rows, 90)).toBe(3);
    expect(dropIndexAt([], 10)).toBe(0);
  });

  it("标题只取第一行", () => {
    const [item] = todoStickyItems([task("x", { description: "\n  买牛奶 \n第二行" })], now);
    expect(item.title).toBe("买牛奶");
  });

  it("输入框按自然语言识别时间、标签与项目", () => {
    expect(parseTodoInput("   ", now)).toBeNull();
    const plain = parseTodoInput("买牛奶", now);
    expect(plain).toMatchObject({ kind: "task", payload: { description: "买牛奶", reminderTime: null }, hint: "已添加" });
    const timed = parseTodoInput("明天下午3点 交周报 #工作 @项目A", now);
    expect(timed?.kind).toBe("task");
    if (timed?.kind === "task") {
      expect(timed.payload.reminderTime).toBe("2026-11-05T15:00:00");
      expect(timed.payload.tags).toEqual(["工作"]);
      expect(timed.payload.project).toBe("项目A");
      expect(timed.payload.description).toBe("交周报");
      expect(timed.hint).toBe("已添加，明天 15:00 提醒");
    }
    expect(parseTodoInput("每天早上9点 吃药", now)?.kind).toBe("recurring");
  });
});
