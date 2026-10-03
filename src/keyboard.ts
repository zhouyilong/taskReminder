// 主窗口快捷键：按键到动作的映射（纯函数），由 App.vue 的全局 keydown 监听执行。
import { TAB_KEYS, type TabKey } from "./navigation";

export type ShortcutAction =
  | { type: "tab"; tab: TabKey }
  | { type: "new" }
  | { type: "search" }
  | { type: "palette" }
  | { type: "escape" }
  | { type: "delete" };

type KeyLike = Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey"> & {
  target?: EventTarget | null;
  isComposing?: boolean;
};

/** 快捷键说明（设置中展示）。 */
export const SHORTCUT_HELP: Array<{ keys: string; label: string }> = [
  { keys: "Ctrl + N", label: "新建（聚焦当前页的新建输入框）" },
  { keys: "Ctrl + K", label: "全局搜索（待办、循环提醒、已办与便签内容）" },
  { keys: "Ctrl + F", label: "搜索（聚焦当前页的搜索框）" },
  { keys: "Alt + 1 … 9", label: "切换到侧边栏中的第 1 … 9 页" },
  { keys: "Esc", label: "关闭弹窗 / 退出多选" },
  { keys: "Delete", label: "删除多选中的待办（需确认）" }
];

/** 焦点在输入框、下拉框或编辑器中时，单键快捷键（Esc 以外）不生效。 */
export const isTypingTarget = (target: EventTarget | null | undefined) => {
  const element = target as HTMLElement | null | undefined;
  if (!element || typeof element.closest !== "function") return false;
  return Boolean(element.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]'));
};

const digitOf = (event: KeyLike) => {
  const match = /^Digit([1-9])$/.exec(event.code) ?? /^([1-9])$/.exec(event.key);
  return match ? Number(match[1]) : null;
};

export const resolveShortcut = (event: KeyLike): ShortcutAction | null => {
  if (event.isComposing) return null;
  const mod = event.ctrlKey || event.metaKey;
  const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
  if (mod && !event.altKey && !event.shiftKey) {
    if (key === "n") return { type: "new" };
    if (key === "f") return { type: "search" };
    if (key === "k") return { type: "palette" };
    return null;
  }
  if (event.altKey && !mod && !event.shiftKey) {
    const digit = digitOf(event);
    if (digit && digit <= TAB_KEYS.length) return { type: "tab", tab: TAB_KEYS[digit - 1] };
    return null;
  }
  if (mod || event.altKey) return null;
  if (key === "Escape") return { type: "escape" };
  if (key === "Delete" && !isTypingTarget(event.target)) return { type: "delete" };
  return null;
};

/** 视图之间传递快捷键：App 发出，当前视图按需处理（如待办列表的退出多选、删除选中）。 */
export const VIEW_SHORTCUT_EVENT = "app-view-shortcut";
