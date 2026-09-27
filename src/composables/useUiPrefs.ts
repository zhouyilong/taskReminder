// 主题、界面缩放、整体透明度与侧边栏状态（模块级单例），变化时广播给便签等其他窗口。
import { ref, watch } from "vue";
import { emit, emitTo } from "@tauri-apps/api/event";
import { getAllWebviewWindows } from "@tauri-apps/api/webviewWindow";
import { api } from "../api";
import { safeStorage } from "../safeStorage";
import type { UiStatePayload } from "../types";

const STICKY_NOTE_ITEM_PREFIX = "sticky-note-item-";

const isLightTheme = ref(safeStorage.getItem("appTheme") === "light");
const uiScale = ref(Number(safeStorage.getItem("uiScale") ?? "1"));
const windowOpacity = ref(Number(safeStorage.getItem("windowOpacity") ?? "1"));
const isSidebarCollapsed = ref(safeStorage.getItem("sidebarCollapsed") === "1");

const emitCurrentUiState = async () => {
  const payload: UiStatePayload = {
    uiScale: uiScale.value,
    theme: isLightTheme.value ? "light" : "dark",
    windowOpacity: windowOpacity.value
  };
  try {
    const stickyWindows = (await getAllWebviewWindows()).filter(window => window.label.startsWith(STICKY_NOTE_ITEM_PREFIX));
    await Promise.allSettled([
      emit("ui-state-changed", payload),
      emit("app-theme-updated", payload.theme),
      emit("ui-scale-changed", payload.uiScale),
      emit("window-opacity-changed", payload.windowOpacity),
      ...stickyWindows.map(window => emitTo(window.label, "ui-state-changed", payload)),
      ...stickyWindows.map(window => emitTo(window.label, "app-theme-updated", payload.theme)),
      ...stickyWindows.map(window => emitTo(window.label, "ui-scale-changed", payload.uiScale)),
      ...stickyWindows.map(window => emitTo(window.label, "window-opacity-changed", payload.windowOpacity))
    ]);
  } catch (error) {
    console.error("[main] 前端同步 UI 状态失败", error);
  }
  try {
    await api.emitUiStateChanged(payload.uiScale, payload.theme, payload.windowOpacity);
  } catch (error) {
    console.error("[main] 广播 UI 状态失败", error);
  }
};

const toggleTheme = () => {
  isLightTheme.value = !isLightTheme.value;
  safeStorage.setItem("appTheme", isLightTheme.value ? "light" : "dark");
  void emitCurrentUiState();
};

const toggleSidebar = () => {
  isSidebarCollapsed.value = !isSidebarCollapsed.value;
  safeStorage.setItem("sidebarCollapsed", isSidebarCollapsed.value ? "1" : "0");
};

watch(uiScale, value => {
  const normalized = Math.min(1.2, Math.max(0.8, Number(value) || 1));
  if (normalized !== value) {
    uiScale.value = normalized;
    return;
  }
  safeStorage.setItem("uiScale", normalized.toString());
  void emitCurrentUiState();
});

watch(windowOpacity, value => {
  const normalized = Math.min(1, Math.max(0.3, Number(value) || 1));
  if (normalized !== value) {
    windowOpacity.value = normalized;
    return;
  }
  safeStorage.setItem("windowOpacity", normalized.toString());
  void emitCurrentUiState();
});

export const useUiPrefs = () => ({
  isLightTheme,
  uiScale,
  windowOpacity,
  isSidebarCollapsed,
  emitCurrentUiState,
  toggleTheme,
  toggleSidebar,
});
