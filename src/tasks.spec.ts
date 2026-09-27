import { describe, expect, it } from "vitest";
import { collectTags, filterAndSortTasks, normalizeTags, sortTasks } from "./tasks";
import type { Task } from "./types";

const task = (id: string, extra: Partial<Task> = {}): Task => ({
  id,
  description: id,
  type: "ONE_TIME",
  status: "PENDING",
  createdAt: `2026-09-0${id.length}T00:00:00`,
  ...extra,
});

const ids = (tasks: Task[]) => tasks.map(item => item.id);

describe("tasks helpers", () => {
  it("normalizes tags like the backend", () => {
    expect(normalizeTags(["#工作", " 工作 ", "Work", "work", "a,b", ""])).toEqual(["工作", "Work", "ab"]);
  });

  it("collects tags with counts", () => {
    const list = [task("a", { tags: ["工作", "周报"] }), task("bb", { tags: ["工作"] }), task("ccc")];
    expect(collectTags(list)).toEqual([
      { tag: "工作", count: 2 },
      { tag: "周报", count: 1 },
    ]);
  });

  it("filters by tag, priority and query", () => {
    const list = [
      task("a", { tags: ["工作"], priority: 3, description: "写周报" }),
      task("bb", { tags: ["生活"], priority: 1, stickyContent: "买牛奶" }),
      task("ccc", { priority: 3 }),
    ];
    expect(ids(filterAndSortTasks(list, { query: "", tag: "工作", priority: -1 }, "created"))).toEqual(["a"]);
    expect(ids(filterAndSortTasks(list, { query: "", tag: "", priority: 3 }, "created"))).toEqual(["a", "ccc"]);
    expect(ids(filterAndSortTasks(list, { query: "牛奶", tag: "", priority: -1 }, "created"))).toEqual(["bb"]);
    expect(ids(filterAndSortTasks(list, { query: "#生活", tag: "", priority: -1 }, "created"))).toEqual(["bb"]);
  });

  it("sorts by reminder time with unset last, and by priority", () => {
    const list = [
      task("a", { reminderTime: "2026-09-30T09:00:00", priority: 1 }),
      task("bb"),
      task("ccc", { reminderTime: "2026-09-28T09:00:00", priority: 3 }),
      task("dddd", { priority: 3 }),
    ];
    expect(ids(sortTasks(list, "reminder"))).toEqual(["ccc", "a", "bb", "dddd"]);
    expect(ids(sortTasks(list, "priority"))).toEqual(["ccc", "dddd", "a", "bb"]);
    expect(ids(sortTasks(list, "created"))).toEqual(["a", "bb", "ccc", "dddd"]);
  });
});
