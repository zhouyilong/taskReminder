import { describe, expect, it } from "vitest";
import {
  collectLabelUsage,
  collectProjects,
  collectTags,
  filterAndSortTasks,
  findOtherLabel,
  followRename,
  normalizeProject,
  normalizeTags,
  sortTasks
} from "./tasks";
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

  it("normalizes project names like the backend", () => {
    expect(normalizeProject("  @装修  ")).toBe("装修");
    expect(normalizeProject("＠＠家\u0007务")).toBe("家务");
    expect(normalizeProject(" @ ")).toBe("");
    expect(normalizeProject(null)).toBe("");
    expect(Array.from(normalizeProject("长".repeat(40)))).toHaveLength(32);
  });

  it("collects projects and filters by project or ungrouped", () => {
    const list = [
      task("a", { project: "装修" }),
      task("bb", { project: "工作" }),
      task("ccc", { project: "装修" }),
      task("dddd")
    ];
    expect(collectProjects(list)).toEqual([
      { project: "装修", count: 2 },
      { project: "工作", count: 1 }
    ]);
    const filter = { query: "", tag: "", priority: -1 };
    expect(ids(filterAndSortTasks(list, { ...filter, project: "装修" }, "created"))).toEqual(["a", "ccc"]);
    expect(ids(filterAndSortTasks(list, { ...filter, project: "" }, "created"))).toEqual(["dddd"]);
    expect(ids(filterAndSortTasks(list, { ...filter, project: null }, "created"))).toHaveLength(4);
    expect(ids(filterAndSortTasks(list, { ...filter, query: "@工" }, "created"))).toEqual(["bb"]);
  });
});

describe("标签与项目管理", () => {
  it("汇总待办、已完成与循环提醒中的标签，项目只来自待办", () => {
    const usage = collectLabelUsage(
      [{ tags: ["工作"], project: "装修" }],
      [{ tags: ["工作", "家务"], project: "装修" }],
      [{ tags: ["工作"] }]
    );
    expect(usage.tags).toEqual([
      { tag: "工作", count: 3 },
      { tag: "家务", count: 1 }
    ]);
    expect(usage.projects).toEqual([{ project: "装修", count: 2 }]);
  });

  it("改成已有的名称时提示合并，只改大小写不算", () => {
    const names = ["工作", "Home", "家务"];
    expect(findOtherLabel(names, "家务", "home")).toBe("Home");
    expect(findOtherLabel(names, "Home", "HOME")).toBeNull();
    expect(findOtherLabel(names, "家务", "新名称")).toBeNull();
  });

  it("筛选跟随改名", () => {
    expect(followRename("Work", "work", "工作")).toBe("工作");
    expect(followRename("家务", "work", "工作")).toBe("家务");
    expect(followRename("", "work", "工作")).toBe("");
    // 移除标签回到全部；项目移出后筛选未分组，未筛选项目时不变。
    expect(followRename("work", "work", "")).toBe("");
    expect(followRename<string | null>(null, "装修", "")).toBeNull();
    expect(followRename<string | null>("装修", "装修", "")).toBe("");
  });
});
