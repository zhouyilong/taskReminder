export type TabKey = "today" | "tasks" | "completed" | "recurring" | "records" | "stats" | "trash";

export const TAB_KEYS: TabKey[] = ["today", "tasks", "completed", "recurring", "records", "stats", "trash"];

export const isTabKey = (value: unknown): value is TabKey =>
  typeof value === "string" && (TAB_KEYS as string[]).includes(value);
