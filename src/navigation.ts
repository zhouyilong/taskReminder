export type TabKey =
  | "today"
  | "calendar"
  | "tasks"
  | "completed"
  | "recurring"
  | "stickies"
  | "records"
  | "stats"
  | "trash";

export const TAB_KEYS: TabKey[] = [
  "today",
  "calendar",
  "tasks",
  "completed",
  "recurring",
  "stickies",
  "records",
  "stats",
  "trash",
];

export const isTabKey = (value: unknown): value is TabKey =>
  typeof value === "string" && (TAB_KEYS as string[]).includes(value);
