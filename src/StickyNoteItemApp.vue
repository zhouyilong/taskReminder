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
          @input="handleTitleInput"
        />
        <button class="paper-note-close" type="button" @click.stop="closeNote">×</button>
      </header>
      <textarea
        v-model="note.content"
        class="paper-note-editor"
        placeholder="在这里写下便签内容..."
        @input="handleContentInput"
      />
      <div class="paper-note-footer">{{ saveHint }}</div>
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
import type { StickyNote } from "./types";

const note = ref<StickyNote | null>(null);
const saveHint = ref("自动保存");
const windowRef = getCurrentWindow();
const saveTimers = {
  title: 0,
  content: 0
};
let storageHandler: ((event: StorageEvent) => void) | null = null;
let unlistenRefresh: UnlistenFn | null = null;

const applyThemeFromStorage = () => {
  const useLight = safeStorage.getItem("appTheme") === "light";
  document.documentElement.classList.toggle("light-theme", useLight);
  document.body.classList.toggle("light-theme", useLight);
};

const scheduleTitleSave = () => {
  if (!note.value) {
    return;
  }
  if (saveTimers.title) {
    window.clearTimeout(saveTimers.title);
  }
  saveTimers.title = window.setTimeout(async () => {
    if (!note.value) {
      return;
    }
    try {
      await api.updateStickyNoteTitle({
        taskId: note.value.taskId,
        title: note.value.title
      });
    } catch (error) {
      console.error("[sticky-note-item] 标题保存失败", error);
    }
  }, 400);
};

const scheduleContentSave = () => {
  if (!note.value) {
    return;
  }
  if (saveTimers.content) {
    window.clearTimeout(saveTimers.content);
  }
  saveHint.value = "保存中...";
  saveTimers.content = window.setTimeout(async () => {
    if (!note.value) {
      return;
    }
    try {
      await api.saveStickyNoteContent({
        taskId: note.value.taskId,
        content: note.value.content
      });
      saveHint.value = `已保存 ${new Date().toLocaleTimeString("zh-CN", { hour12: false })}`;
    } catch (error) {
      console.error("[sticky-note-item] 内容保存失败", error);
      saveHint.value = "保存失败";
    }
  }, 450);
};

const handleTitleInput = () => {
  scheduleTitleSave();
};

const handleContentInput = () => {
  scheduleContentSave();
};

const closeNote = async () => {
  if (!note.value) {
    await windowRef.hide();
    return;
  }
  await api.closeStickyNote(note.value.taskId);
};

const loadCurrentNote = async () => {
  const row = await api.getStickyNoteByWindowLabel(windowRef.label);
  note.value = row;
};

onMounted(async () => {
  applyThemeFromStorage();
  await loadCurrentNote();
  unlistenRefresh = await listen<StickyNote>("sticky-note-item-refresh", event => {
    note.value = event.payload;
    saveHint.value = "自动保存";
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
  if (saveTimers.title) {
    window.clearTimeout(saveTimers.title);
  }
  if (saveTimers.content) {
    window.clearTimeout(saveTimers.content);
  }
});
</script>
