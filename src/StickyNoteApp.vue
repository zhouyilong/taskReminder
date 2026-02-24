<template>
  <div class="sticky-board" :data-tauri-drag-region="props.embedded ? undefined : 'false'">
    <div class="sticky-topbar" :data-tauri-drag-region="props.embedded ? undefined : ''">
      <div class="sticky-brand">桌面便签</div>
      <div class="sticky-topbar-actions">
        <button class="sticky-ghost-button" type="button" @click="refreshData">刷新</button>
        <button class="sticky-close-window" type="button" @click="closeWindow">关闭</button>
      </div>
    </div>

    <div class="sticky-layout sticky-manager-layout">
      <aside class="sticky-task-panel" :data-tauri-drag-region="props.embedded ? undefined : 'false'">
        <div class="sticky-list-toolbar">
          <div class="sticky-task-title">便签列表</div>
          <button
            class="sticky-add-note"
            type="button"
            :disabled="creatingNote"
            :data-tauri-drag-region="props.embedded ? undefined : 'false'"
            @click="addCustomNote"
          >
            {{ creatingNote ? "新增中..." : "新增便签" }}
          </button>
        </div>
        <input
          v-model="taskKeyword"
          class="sticky-task-search"
          placeholder="搜索..."
          :data-tauri-drag-region="props.embedded ? undefined : 'false'"
        />
        <div class="sticky-task-list-frame">
          <div class="sticky-task-list compact">
            <div
              v-for="entry in filteredEntries"
              :key="entry.id"
              class="sticky-task-row"
              :class="{ active: isNoteOpen(entry.id) }"
              :data-tauri-drag-region="props.embedded ? undefined : 'false'"
              @contextmenu.prevent.stop="openEntryContextMenu($event, entry)"
            >
              <button
                class="sticky-task-item compact"
                :class="{ active: isNoteOpen(entry.id) }"
                type="button"
                :data-tauri-drag-region="props.embedded ? undefined : 'false'"
                @click="openFromEntry(entry)"
              >
                <span class="sticky-item-dot task"></span>
                <span class="sticky-task-text compact">{{ entry.title }}</span>
              </button>
              <button
                v-if="isNoteOpen(entry.id)"
                class="sticky-item-close"
                type="button"
                title="关闭便签"
                :data-tauri-drag-region="props.embedded ? undefined : 'false'"
                @click.stop="closeNote(entry.id)"
              >
                ×
              </button>
            </div>
            <div v-if="filteredEntries.length === 0" class="sticky-task-empty">暂无便签项</div>
          </div>
        </div>
        <div
          v-if="entryContextMenu.visible"
          class="context-menu sticky-context-menu"
          :style="{ left: `${entryContextMenu.x}px`, top: `${entryContextMenu.y}px` }"
          :data-tauri-drag-region="props.embedded ? undefined : 'false'"
          @click.stop
        >
          <button class="context-menu-item" type="button" @click="markEntryCompleted">
            标记完成
          </button>
        </div>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { safeStorage } from "./safeStorage";
import { api } from "./api";
import type { StickyNote, Task } from "./types";

const props = withDefaults(defineProps<{ embedded?: boolean }>(), {
  embedded: false
});
const emitClose = defineEmits<{
  (event: "request-close"): void;
}>();

type NoteListEntry = {
  id: string;
  title: string;
};

const NOTE_PADDING = 12;
const tasks = ref<Task[]>([]);
const allNotes = ref<StickyNote[]>([]);
const taskKeyword = ref("");
const creatingNote = ref(false);
const entryContextMenu = ref({
  visible: false,
  x: 0,
  y: 0,
  taskId: null as string | null
});
let storageHandler: ((event: StorageEvent) => void) | null = null;
let unlistenChanged: UnlistenFn | null = null;
let unlistenThemeChanged: UnlistenFn | null = null;
let unlistenSettingsUpdated: UnlistenFn | null = null;

const noteEntries = computed<NoteListEntry[]>(() => {
  const taskEntries = tasks.value.map(task => ({
    id: task.id,
    title: task.description
  }));
  return taskEntries;
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

const withInvokeTimeout = async <T,>(
  promise: Promise<T>,
  timeoutMs: number,
  timeoutMessage: string
): Promise<T> => {
  let timer = 0;
  try {
    return await Promise.race([
      promise,
      new Promise<T>((_, reject) => {
        timer = window.setTimeout(() => {
          reject(new Error(timeoutMessage));
        }, timeoutMs);
      })
    ]);
  } finally {
    if (timer) {
      window.clearTimeout(timer);
    }
  }
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
  try {
    await withInvokeTimeout(
      api.openStickyNote({
        taskId: task.id,
        title: task.description,
        defaultX: position.x,
        defaultY: position.y
      }),
      5000,
      "打开便签超时"
    );
    await refreshData();
  } catch (error) {
    console.error("[sticky-note] 打开便签失败", error);
    const message = error instanceof Error ? error.message : String(error);
    alert(`打开便签失败：${message}`);
  }
};

const addCustomNote = async () => {
  if (creatingNote.value) {
    return;
  }
  creatingNote.value = true;
  try {
    const position = buildDefaultPosition();
    await withInvokeTimeout(
      api.createStickyNote({
        defaultX: position.x,
        defaultY: position.y
      }),
      5000,
      "新增便签超时"
    );
    await refreshData();
  } catch (error) {
    console.error("[sticky-note] 新增便签失败", error);
    const message = error instanceof Error ? error.message : String(error);
    alert(`新增便签失败：${message}`);
  } finally {
    creatingNote.value = false;
  }
};

const hideEntryContextMenu = () => {
  entryContextMenu.value.visible = false;
  entryContextMenu.value.taskId = null;
};

const openEntryContextMenu = (event: MouseEvent, entry: NoteListEntry) => {
  const menuWidth = 144;
  const menuHeight = 44;
  const maxX = Math.max(8, window.innerWidth - menuWidth - 8);
  const maxY = Math.max(8, window.innerHeight - menuHeight - 8);
  entryContextMenu.value.visible = true;
  entryContextMenu.value.x = Math.min(Math.max(event.clientX, 8), maxX);
  entryContextMenu.value.y = Math.min(Math.max(event.clientY, 8), maxY);
  entryContextMenu.value.taskId = entry.id;
};

const markEntryCompleted = async () => {
  const taskId = entryContextMenu.value.taskId;
  hideEntryContextMenu();
  if (!taskId) {
    return;
  }
  try {
    await api.completeTask(taskId);
    await refreshData();
  } catch (error) {
    console.error("[sticky-note] 标记完成失败", error);
    const message = error instanceof Error ? error.message : String(error);
    alert(`标记完成失败：${message}`);
  }
};

const openFromEntry = async (entry: NoteListEntry) => {
  hideEntryContextMenu();
  const task = tasks.value.find(item => item.id === entry.id);
  if (!task) {
    return;
  }
  await openTaskNote(task);
};

const closeNote = async (noteId: string) => {
  await api.closeStickyNote(noteId);
  await refreshData();
};

const closeWindow = async () => {
  if (props.embedded) {
    emitClose("request-close");
    return;
  }
  await api.setStickyNoteWindowVisible(false);
};

onMounted(async () => {
  applyThemeFromStorage();
  await refreshData();
  window.addEventListener("click", hideEntryContextMenu);
  window.addEventListener("contextmenu", hideEntryContextMenu);
  storageHandler = event => {
    if (event.key === "appTheme") {
      applyThemeFromStorage();
    }
  };
  window.addEventListener("storage", storageHandler);
  unlistenChanged = await listen("sticky-note-changed", () => {
    void refreshData();
  });
  unlistenSettingsUpdated = await listen("sticky-note-settings-updated", () => {
    void refreshData();
  });
  unlistenThemeChanged = await listen<string>("app-theme-updated", event => {
    applyTheme(event.payload === "light");
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("click", hideEntryContextMenu);
  window.removeEventListener("contextmenu", hideEntryContextMenu);
  if (storageHandler) {
    window.removeEventListener("storage", storageHandler);
  }
  if (unlistenChanged) {
    unlistenChanged();
  }
  if (unlistenThemeChanged) {
    unlistenThemeChanged();
  }
  if (unlistenSettingsUpdated) {
    unlistenSettingsUpdated();
  }
});
</script>
