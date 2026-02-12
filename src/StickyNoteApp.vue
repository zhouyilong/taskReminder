<template>
  <div class="sticky-board">
    <div class="sticky-topbar" data-tauri-drag-region>
      <div class="sticky-brand">桌面便签</div>
      <div class="sticky-topbar-actions">
        <button class="sticky-ghost-button" type="button" @click="refreshData">刷新</button>
        <button class="sticky-close-window" type="button" @click="closeWindow">关闭</button>
      </div>
    </div>

    <div class="sticky-layout">
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
          <button
            v-for="entry in filteredEntries"
            :key="entry.id"
            class="sticky-task-item compact"
            :class="{ active: isNoteOpen(entry.id) }"
            type="button"
            @click="openFromEntry(entry)"
          >
            <span class="sticky-item-dot" :class="entry.noteType === 'CUSTOM' ? 'custom' : 'task'"></span>
            <span class="sticky-task-text compact">{{ entry.title }}</span>
          </button>
          <div v-if="filteredEntries.length === 0" class="sticky-task-empty">暂无便签项</div>
        </div>
      </aside>

      <section class="sticky-canvas">
        <div v-if="openNotes.length === 0" class="sticky-canvas-empty">
          点击左侧项目即可打开便签
        </div>
      </section>
    </div>

    <div class="sticky-notes-layer">
      <article
        v-for="note in sortedOpenNotes"
        :key="note.taskId"
        class="paper-note"
        :style="{ left: note.posX + 'px', top: note.posY + 'px', zIndex: note.zIndex }"
        @mousedown="bringToFront(note.taskId)"
      >
        <div class="paper-pin"></div>
        <header class="paper-note-header" @mousedown.prevent="startDragging($event, note.taskId)">
          <input
            v-model="note.title"
            class="paper-note-title-input"
            type="text"
            placeholder="便签标题"
            @mousedown.stop
            @input="handleTitleInput(note.taskId)"
          />
          <button class="paper-note-close" type="button" @click.stop="closeTaskNote(note.taskId)">×</button>
        </header>
        <textarea
          v-model="note.content"
          class="paper-note-editor"
          placeholder="在这里写下便签内容..."
          @input="handleNoteInput(note.taskId)"
        />
        <div class="paper-note-footer">{{ note.saveHint }}</div>
      </article>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { safeStorage } from "./safeStorage";
import { api } from "./api";
import type { StickyNote, Task } from "./types";

const NOTE_PADDING = 12;

type BoardNote = StickyNote & {
  zIndex: number;
  saveHint: string;
};

type NoteListEntry = {
  id: string;
  title: string;
  noteType: "TASK" | "CUSTOM";
};

type DraggingState = {
  taskId: string;
  startX: number;
  startY: number;
  baseX: number;
  baseY: number;
};

const tasks = ref<Task[]>([]);
const allNotes = ref<StickyNote[]>([]);
const openNotes = ref<BoardNote[]>([]);
const taskKeyword = ref("");
const dragging = ref<DraggingState | null>(null);
const zCounter = ref(100);
const saveTimers = new Map<string, number>();
const titleSaveTimers = new Map<string, number>();
let storageHandler: ((event: StorageEvent) => void) | null = null;

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

const sortedOpenNotes = computed(() => {
  return [...openNotes.value].sort((a, b) => a.zIndex - b.zIndex);
});

const applyThemeFromStorage = () => {
  const useLight = safeStorage.getItem("appTheme") === "light";
  document.documentElement.classList.toggle("light-theme", useLight);
  document.body.classList.toggle("light-theme", useLight);
};

const syncOpenNotes = () => {
  const taskTitleMap = new Map(tasks.value.map(task => [task.id, task.description]));
  const visibleNotes = allNotes.value
    .filter(note => note.isOpen)
    .map((note, index) => ({
      ...note,
      title: note.title?.trim() || taskTitleMap.get(note.taskId) || "待办便签",
      zIndex: 100 + index,
      saveHint: "自动保存"
    }));
  zCounter.value = 100 + visibleNotes.length;
  openNotes.value = visibleNotes;
};

const refreshData = async () => {
  const [taskRows, noteRows] = await Promise.all([
    api.listActiveTasks(),
    api.listStickyNotes()
  ]);
  tasks.value = taskRows;
  allNotes.value = noteRows;
  syncOpenNotes();
};

const isNoteOpen = (noteId: string) => {
  return openNotes.value.some(note => note.taskId === noteId);
};

const bringToFront = (taskId: string) => {
  const target = openNotes.value.find(note => note.taskId === taskId);
  if (!target) {
    return;
  }
  zCounter.value += 1;
  target.zIndex = zCounter.value;
};

const clampPosition = (x: number, y: number) => {
  if (!Number.isFinite(x) || !Number.isFinite(y)) {
    return { x: NOTE_PADDING, y: NOTE_PADDING };
  }
  return {
    x,
    y
  };
};

const buildDefaultPosition = () => {
  const opened = openNotes.value.length;
  return clampPosition(
    NOTE_PADDING + (opened % 8) * 20,
    NOTE_PADDING + (opened % 8) * 14
  );
};

const openTaskNote = async (task: Task) => {
  const existing = openNotes.value.find(note => note.taskId === task.id);
  if (existing) {
    bringToFront(task.id);
    return;
  }
  const position = buildDefaultPosition();
  await api.openStickyNote({
    taskId: task.id,
    title: task.description,
    defaultX: position.x,
    defaultY: position.y
  });
  await refreshData();
  bringToFront(task.id);
};

const openCustomNote = async (noteId: string, title: string) => {
  const existing = openNotes.value.find(note => note.taskId === noteId);
  if (existing) {
    bringToFront(noteId);
    return;
  }
  await api.openStickyNote({ taskId: noteId, title });
  await refreshData();
  bringToFront(noteId);
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
  const created = await api.createStickyNote({
    title: `新便签 ${allNotes.value.filter(note => note.noteType === "CUSTOM").length + 1}`,
    defaultX: position.x,
    defaultY: position.y
  });
  await refreshData();
  bringToFront(created.taskId);
};

const closeTaskNote = async (taskId: string) => {
  await api.closeStickyNote(taskId);
  openNotes.value = openNotes.value.filter(note => note.taskId !== taskId);
  const timer = saveTimers.get(taskId);
  if (timer !== undefined) {
    clearTimeout(timer);
    saveTimers.delete(taskId);
  }
  const titleTimer = titleSaveTimers.get(taskId);
  if (titleTimer !== undefined) {
    clearTimeout(titleTimer);
    titleSaveTimers.delete(taskId);
  }
};

const scheduleTitleSave = (taskId: string) => {
  const note = openNotes.value.find(item => item.taskId === taskId);
  if (!note) {
    return;
  }
  const timer = titleSaveTimers.get(taskId);
  if (timer !== undefined) {
    clearTimeout(timer);
  }
  const nextTimer = window.setTimeout(async () => {
    const latest = openNotes.value.find(item => item.taskId === taskId);
    if (!latest) {
      titleSaveTimers.delete(taskId);
      return;
    }
    try {
      await api.updateStickyNoteTitle({
        taskId,
        title: latest.title
      });
    } catch (error) {
      console.error("[sticky-note] 标题保存失败", error);
    } finally {
      titleSaveTimers.delete(taskId);
    }
  }, 450);
  titleSaveTimers.set(taskId, nextTimer);
};

const handleTitleInput = (taskId: string) => {
  const note = openNotes.value.find(item => item.taskId === taskId);
  if (!note) {
    return;
  }
  const shadow = allNotes.value.find(item => item.taskId === taskId);
  if (shadow) {
    shadow.title = note.title;
  }
  scheduleTitleSave(taskId);
};

const scheduleSave = (taskId: string) => {
  const note = openNotes.value.find(item => item.taskId === taskId);
  if (!note) {
    return;
  }
  const timer = saveTimers.get(taskId);
  if (timer !== undefined) {
    clearTimeout(timer);
  }
  note.saveHint = "保存中...";
  const nextTimer = window.setTimeout(async () => {
    const latest = openNotes.value.find(item => item.taskId === taskId);
    if (!latest) {
      saveTimers.delete(taskId);
      return;
    }
    try {
      await api.saveStickyNoteContent({
        taskId,
        content: latest.content
      });
      latest.saveHint = `已保存 ${new Date().toLocaleTimeString("zh-CN", { hour12: false })}`;
    } catch (error) {
      console.error("[sticky-note] 保存失败", error);
      latest.saveHint = "保存失败";
    } finally {
      saveTimers.delete(taskId);
    }
  }, 500);
  saveTimers.set(taskId, nextTimer);
};

const handleNoteInput = (taskId: string) => {
  scheduleSave(taskId);
};

const onWindowMouseMove = (event: MouseEvent) => {
  const state = dragging.value;
  if (!state) {
    return;
  }
  const note = openNotes.value.find(item => item.taskId === state.taskId);
  if (!note) {
    return;
  }
  const clamped = clampPosition(
    state.baseX + (event.clientX - state.startX),
    state.baseY + (event.clientY - state.startY)
  );
  note.posX = clamped.x;
  note.posY = clamped.y;
};

const stopDragging = async () => {
  const state = dragging.value;
  dragging.value = null;
  if (!state) {
    return;
  }
  const note = openNotes.value.find(item => item.taskId === state.taskId);
  if (!note) {
    return;
  }
  try {
    await api.moveStickyNote({
      taskId: note.taskId,
      x: note.posX,
      y: note.posY
    });
  } catch (error) {
    console.error("[sticky-note] 保存位置失败", error);
  }
};

const onWindowMouseUp = () => {
  void stopDragging();
};

const startDragging = (event: MouseEvent, taskId: string) => {
  if (
    (event.target as HTMLElement).closest(".paper-note-close") ||
    (event.target as HTMLElement).closest(".paper-note-title-input")
  ) {
    return;
  }
  const note = openNotes.value.find(item => item.taskId === taskId);
  if (!note) {
    return;
  }
  bringToFront(taskId);
  dragging.value = {
    taskId,
    startX: event.clientX,
    startY: event.clientY,
    baseX: note.posX,
    baseY: note.posY
  };
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
  window.addEventListener("mousemove", onWindowMouseMove);
  window.addEventListener("mouseup", onWindowMouseUp);
});

onBeforeUnmount(() => {
  if (storageHandler) {
    window.removeEventListener("storage", storageHandler);
  }
  window.removeEventListener("mousemove", onWindowMouseMove);
  window.removeEventListener("mouseup", onWindowMouseUp);
  saveTimers.forEach(timer => clearTimeout(timer));
  saveTimers.clear();
  titleSaveTimers.forEach(timer => clearTimeout(timer));
  titleSaveTimers.clear();
});
</script>
