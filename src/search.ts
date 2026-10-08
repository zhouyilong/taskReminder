// 全局搜索（Ctrl+K）与各列表的关键词搜索：纯函数，便于测试。
//
// 规则：查询按空白拆成多个词，全部命中才算匹配（且）；不区分大小写。
// 以 # 开头的词只匹配标签（前缀，“#工”可命中“#工作”），以 @ 开头的词只匹配项目（前缀，v2.2），
// 其余的词在标题、标签、项目、正文中任一处命中即可。
import { taskAnchorTime } from "./due";
import { markdownToPlainText } from "./markdown";
import type { RecurringTask, Task } from "./types";

export interface ParsedQuery {
  /** 普通词（已转小写）。 */
  terms: string[];
  /** # 开头的标签词（去掉 #，已转小写）。 */
  tags: string[];
  /** @ 开头的项目词（去掉 @，已转小写，v2.2）。 */
  projects: string[];
}

export const parseQuery = (query: string): ParsedQuery => {
  const terms: string[] = [];
  const tags: string[] = [];
  const projects: string[] = [];
  for (const part of query.trim().toLowerCase().split(/\s+/)) {
    if (!part) continue;
    const tag = /^[#＃]+(.*)$/.exec(part);
    const project = /^[@＠]+(.*)$/.exec(part);
    if (tag) {
      if (tag[1]) tags.push(tag[1]);
    } else if (project) {
      if (project[1]) projects.push(project[1]);
    } else {
      terms.push(part);
    }
  }
  return { terms, tags, projects };
};

export const isEmptyQuery = (query: ParsedQuery) =>
  query.terms.length === 0 && query.tags.length === 0 && query.projects.length === 0;

/** 标签词、项目词（都是前缀匹配）是否全部命中。 */
const matchesMeta = (tags: string[], project: string, query: ParsedQuery) =>
  query.tags.every(tag => tags.some(item => item.startsWith(tag))) &&
  query.projects.every(item => project.startsWith(item));

/** 关键词搜索：每个词都要在某个字段中出现；标签词只与标签比较、项目词只与项目比较（前缀）。 */
export const matchesKeyword = (
  fields: { text: Array<string | null | undefined>; tags?: string[]; project?: string | null },
  query: string | ParsedQuery
) => {
  const parsed = typeof query === "string" ? parseQuery(query) : query;
  if (isEmptyQuery(parsed)) return true;
  const tags = (fields.tags ?? []).map(tag => tag.toLowerCase());
  const project = (fields.project ?? "").toLowerCase();
  if (!matchesMeta(tags, project, parsed)) return false;
  const haystack = [...fields.text, ...tags.map(tag => `#${tag}`), project ? `@${project}` : ""]
    .filter(Boolean)
    .join("\n")
    .toLowerCase();
  return parsed.terms.every(term => haystack.includes(term));
};

export type SearchKind = "task" | "recurring" | "completed";

export const SEARCH_GROUPS: Array<{ kind: SearchKind; label: string }> = [
  { kind: "task", label: "待办" },
  { kind: "recurring", label: "循环提醒" },
  { kind: "completed", label: "已办" }
];

/** 命中的字段：标题 > 标签 > 正文，用于排序与决定展示哪段文字。 */
export type SearchField = "title" | "tag" | "body";
const FIELD_RANK: Record<SearchField, number> = { title: 0, tag: 1, body: 2 };

export interface TextSegment {
  text: string;
  hit: boolean;
}

export type SearchHit =
  | { kind: "task" | "completed"; id: string; item: Task; field: SearchField; title: TextSegment[]; snippet: TextSegment[] | null; tags: string[]; hasSticky: boolean }
  | { kind: "recurring"; id: string; item: RecurringTask; field: SearchField; title: TextSegment[]; snippet: TextSegment[] | null; tags: string[]; hasSticky: false };

export interface SearchGroup {
  kind: SearchKind;
  label: string;
  hits: SearchHit[];
  /** 命中总数（hits 只取前 limit 条）。 */
  total: number;
}

/** 把文本按命中的词切成片段，供界面高亮。 */
export const highlight = (text: string, terms: string[]): TextSegment[] => {
  const lower = text.toLowerCase();
  const marks = new Array<boolean>(text.length).fill(false);
  for (const term of terms) {
    if (!term) continue;
    let index = lower.indexOf(term);
    while (index >= 0) {
      marks.fill(true, index, index + term.length);
      index = lower.indexOf(term, index + term.length);
    }
  }
  const segments: TextSegment[] = [];
  for (let index = 0; index < text.length; index += 1) {
    const last = segments[segments.length - 1];
    if (last && last.hit === marks[index]) {
      last.text += text[index];
    } else {
      segments.push({ text: text[index], hit: marks[index] });
    }
  }
  return segments;
};

/** 从正文中截取第一个命中词前后的一段（两端超出时加省略号）。 */
export const excerpt = (text: string, terms: string[], radius = 24) => {
  const lower = text.toLowerCase();
  const positions = terms.map(term => lower.indexOf(term)).filter(index => index >= 0);
  if (!positions.length) return text.length > radius * 2 ? `${text.slice(0, radius * 2)}…` : text;
  const first = Math.min(...positions);
  const start = Math.max(0, first - radius);
  const end = Math.min(text.length, first + radius * 2);
  return `${start > 0 ? "…" : ""}${text.slice(start, end)}${end < text.length ? "…" : ""}`;
};

interface Candidate {
  title: string;
  tags: string[];
  /** 项目（v2.2），与标签同一档。 */
  project: string;
  body: string;
}

/** 判断是否命中并找出最靠前的字段；不命中返回 null。 */
const matchCandidate = (candidate: Candidate, query: ParsedQuery): SearchField | null => {
  const tags = candidate.tags.map(tag => tag.toLowerCase());
  const project = candidate.project.toLowerCase();
  if (!matchesMeta(tags, project, query)) return null;
  const title = candidate.title.toLowerCase();
  const tagText = [...tags.map(tag => `#${tag}`), project ? `@${project}` : ""].join(" ");
  const body = candidate.body.toLowerCase();
  let best: SearchField | null = query.terms.length
    ? null
    : query.tags.length || query.projects.length
      ? "tag"
      : null;
  for (const term of query.terms) {
    const field: SearchField | null = title.includes(term) ? "title" : tagText.includes(term) ? "tag" : body.includes(term) ? "body" : null;
    if (!field) return null;
    if (!best || FIELD_RANK[field] < FIELD_RANK[best]) best = field;
  }
  return best;
};

const compareTime = (a?: string | null, b?: string | null) => {
  if (a && b) return a.localeCompare(b);
  if (a) return -1;
  if (b) return 1;
  return 0;
};

export const searchAll = (
  input: { tasks: Task[]; recurringTasks: RecurringTask[]; completedTasks: Task[] },
  rawQuery: string,
  limit = 8
): SearchGroup[] => {
  const query = parseQuery(rawQuery);
  if (isEmptyQuery(query)) return [];
  const terms = query.terms;

  const taskHits = (kind: "task" | "completed", tasks: Task[]) =>
    tasks.flatMap(task => {
      const body = markdownToPlainText(task.stickyContent);
      const tags = task.tags ?? [];
      const field = matchCandidate({ title: task.description, tags, project: task.project ?? "", body }, query);
      if (!field) return [];
      const snippetSource = field === "body" ? excerpt(body, terms) : null;
      return [
        {
          kind,
          id: task.id,
          item: task,
          field,
          title: highlight(task.description, terms),
          snippet: snippetSource ? highlight(snippetSource, terms) : null,
          tags,
          hasSticky: !!task.stickyContent?.trim()
        } as SearchHit
      ];
    });

  const recurringHits = input.recurringTasks.flatMap(task => {
    const tags = task.tags ?? [];
    const field = matchCandidate({ title: task.description, tags, project: "", body: "" }, query);
    if (!field) return [];
    return [
      {
        kind: "recurring",
        id: task.id,
        item: task,
        field,
        title: highlight(task.description, terms),
        snippet: null,
        tags,
        hasSticky: false
      } as SearchHit
    ];
  });

  const sortHits = (hits: SearchHit[], time: (hit: SearchHit) => string | null | undefined, descending = false) =>
    [...hits].sort(
      (a, b) =>
        FIELD_RANK[a.field] - FIELD_RANK[b.field] ||
        (descending ? compareTime(time(b), time(a)) : compareTime(time(a), time(b))) ||
        a.item.description.localeCompare(b.item.description)
    );

  const byKind: Record<SearchKind, SearchHit[]> = {
    task: sortHits(taskHits("task", input.tasks), hit => taskAnchorTime(hit.item as Task)),
    recurring: sortHits(recurringHits, hit => (hit.item as RecurringTask).nextTrigger),
    // 已办按完成时间倒序：最近完成的在前。
    completed: sortHits(taskHits("completed", input.completedTasks), hit => (hit.item as Task).completedAt, true)
  };

  return SEARCH_GROUPS.map(group => ({
    kind: group.kind,
    label: group.label,
    hits: byKind[group.kind].slice(0, limit),
    total: byKind[group.kind].length
  })).filter(group => group.total > 0);
};

/** 按界面顺序把各组的命中展开成一列，供 ↑↓ 选择。 */
export const flattenHits = (groups: SearchGroup[]) => groups.flatMap(group => group.hits);

/** “2026-10-05T09:00:00” → “10月5日 09:00”。 */
const shortDateTime = (value: string) => {
  const match = /^\d{4}-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(value);
  return match ? `${Number(match[1])}月${Number(match[2])}日 ${match[3]}:${match[4]}` : value;
};

/** 结果行右侧的时间说明。 */
export const hitMeta = (hit: SearchHit) => {
  if (hit.kind === "recurring") {
    return hit.item.isPaused ? "已暂停" : `下次 ${shortDateTime(hit.item.nextTrigger)}`;
  }
  if (hit.kind === "completed") {
    return hit.item.completedAt ? `完成于 ${shortDateTime(hit.item.completedAt)}` : "";
  }
  if (hit.item.dueAt) return `截止 ${shortDateTime(hit.item.dueAt)}`;
  if (hit.item.reminderTime) return `提醒 ${shortDateTime(hit.item.reminderTime)}`;
  return "";
};
