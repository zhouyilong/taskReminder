import { describe, expect, it } from "vitest";
import { isTypingTarget, resolveShortcut } from "./keyboard";

const key = (init: Partial<KeyboardEvent> & { target?: unknown }) => ({
  key: "",
  code: "",
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...init
}) as Parameters<typeof resolveShortcut>[0];

const fakeElement = (matches: boolean) => ({ closest: () => (matches ? {} : null) }) as unknown as EventTarget;

describe("resolveShortcut", () => {
  it("maps Ctrl/⌘ combinations", () => {
    expect(resolveShortcut(key({ key: "n", ctrlKey: true }))).toEqual({ type: "new" });
    expect(resolveShortcut(key({ key: "N", metaKey: true }))).toEqual({ type: "new" });
    expect(resolveShortcut(key({ key: "f", ctrlKey: true }))).toEqual({ type: "search" });
    expect(resolveShortcut(key({ key: "k", ctrlKey: true }))).toEqual({ type: "palette" });
    expect(resolveShortcut(key({ key: "K", metaKey: true }))).toEqual({ type: "palette" });
    expect(resolveShortcut(key({ key: "n", ctrlKey: true, shiftKey: true }))).toBeNull();
    expect(resolveShortcut(key({ key: "s", ctrlKey: true }))).toBeNull();
  });

  it("switches tabs with Alt + digit", () => {
    expect(resolveShortcut(key({ key: "1", code: "Digit1", altKey: true }))).toEqual({ type: "tab", tab: "today" });
    expect(resolveShortcut(key({ key: "¡", code: "Digit3", altKey: true }))).toEqual({ type: "tab", tab: "tasks" });
    expect(resolveShortcut(key({ key: "9", code: "Digit9", altKey: true }))).toEqual({ type: "tab", tab: "trash" });
    expect(resolveShortcut(key({ key: "0", code: "Digit0", altKey: true }))).toBeNull();
  });

  it("handles Escape and Delete, ignoring Delete while typing or composing", () => {
    expect(resolveShortcut(key({ key: "Escape" }))).toEqual({ type: "escape" });
    expect(resolveShortcut(key({ key: "Delete", target: fakeElement(false) }))).toEqual({ type: "delete" });
    expect(resolveShortcut(key({ key: "Delete", target: fakeElement(true) }))).toBeNull();
    expect(resolveShortcut(key({ key: "Escape", isComposing: true } as never))).toBeNull();
    expect(resolveShortcut(key({ key: "a" }))).toBeNull();
  });
});

describe("isTypingTarget", () => {
  it("detects editable targets", () => {
    expect(isTypingTarget(fakeElement(true))).toBe(true);
    expect(isTypingTarget(fakeElement(false))).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
  });
});
