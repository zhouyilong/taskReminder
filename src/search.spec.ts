import { describe, expect, it } from "vitest";
import { excerpt, flattenHits, highlight, hitMeta, matchesKeyword, parseQuery, searchAll } from "./search";
import type { RecurringTask, Task } from "./types";

const task = (id: string, description: string, extra: Partial<Task> = {}): Task => ({
  id,
  description,
  type: "ONE_TIME",
  status: "PENDING",
  createdAt: "2026-09-01T00:00:00",
  tags: [],
  ...extra
});

const recurring = (id: string, description: string, extra: Partial<RecurringTask> = {}): RecurringTask => ({
  id,
  description,
  type: "RECURRING",
  status: "PENDING",
  createdAt: "2026-09-01T00:00:00",
  intervalMinutes: 60,
  nextTrigger: "2026-10-04T09:00:00",
  isPaused: false,
  repeatMode: "DAILY",
  tags: [],
  ...extra
});

const text = (segments: Array<{ text: string }> | null) => segments?.map(item => item.text).join("") ?? null;
const hits = (segments: Array<{ text: string; hit: boolean }> | null) =>
  segments?.filter(item => item.hit).map(item => item.text) ?? [];

describe("parseQuery", () => {
  it("splits terms and #tags, ignoring a bare #", () => {
    expect(parseQuery("  周报 #工作  ＃Home # ")).toEqual({ terms: ["周报"], tags: ["工作", "home"], projects: [] });
  });

  it("splits @projects, ignoring a bare @", () => {
    expect(parseQuery("瓷砖 @装修 ＠Home @")).toEqual({ terms: ["瓷砖"], tags: [], projects: ["装修", "home"] });
  });
});

describe("matchesKeyword", () => {
  it("requires every term, case-insensitively", () => {
    const fields = { text: ["Weekly Report", "汇总数据"], tags: ["工作"] };
    expect(matchesKeyword(fields, "report 汇总")).toBe(true);
    expect(matchesKeyword(fields, "report 预算")).toBe(false);
    expect(matchesKeyword(fields, "")).toBe(true);
  });

  it("matches #tag terms against tags by prefix only", () => {
    const fields = { text: ["工作安排"], tags: ["工作"] };
    expect(matchesKeyword(fields, "#工")).toBe(true);
    expect(matchesKeyword(fields, "#作")).toBe(false);
    expect(matchesKeyword({ text: ["工作安排"], tags: [] }, "#工作")).toBe(false);
  });
});

describe("@project terms", () => {
  it("matches projects by prefix and plain terms against the project too", () => {
    const fields = { text: ["买瓷砖"], tags: ["采购"], project: "装修" };
    expect(matchesKeyword(fields, "@装")).toBe(true);
    expect(matchesKeyword(fields, "@修")).toBe(false);
    expect(matchesKeyword(fields, "装修 瓷砖")).toBe(true);
    expect(matchesKeyword({ text: ["买瓷砖"] }, "@装修")).toBe(false);
  });

  it("filters global search by project and ranks it with tags", () => {
    const groups = searchAll(
      {
        tasks: [task("a", "买瓷砖", { project: "装修" }), task("b", "写周报")],
        recurringTasks: [recurring("r", "装修进度")],
        completedTasks: [task("c", "量尺寸", { status: "COMPLETED", project: "装修" })]
      },
      "@装修"
    );
    expect(groups.map(group => [group.kind, group.hits.map(hit => hit.id), group.hits[0].field])).toEqual([
      ["task", ["a"], "tag"],
      ["completed", ["c"], "tag"]
    ]);
  });
});

describe("highlight / excerpt", () => {
  it("marks every occurrence of every term", () => {
    expect(highlight("周报周报总结", ["周报"]).map(item => [item.text, item.hit])).toEqual([
      ["周报周报", true],
      ["总结", false]
    ]);
    expect(hits(highlight("Plan the Plan", ["plan"]))).toEqual(["Plan", "Plan"]);
  });

  it("cuts around the first hit with ellipses", () => {
    const body = `${"甲".repeat(40)}关键${"乙".repeat(80)}`;
    const result = excerpt(body, ["关键"], 10);
    expect(result.startsWith("…")).toBe(true);
    expect(result.endsWith("…")).toBe(true);
    expect(result).toContain("关键");
    expect(excerpt("短文本", ["无"])).toBe("短文本");
  });
});

describe("searchAll", () => {
  const input = {
    tasks: [
      task("body", "整理资料", { stickyContent: "- [ ] 准备**周报**数据", reminderTime: "2026-10-05T09:00:00" }),
      task("title-late", "写周报", { reminderTime: "2026-10-06T09:00:00" }),
      task("title-early", "周报评审", { dueAt: "2026-10-04T18:00:00" }),
      task("tag", "整理", { tags: ["周报"] }),
      task("other", "买菜")
    ],
    recurringTasks: [recurring("rec", "每周五交周报", { tags: ["工作"] })],
    completedTasks: [
      task("done-old", "上周周报", { status: "COMPLETED", completedAt: "2026-09-26T18:00:00" }),
      task("done-new", "本周周报", { status: "COMPLETED", completedAt: "2026-10-02T18:00:00" })
    ]
  };

  it("returns nothing for an empty query", () => {
    expect(searchAll(input, "   ")).toEqual([]);
  });

  it("groups by kind and ranks title > tag > body, then by time", () => {
    const groups = searchAll(input, "周报");
    expect(groups.map(group => group.kind)).toEqual(["task", "recurring", "completed"]);
    expect(groups[0].hits.map(hit => hit.id)).toEqual(["title-early", "title-late", "tag", "body"]);
    // 已办按完成时间倒序。
    expect(groups[2].hits.map(hit => hit.id)).toEqual(["done-new", "done-old"]);
  });

  it("shows a plain-text snippet only for body hits", () => {
    const [tasks] = searchAll(input, "周报");
    const body = tasks.hits.find(hit => hit.id === "body")!;
    expect(body.field).toBe("body");
    expect(text(body.snippet)).toBe("准备周报数据");
    expect(hits(body.snippet)).toEqual(["周报"]);
    expect(body.hasSticky).toBe(true);
    expect(tasks.hits.find(hit => hit.id === "title-late")!.snippet).toBeNull();
  });

  it("requires all terms and supports #tag filters", () => {
    expect(flattenHits(searchAll(input, "周报 评审")).map(hit => hit.id)).toEqual(["title-early"]);
    expect(flattenHits(searchAll(input, "#工作")).map(hit => hit.id)).toEqual(["rec"]);
    expect(flattenHits(searchAll(input, "#工作 买菜"))).toEqual([]);
  });

  it("limits each group but reports the total", () => {
    const many = { tasks: Array.from({ length: 12 }, (_, index) => task(`t${index}`, `周报 ${index}`)), recurringTasks: [], completedTasks: [] };
    const [group] = searchAll(many, "周报", 8);
    expect(group.hits).toHaveLength(8);
    expect(group.total).toBe(12);
  });

  it("describes each hit's time", () => {
    const all = flattenHits(searchAll(input, "周报"));
    const meta = (id: string) => hitMeta(all.find(hit => hit.id === id)!);
    expect(meta("title-early")).toBe("截止 10月4日 18:00");
    expect(meta("title-late")).toBe("提醒 10月6日 09:00");
    expect(meta("tag")).toBe("");
    expect(meta("rec")).toBe("下次 10月4日 09:00");
    expect(meta("done-new")).toBe("完成于 10月2日 18:00");
  });
});
