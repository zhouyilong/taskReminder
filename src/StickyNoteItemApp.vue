<template>
  <div class="sticky-note-item-root">
    <article v-if="note" class="paper-note paper-note-window">
      <div class="paper-pin"></div>
      <header class="paper-note-header" data-tauri-drag-region>
        <input
          v-model="note.title"
          class="paper-note-title-input"
          type="text"
          placeholder="便签标题"
          @mousedown.stop
          @input="handleTitleInput"
        />
        <div class="paper-note-actions">
          <button
            class="paper-note-action"
            type="button"
            title="新增便签"
            @mousedown.stop.prevent
            @click.stop="createSiblingNote"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            </svg>
          </button>
          <button
            class="paper-note-action complete"
            type="button"
            title="标记完成并关闭"
            @mousedown.stop.prevent
            @click.stop="completeAndCloseNote"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M6 12.5l4 4 8-8" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
        <button
          class="paper-note-close"
          type="button"
          @mousedown.stop.prevent
          @click.stop="closeNote"
        >
          ×
        </button>
        </div>
      </header>
      <textarea
        v-model="note.content"
        class="paper-note-editor"
        placeholder="在这里写下便签内容..."
        @mousedown.stop
        @input="handleContentInput"
      />
      <footer class="paper-note-footer">
        <span class="paper-note-save-hint">
          <template v-if="saveCountdownSeconds > 0">
            编辑中，<span class="paper-note-save-countdown">{{ saveCountdownSeconds }}</span>秒后自动保存
          </template>
          <template v-else>{{ saveHint }}</template>
        </span>
      </footer>
    </article>
    <div v-else class="sticky-item-loading">载入便签...</div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { safeStorage } from "./safeStorage";
import { api } from "./api";
import type { AppSettings, StickyNote } from "./types";

const note = ref<StickyNote | null>(null);
const saveHint = ref("自动保存");
const saveCountdownSeconds = ref(0);
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
let storageHandler: ((event: StorageEvent) => void) | null = null;
let unlistenRefresh: UnlistenFn | null = null;
let unlistenThemeChanged: UnlistenFn | null = null;
let unlistenSettingsUpdated: UnlistenFn | null = null;

const applyTheme = (useLight: boolean) => {
  document.documentElement.classList.toggle("light-theme", useLight);
  document.body.classList.toggle("light-theme", useLight);
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

const normalizeTitle = (title: string) => {
  const resolved = title.trim();
  return resolved ? resolved : "便签";
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
    saveHint.value = "自动保存";
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
  if (!note.value.content.trim()) {
    note.value.content = note.value.title;
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
    payload.defaultY = Math.max(0, position.y + 22);
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
  saveHint.value = "自动保存";
};

onMounted(async () => {
  applyThemeFromStorage();
  await loadCurrentNote();
  try {
    const settings = await api.getSettings();
    applyStickyOpacity(settings.stickyNoteOpacity ?? DEFAULT_STICKY_OPACITY);
  } catch (error) {
    console.error("[sticky-note-item] 读取透明度设置失败", error);
    applyStickyOpacity(DEFAULT_STICKY_OPACITY);
  }
  unlistenRefresh = await listen<StickyNote>(refreshEventName, event => {
    clearAutoSaveTrackers();
    note.value = event.payload;
    lastSavedTitle = normalizeTitle(event.payload.title);
    lastSavedContent = event.payload.content;
    saveHint.value = "自动保存";
  });
  unlistenSettingsUpdated = await listen<AppSettings>("sticky-note-settings-updated", event => {
    applyStickyOpacity(event.payload.stickyNoteOpacity ?? DEFAULT_STICKY_OPACITY);
  });
  unlistenThemeChanged = await listen<string>("app-theme-updated", event => {
    applyTheme(event.payload === "light");
  });
  storageHandler = event => {
    if (event.key === "appTheme") {
      applyThemeFromStorage();
    }
  };
  window.addEventListener("storage", storageHandler);
});

onBeforeUnmount(() => {
  if (storageHandler) {
    window.removeEventListener("storage", storageHandler);
  }
  if (unlistenRefresh) {
    unlistenRefresh();
  }
  if (unlistenThemeChanged) {
    unlistenThemeChanged();
  }
  if (unlistenSettingsUpdated) {
    unlistenSettingsUpdated();
  }
  clearAutoSaveTrackers();
});
</script>
