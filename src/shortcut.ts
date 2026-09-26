// 全局快捷键（Tauri 加速键格式，如 "CommandOrControl+Alt+N"）的录制与展示。

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph"]);

const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);

/** 把键盘事件转换为加速键；只按了修饰键或缺少修饰键时返回 null。 */
export const acceleratorFromEvent = (event: KeyboardEvent): string | null => {
  if (MODIFIER_KEYS.has(event.key)) {
    return null;
  }
  const modifiers: string[] = [];
  if (event.ctrlKey || event.metaKey) {
    modifiers.push("CommandOrControl");
  }
  if (event.altKey) {
    modifiers.push("Alt");
  }
  if (event.shiftKey) {
    modifiers.push("Shift");
  }
  const key = keyFromCode(event.code);
  if (!key) {
    return null;
  }
  // 全局快捷键至少需要 Ctrl/Alt 之一，避免占用普通输入按键（功能键除外）。
  const isFunctionKey = /^F\d{1,2}$/.test(key);
  if (!isFunctionKey && !modifiers.some(item => item === "CommandOrControl" || item === "Alt")) {
    return null;
  }
  return [...modifiers, key].join("+");
};

const keyFromCode = (code: string): string | null => {
  if (/^Key[A-Z]$/.test(code)) {
    return code.slice(3);
  }
  if (/^Digit\d$/.test(code)) {
    return code.slice(5);
  }
  if (/^F\d{1,2}$/.test(code)) {
    return code;
  }
  const named: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Backquote: "`",
    Minus: "-",
    Equal: "=",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Semicolon: ";",
    Quote: "'",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right"
  };
  return named[code] ?? null;
};

/** 将加速键格式化为界面展示文本，如 "Ctrl + Alt + N"。 */
export const formatAccelerator = (accelerator: string | null | undefined) => {
  if (!accelerator) {
    return "未设置";
  }
  return accelerator
    .split("+")
    .map(part => {
      if (part === "CommandOrControl" || part === "CmdOrCtrl") {
        return isMac ? "⌘" : "Ctrl";
      }
      return part;
    })
    .join(" + ");
};
