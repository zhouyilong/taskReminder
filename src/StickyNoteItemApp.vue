<template>
  <div class="sticky-note-item-root" :style="rootStyle">
    <article v-if="note" class="paper-note paper-note-window">
      <!-- Invisible resize borders -->
      <div class="resize-edge resize-top" @mousedown.stop.prevent="startResize('North')"></div>
      <div class="resize-edge resize-right" @mousedown.stop.prevent="startResize('East')"></div>
      <div class="resize-edge resize-bottom" @mousedown.stop.prevent="startResize('South')"></div>
      <div class="resize-edge resize-left" @mousedown.stop.prevent="startResize('West')"></div>
      <div class="resize-edge resize-top-left" @mousedown.stop.prevent="startResize('NorthWest')"></div>
      <div class="resize-edge resize-top-right" @mousedown.stop.prevent="startResize('NorthEast')"></div>
      <div class="resize-edge resize-bottom-left" @mousedown.stop.prevent="startResize('SouthWest')"></div>
      <div class="resize-edge resize-bottom-right" @mousedown.stop.prevent="startResize('SouthEast')"></div>

      <header
        class="paper-note-header"
        :data-tauri-drag-region="isPinned ? null : ''"
        :class="{ 'paper-note-header-pinned': isPinned }"
      >
        <input
          v-model="note.title"
          class="paper-note-title-input"
          type="text"
          placeholder="便签标题"
          :title="note.title && note.title.trim() ? note.title : '便签'"
          @mousedown.stop
          @input="handleTitleInput"
        />
        <div class="paper-note-actions">
          <button
            class="paper-note-action pin"
            :class="{ active: isPinned }"
            type="button"
            :title="isPinned ? '取消锚定（取消置顶并恢复可移动）' : '锚定便签（置顶并锁定位置）'"
            @mousedown.stop.prevent
            @click.stop="handleTogglePin"
          >
            <svg
              viewBox="0 0 1024 1024"
              aria-hidden="true"
              class="pin-icon-pencil"
            >
              <path
                d="M628.992 46.208a32 32 0 0 1 22.656 9.344l316.8 316.8a32 32 0 0 1 0 45.248c-30.72 30.72-68.608 37.632-96.192 37.632-11.328 0-21.44-1.152-29.44-2.496l-200.576 200.576a379.328 379.328 0 0 1 10.24 64.832c2.944 44.928-2.048 107.968-46.08 152a32 32 0 0 1-45.248 0l-181.056-180.992-203.648 203.648c-12.48 12.48-78.016 57.728-90.496 45.248-12.48-12.48 32.768-78.08 45.248-90.496l203.648-203.648-180.992-181.056a32 32 0 0 1 0-45.248c44.032-44.032 107.072-49.088 152-46.08a379.008 379.008 0 0 1 64.832 10.24l200.576-200.512a177.408 177.408 0 0 1-2.56-29.504c0-27.52 6.912-65.408 37.696-96.192a32 32 0 0 1 22.592-9.344z m7.808 135.168v-0.128 0.128z m0-0.128v0.128a32 32 0 0 1-7.808 32.64L402.752 440.192a32 32 0 0 1-32.704 7.68H369.92l-0.896-0.256a288.448 288.448 0 0 0-18.432-4.864 315.008 315.008 0 0 0-48.96-7.424c-27.008-1.792-53.504 0.512-75.2 9.6l352.64 352.576c9.024-21.76 11.328-48.192 9.536-75.2a315.136 315.136 0 0 0-12.288-67.456l-0.256-0.832v-0.064a32 32 0 0 1 7.68-32.768l226.304-226.24a32 32 0 0 1 34.048-7.36l6.144 1.408c5.568 1.088 13.312 2.176 22.016 2.176 7.296 0 14.72-0.704 21.952-2.56L635.328 129.792c-1.856 7.232-2.56 14.72-2.56 21.952a113.856 113.856 0 0 0 3.968 29.44z"
                fill="currentColor"
              ></path>
            </svg>
          </button>
          <button
            class="paper-note-action"
            type="button"
            title="新增便签"
            @mousedown.stop.prevent
            @click.stop="createSiblingNote"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" stroke="currentColor" fill="none" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
          </button>
          <button
            class="paper-note-action complete"
            type="button"
            title="标记完成并关闭"
            @mousedown.stop.prevent
            @click.stop="completeAndCloseNote"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" stroke="currentColor" fill="none" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
          </button>
          <button
            class="paper-note-close"
            type="button"
            title="关闭"
            @mousedown.stop.prevent
            @click.stop="closeNote"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" stroke="currentColor" fill="none" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </header>
      <MarkdownNoteEditor
        v-model="note.content"
        class="paper-note-editor paper-note-editor-host"
        variant="ghost"
        :theme="editorTheme"
        placeholder="在这里写下便签内容..."
        @update:modelValue="handleContentInput"
      />
      <footer class="paper-note-footer">
        <span class="paper-note-time">{{ formattedCreatedAt }}</span>
        <div class="paper-note-footer-right">
          <span class="paper-note-save-hint">
            <template v-if="saveCountdownSeconds > 0">
              <span class="paper-note-save-countdown">{{ saveCountdownSeconds }}</span>秒后保存
            </template>
            <template v-else-if="saveHint">{{ saveHint }}</template>
          </span>
        </div>
      </footer>
    </article>
    <div v-else class="sticky-item-loading">载入便签...</div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow, type ResizeDirection } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import MarkdownNoteEditor from "./components/MarkdownNoteEditor.vue";
import { safeStorage } from "./safeStorage";
import { api } from "./api";
import type { AppSettings, StickyNote, UiStatePayload } from "./types";

const note = ref<StickyNote | null>(null);
const saveHint = ref("");
const saveCountdownSeconds = ref(0);
const isPinned = ref(false);
const editorTheme = ref<"light" | "dark">(safeStorage.getItem("appTheme") === "light" ? "light" : "dark");
const windowRef = getCurrentWindow();
const refreshEventName = `sticky-note-item-refresh-${windowRef.label.replace(/^sticky-note-item-/, "")}`;
const AUTO_SAVE_IDLE_MS = 5000;
const AUTO_SAVE_IDLE_SECONDS = AUTO_SAVE_IDLE_MS / 1000;
let saveTimer = 0;
let saveCountdownTimer = 0;
let lastSavedTitle = "";
let lastSavedContent = "";
const MIN_STICKY_OPACITY = 0.35;
const MAX_STICKY_OPACITY = 1;
const DEFAULT_STICKY_OPACITY = 0.95;
const MIN_UI_SCALE = 0.8;
const MAX_UI_SCALE = 1.2;
const DEFAULT_UI_SCALE = 1;
const MIN_WINDOW_OPACITY = 0.3;
const MAX_WINDOW_OPACITY = 1.0;
const DEFAULT_WINDOW_OPACITY = 1.0;
let storageHandler: ((event: StorageEvent) => void) | null = null;
let unlistenRefresh: UnlistenFn | null = null;
let unlistenSettingsUpdated: UnlistenFn | null = null;
let unlistenSettingsUpdatedGeneric: UnlistenFn | null = null;
let unlistenUiStateChanged: UnlistenFn | null = null;
let unlistenUiStateChangedWindow: UnlistenFn | null = null;
let unlistenThemeChangedLegacy: UnlistenFn | null = null;
let unlistenScaleChangedLegacy: UnlistenFn | null = null;
let unlistenWindowOpacityChangedLegacy: UnlistenFn | null = null;
let windowUiStateHandler: ((event: Event) => void) | null = null;
let uiStatePollInterval: number = 0;
// Tracks last-applied state key for change detection in the poll loop.
let lastUiStateKey = "";

const formattedCreatedAt = computed(() => {
  if (!note.value?.createdAt) return "";
  try {
    const date = new Date(note.value.createdAt);
    return date.toLocaleString("zh-CN", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      hour12: false
    });
  } catch (e) {
    return "";
  }
});

const applyTheme = (useLight: boolean) => {
  document.documentElement.classList.toggle("light-theme", useLight);
  document.body.classList.toggle("light-theme", useLight);
  editorTheme.value = useLight ? "light" : "dark";
};

const applyThemeFromStorage = () => {
  applyTheme(safeStorage.getItem("appTheme") === "light");
};

const normalizeStickyOpacity = (value: number) => {
  if (!Number.isFinite(value)) {
    return DEFAULT_STICKY_OPACITY;
  }
  return Math.min(MAX_STICKY_OPACITY, Math.max(MIN_STICKY_OPACITY, value));
};

const applyStickyOpacity = (value: number) => {
  const normalized = normalizeStickyOpacity(value);
  document.documentElement.style.setProperty("--sticky-note-opacity", normalized.toFixed(2));
};

const normalizeUiScale = (value: number): number => {
  if (!Number.isFinite(value)) return DEFAULT_UI_SCALE;
  return Math.min(MAX_UI_SCALE, Math.max(MIN_UI_SCALE, value));
};

const uiScale = ref(DEFAULT_UI_SCALE);

const applyUiScale = (value: number) => {
  uiScale.value = normalizeUiScale(value);
};

const rootStyle = computed(() => {
  const scale = uiScale.value || DEFAULT_UI_SCALE;
  return {
    width: "100%",
    height: "100%",
    "--sticky-ui-scale": `${scale}`,
    opacity: windowOpacity.value
  };
});

const normalizeWindowOpacity = (value: number): number => {
  if (!Number.isFinite(value)) return DEFAULT_WINDOW_OPACITY;
  return Math.min(MAX_WINDOW_OPACITY, Math.max(MIN_WINDOW_OPACITY, value));
};

const windowOpacity = ref(DEFAULT_WINDOW_OPACITY);

const applyWindowOpacity = (value: number) => {
  windowOpacity.value = normalizeWindowOpacity(value);
};

const normalizeTitle = (title: string) => {
  const resolved = title.trim();
  return resolved ? resolved : "便签";
};

const syncPinnedState = async () => {
  try {
    isPinned.value = await api.getStickyNotePinnedByWindowLabel(windowRef.label);
  } catch (error) {
    console.warn("[sticky-note-item] 读取锚定状态失败", error);
    try {
      isPinned.value = await windowRef.isAlwaysOnTop();
    } catch {
      isPinned.value = false;
    }
  }
};

const runWindowAction = async (label: string, action: () => Promise<void>) => {
  try {
    await action();
    return true;
  } catch (error) {
    console.warn(`[sticky-note-item] ${label}失败`, error);
    return false;
  }
};

const togglePin = async () => {
  const nextPinned = !isPinned.value;
  if (nextPinned) {
    await runWindowAction("取消底层", () => windowRef.setAlwaysOnBottom(false));
    await runWindowAction("开启置顶", () => windowRef.setAlwaysOnTop(true));
    // Focus failure should not block pin state sync.
    await runWindowAction("激活窗口", () => windowRef.setFocus());
  } else {
    await runWindowAction("关闭置顶", () => windowRef.setAlwaysOnTop(false));
    await runWindowAction("恢复底层", () => windowRef.setAlwaysOnBottom(true));
  }
  await syncPinnedState();
};

const clearSaveTimer = () => {
  if (saveTimer) {
    window.clearTimeout(saveTimer);
    saveTimer = 0;
  }
};

const clearSaveCountdownTimer = () => {
  if (saveCountdownTimer) {
    window.clearInterval(saveCountdownTimer);
    saveCountdownTimer = 0;
  }
  saveCountdownSeconds.value = 0;
};

const clearAutoSaveTrackers = () => {
  clearSaveTimer();
  clearSaveCountdownTimer();
};

const flushPendingSave = async (force = false) => {
  if (!note.value) {
    return;
  }

  if (!force && saveTimer) {
    window.clearTimeout(saveTimer);
    saveTimer = 0;
  }
  clearSaveCountdownTimer();

  const nextTitle = normalizeTitle(note.value.title);
  const nextContent = note.value.content;
  const titleChanged = nextTitle !== lastSavedTitle;
  const contentChanged = nextContent !== lastSavedContent;

  if (!titleChanged && !contentChanged) {
    saveHint.value = "";
    return;
  }

  saveHint.value = "保存中...";
  note.value.title = nextTitle;
  try {
    if (titleChanged) {
      await api.updateStickyNoteTitle({
        taskId: note.value.taskId,
        title: nextTitle
      });
    }
    if (contentChanged) {
      await api.saveStickyNoteContent({
        taskId: note.value.taskId,
        content: nextContent
      });
    }
    lastSavedTitle = nextTitle;
    lastSavedContent = nextContent;
    saveHint.value = `已保存 ${new Date().toLocaleTimeString("zh-CN", { hour12: false })}`;
    // Clear the success hint after a few seconds so it returns to blank
    setTimeout(() => {
      if (saveHint.value.startsWith("已保存")) {
        saveHint.value = "";
      }
    }, 4000);
  } catch (error) {
    console.error("[sticky-note-item] 自动保存失败", error);
    saveHint.value = "保存失败";
  }
};

const scheduleAutoSave = () => {
  if (!note.value) {
    return;
  }
  clearAutoSaveTrackers();
  saveCountdownSeconds.value = AUTO_SAVE_IDLE_SECONDS;
  saveCountdownTimer = window.setInterval(() => {
    if (saveCountdownSeconds.value > 1) {
      saveCountdownSeconds.value -= 1;
    }
  }, 1000);
  saveTimer = window.setTimeout(() => {
    void flushPendingSave();
  }, AUTO_SAVE_IDLE_MS);
};

const handleTitleInput = () => {
  if (!note.value) {
    return;
  }
  scheduleAutoSave();
};

const handleContentInput = () => {
  scheduleAutoSave();
};

const closeNote = async () => {
  clearAutoSaveTrackers();
  await flushPendingSave(true);
  try {
    await windowRef.hide();
  } catch (error) {
    console.error("[sticky-note-item] 本地隐藏失败", error);
  }
  try {
    await api.closeStickyNoteByWindowLabel(windowRef.label);
  } catch (error) {
    console.error("[sticky-note-item] 关闭便签失败", error);
  }
};

const startResize = async (edge: ResizeDirection) => {
  try {
    await windowRef.startResizeDragging(edge);
  } catch (error) {
    console.error("[sticky-note-item] 调整大小失败", error);
  }
};

const applyUiState = (payload: UiStatePayload) => {
  applyUiScale(payload.uiScale);
  applyTheme(payload.theme === "light");
  applyWindowOpacity(payload.windowOpacity);
};

const rememberUiState = (payload: UiStatePayload) => {
  applyUiState(payload);
  lastUiStateKey = `${payload.uiScale}:${payload.theme}:${payload.windowOpacity}`;
};

const resolveUiStatePayload = (value: Partial<UiStatePayload> | null | undefined): UiStatePayload | null => {
  if (!value || typeof value !== "object") {
    return null;
  }
  return {
    uiScale: Number(value.uiScale ?? DEFAULT_UI_SCALE),
    theme: value.theme === "light" ? "light" : "dark",
    windowOpacity: Number(value.windowOpacity ?? DEFAULT_WINDOW_OPACITY)
  };
};

const readInjectedUiState = (): UiStatePayload | null => {
  const candidate = (window as Window & { __TASKREMINDER_UI_STATE?: Partial<UiStatePayload> }).__TASKREMINDER_UI_STATE;
  return resolveUiStatePayload(candidate);
};

const handleTogglePin = async () => {
  const nextPinned = !isPinned.value;
  try {
    const applied = await api.setStickyNotePinnedByWindowLabel(windowRef.label, nextPinned);
    isPinned.value = applied;
    if (applied) {
      await windowRef.setFocus();
    }
  } catch (error) {
    console.warn("[sticky-note-item] 切换锚定状态失败", error);
    return;
  }
  await syncPinnedState();
};

const resolveCurrentLogicalPosition = async () => {
  try {
    const [scaleFactor, position] = await Promise.all([
      windowRef.scaleFactor(),
      windowRef.outerPosition()
    ]);
    return {
      x: position.x / scaleFactor,
      y: position.y / scaleFactor
    };
  } catch (error) {
    console.warn("[sticky-note-item] 读取当前窗口位置失败", error);
    return null;
  }
};

const createSiblingNote = async () => {
  const position = await resolveCurrentLogicalPosition();
  const payload: { defaultX?: number; defaultY?: number } = {};
  if (position) {
    payload.defaultX = Math.max(0, position.x + 26);
    payload.defaultY = Math.max(0, position.y - 22);
  }
  try {
    await api.createStickyNote(payload);
  } catch (error) {
    console.error("[sticky-note-item] 新增便签失败", error);
    const message = error instanceof Error ? error.message : String(error);
    alert(`新增便签失败：${message}`);
  }
};

const completeAndCloseNote = async () => {
  if (!note.value) {
    return;
  }
  const taskId = note.value.taskId;
  clearAutoSaveTrackers();
  await flushPendingSave(true);
  try {
    await api.completeTask(taskId);
  } catch (error) {
    console.error("[sticky-note-item] 标记完成失败", error);
    const message = error instanceof Error ? error.message : String(error);
    alert(`标记完成失败：${message}`);
    return;
  }
  try {
    await api.closeStickyNote(taskId);
  } catch (error) {
    console.error("[sticky-note-item] 关闭已完成便签失败", error);
    await closeNote();
  }
};

const loadCurrentNote = async () => {
  const row = await api.getStickyNoteByWindowLabel(windowRef.label);
  note.value = row;
  if (row) {
    lastSavedTitle = normalizeTitle(row.title);
    lastSavedContent = row.content;
  } else {
    lastSavedTitle = "";
    lastSavedContent = "";
  }
  clearSaveCountdownTimer();
  saveHint.value = "";
};

const loadCurrentNoteWithRetry = async () => {
  await loadCurrentNote();
  if (note.value) {
    return;
  }
  await new Promise(resolve => {
    window.setTimeout(resolve, 120);
  });
  await loadCurrentNote();
};

onMounted(async () => {
  applyThemeFromStorage();
  applyUiScale(Number(safeStorage.getItem("uiScale") ?? DEFAULT_UI_SCALE));
  applyWindowOpacity(Number(safeStorage.getItem("windowOpacity") ?? DEFAULT_WINDOW_OPACITY));
  const injectedUiState = readInjectedUiState();
  if (injectedUiState) {
    rememberUiState(injectedUiState);
  }
  windowUiStateHandler = event => {
    const payload = event instanceof CustomEvent ? resolveUiStatePayload(event.detail as Partial<UiStatePayload>) : null;
    if (payload) {
      rememberUiState(payload);
    }
  };
  window.addEventListener("taskreminder-ui-state", windowUiStateHandler as EventListener);
  await syncPinnedState();
  try {
    await loadCurrentNoteWithRetry();
  } catch (error) {
    console.error("[sticky-note-item] 读取便签失败", error);
  }
  try {
    const settings = await api.getSettings();
    applyStickyOpacity(settings.stickyNoteOpacity ?? DEFAULT_STICKY_OPACITY);
    applyWindowOpacity(settings.windowOpacity ?? DEFAULT_WINDOW_OPACITY);
  } catch (error) {
    console.error("[sticky-note-item] 读取透明度设置失败", error);
    applyStickyOpacity(DEFAULT_STICKY_OPACITY);
    applyWindowOpacity(DEFAULT_WINDOW_OPACITY);
  }
  unlistenRefresh = await listen<StickyNote>(refreshEventName, event => {
    clearAutoSaveTrackers();
    note.value = event.payload;
    lastSavedTitle = normalizeTitle(event.payload.title);
    lastSavedContent = event.payload.content;
    saveHint.value = "";
  });
  unlistenSettingsUpdated = await listen<AppSettings>("sticky-note-settings-updated", event => {
    applyStickyOpacity(event.payload.stickyNoteOpacity ?? DEFAULT_STICKY_OPACITY);
  });
  // Also listen for the generic settings-updated event (used for windowOpacity sync)
  unlistenSettingsUpdatedGeneric = await listen<AppSettings>("settings-updated", event => {
    applyStickyOpacity(event.payload.stickyNoteOpacity ?? DEFAULT_STICKY_OPACITY);
    applyWindowOpacity(event.payload.windowOpacity ?? DEFAULT_WINDOW_OPACITY);
  });
  try {
    unlistenUiStateChanged = await listen<UiStatePayload>("ui-state-changed", event => {
      rememberUiState(event.payload);
    });
  } catch (error) {
    console.warn("[sticky-note-item] 监听 UI 状态变化失败", error);
  }
  try {
    unlistenUiStateChangedWindow = await windowRef.listen<UiStatePayload>("ui-state-changed", event => {
      rememberUiState(event.payload);
    });
  } catch (error) {
    console.warn("[sticky-note-item] 监听窗口 UI 状态变化失败", error);
  }
  try {
    unlistenThemeChangedLegacy = await listen<string>("app-theme-updated", event => {
      applyTheme(event.payload === "light");
      lastUiStateKey = "";
    });
  } catch (error) {
    console.warn("[sticky-note-item] 监听主题变更失败", error);
  }
  try {
    unlistenScaleChangedLegacy = await listen<number>("ui-scale-changed", event => {
      applyUiScale(event.payload);
      lastUiStateKey = "";
    });
  } catch (error) {
    console.warn("[sticky-note-item] 监听缩放变更失败", error);
  }
  try {
    unlistenWindowOpacityChangedLegacy = await listen<number>("window-opacity-changed", event => {
      applyWindowOpacity(event.payload);
      lastUiStateKey = "";
    });
  } catch (error) {
    console.warn("[sticky-note-item] 监听透明度变更失败", error);
  }
  try {
    const uiState = await api.getUiState();
    if (uiState) {
      rememberUiState(uiState);
    }
  } catch (error) {
    console.warn("[sticky-note-item] 读取 UI 状态快照失败", error);
  }
  // Poll every 2 s as a reliable fallback in case the push event is missed.
  uiStatePollInterval = window.setInterval(async () => {
    try {
      const uiState = await api.getUiState();
      if (!uiState) return;
      const key = `${uiState.uiScale}:${uiState.theme}:${uiState.windowOpacity}`;
      if (key !== lastUiStateKey) {
        rememberUiState(uiState);
      }
    } catch {
      // ignore poll errors silently
    }
  }, 2000);
  storageHandler = event => {
    if (event.key === "appTheme") {
      applyThemeFromStorage();
    }
    if (event.key === "uiScale") {
      applyUiScale(Number(event.newValue ?? DEFAULT_UI_SCALE));
    }
    if (event.key === "windowOpacity") {
      applyWindowOpacity(Number(event.newValue ?? DEFAULT_WINDOW_OPACITY));
    }
  };
  window.addEventListener("storage", storageHandler);
});

onBeforeUnmount(() => {
  if (uiStatePollInterval) {
    window.clearInterval(uiStatePollInterval);
    uiStatePollInterval = 0;
  }
  if (storageHandler) {
    window.removeEventListener("storage", storageHandler);
  }
  if (windowUiStateHandler) {
    window.removeEventListener("taskreminder-ui-state", windowUiStateHandler as EventListener);
  }
  if (unlistenRefresh) {
    unlistenRefresh();
  }
  if (unlistenSettingsUpdated) {
    unlistenSettingsUpdated();
  }
  if (unlistenSettingsUpdatedGeneric) {
    unlistenSettingsUpdatedGeneric();
  }
  if (unlistenUiStateChanged) {
    unlistenUiStateChanged();
  }
  if (unlistenUiStateChangedWindow) {
    unlistenUiStateChangedWindow();
  }
  if (unlistenThemeChangedLegacy) {
    unlistenThemeChangedLegacy();
  }
  if (unlistenScaleChangedLegacy) {
    unlistenScaleChangedLegacy();
  }
  if (unlistenWindowOpacityChangedLegacy) {
    unlistenWindowOpacityChangedLegacy();
  }
  clearAutoSaveTrackers();
});
</script>



