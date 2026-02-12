<template>
  <div class="sticky-board">
    <div class="sticky-topbar" data-tauri-drag-region>
      <div class="sticky-brand">桌面便签</div>
      <div class="sticky-topbar-actions">
        <button class="sticky-ghost-button" type="button" @click="refreshData">刷新</button>
        <button class="sticky-close-window" type="button" @click="closeWindow">关闭</button>
      </div>
    </div>

    <div class="sticky-layout sticky-manager-layout">
      <aside class="sticky-task-panel">
        <div class="sticky-list-toolbar">
          <div class="sticky-task-title">便签列表</div>
          <button class="sticky-add-note" type="button" @click="createCustomNote">+ 新便签</button>
        </div>
        <input
          v-model="taskKeyword"
          class="sticky-task-search"
          placeholder="搜索..."
        />
        <div class="sticky-task-list compact">
          <div
            v-for="entry in filteredEntries"
            :key="entry.id"
            class="sticky-task-row"
            :class="{ active: isNoteOpen(entry.id) }"
          >
            <button
              class="sticky-task-item compact"
              :class="{ active: isNoteOpen(entry.id) }"
              type="button"
              @click="openFromEntry(entry)"
            >
              <span class="sticky-item-dot" :class="entry.noteType === 'CUSTOM' ? 'custom' : 'task'"></span>
              <span class="sticky-task-text compact">{{ entry.title }}</span>
            </button>
            <button
              v-if="isNoteOpen(entry.id)"
              class="sticky-item-close"
              type="button"
              title="关闭便签"
              @click.stop="closeNote(entry.id)"
            >
              ×
            </button>
          </div>
          <div v-if="filteredEntries.length === 0" class="sticky-task-empty">暂无便签项</div>
        </div>
      </aside>

      <section class="sticky-canvas sticky-manager-hint">
        <div class="sticky-canvas-empty">
          点击左侧项目后，会在桌面打开独立便签窗口
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { safeStorage } from "./safeStorage";
import { api } from "./api";
import type { StickyNote, Task } from "./types";

type NoteListEntry = {
  id: string;
  title: string;
  noteType: "TASK" | "CUSTOM";
};

const NOTE_PADDING = 12;
const tasks = ref<Task[]>([]);
const allNotes = ref<StickyNote[]>([]);
const taskKeyword = ref("");
let storageHandler: ((event: StorageEvent) => void) | null = null;
let unlistenChanged: UnlistenFn | null = null;
let unlistenThemeChanged: UnlistenFn | null = null;

const noteEntries = computed<NoteListEntry[]>(() => {
  const noteTitleMap = new Map(allNotes.value.map(note => [note.taskId, note.title]));
  const taskEntries = tasks.value.map(task => ({
    id: task.id,
    title: noteTitleMap.get(task.id)?.trim() || task.description,
    noteType: "TASK" as const
  }));
  const customEntries = allNotes.value
    .filter(note => note.noteType === "CUSTOM")
    .map(note => ({
      id: note.taskId,
      title: note.title || "新便签",
      noteType: "CUSTOM" as const
    }));
  return [...taskEntries, ...customEntries];
});

const filteredEntries = computed(() => {
  const keyword = taskKeyword.value.trim().toLowerCase();
  if (!keyword) {
    return noteEntries.value;
  }
  return noteEntries.value.filter(entry => entry.title.toLowerCase().includes(keyword));
});

const applyTheme = (useLight: boolean) => {
  document.documentElement.classList.toggle("light-theme", useLight);
  document.body.classList.toggle("light-theme", useLight);
};

const applyThemeFromStorage = () => {
  applyTheme(safeStorage.getItem("appTheme") === "light");
};

const refreshData = async () => {
  const [taskRows, noteRows] = await Promise.all([
    api.listActiveTasks(),
    api.listStickyNotes()
  ]);
  tasks.value = taskRows;
  allNotes.value = noteRows;
};

const isNoteOpen = (noteId: string) => {
  return allNotes.value.some(note => note.taskId === noteId && note.isOpen);
};

const buildDefaultPosition = () => {
  const opened = allNotes.value.filter(note => note.isOpen).length;
  return {
    x: NOTE_PADDING + (opened % 9) * 28,
    y: NOTE_PADDING + (opened % 9) * 22
  };
};

const openTaskNote = async (task: Task) => {
  const position = buildDefaultPosition();
  await api.openStickyNote({
    taskId: task.id,
    title: task.description,
    defaultX: position.x,
    defaultY: position.y
  });
  await refreshData();
};

const openCustomNote = async (noteId: string, title: string) => {
  await api.openStickyNote({ taskId: noteId, title });
  await refreshData();
};

const openFromEntry = async (entry: NoteListEntry) => {
  if (entry.noteType === "CUSTOM") {
    await openCustomNote(entry.id, entry.title);
    return;
  }
  const task = tasks.value.find(item => item.id === entry.id);
  if (!task) {
    return;
  }
  await openTaskNote(task);
};

const createCustomNote = async () => {
  const position = buildDefaultPosition();
  await api.createStickyNote({
    title: `新便签 ${allNotes.value.filter(note => note.noteType === "CUSTOM").length + 1}`,
    defaultX: position.x,
    defaultY: position.y
  });
  await refreshData();
};

const closeNote = async (noteId: string) => {
  await api.closeStickyNote(noteId);
  await refreshData();
};

const closeWindow = async () => {
  await api.setStickyNoteWindowVisible(false);
};

onMounted(async () => {
  applyThemeFromStorage();
  await refreshData();
  storageHandler = event => {
    if (event.key === "appTheme") {
      applyThemeFromStorage();
    }
  };
  window.addEventListener("storage", storageHandler);
  unlistenChanged = await listen("sticky-note-changed", () => {
    void refreshData();
  });
  unlistenThemeChanged = await listen<string>("app-theme-updated", event => {
    applyTheme(event.payload === "light");
  });
});

onBeforeUnmount(() => {
  if (storageHandler) {
    window.removeEventListener("storage", storageHandler);
  }
  if (unlistenChanged) {
    unlistenChanged();
  }
  if (unlistenThemeChanged) {
    unlistenThemeChanged();
  }
});
</script>
