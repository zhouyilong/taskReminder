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
        <div class="sticky-task-title">待办列表</div>
        <input
          v-model="taskKeyword"
          class="sticky-task-search"
          placeholder="搜索待办..."
        />
        <div class="sticky-task-list">
          <button
            v-for="task in filteredTasks"
            :key="task.id"
            class="sticky-task-item"
            :class="{ active: isTaskOpen(task.id) }"
            type="button"
            @click="openTaskNote(task)"
          >
            <span class="sticky-task-text">{{ task.description }}</span>
          </button>
          <div v-if="filteredTasks.length === 0" class="sticky-task-empty">暂无待办</div>
        </div>
      </aside>

      <section ref="canvasRef" class="sticky-canvas">
        <div v-if="openNotes.length === 0" class="sticky-canvas-empty">
          点击左侧待办，即可生成一张便签
        </div>

        <article
          v-for="note in sortedOpenNotes"
          :key="note.taskId"
          class="paper-note"
          :style="{ left: note.posX + 'px', top: note.posY + 'px', zIndex: note.zIndex }"
          @mousedown="bringToFront(note.taskId)"
        >
          <div class="paper-pin"></div>
          <header class="paper-note-header" @mousedown.prevent="startDragging($event, note.taskId)">
            <div class="paper-note-title">{{ note.title }}</div>
            <button class="paper-note-close" type="button" @click.stop="closeTaskNote(note.taskId)">×</button>
          </header>
          <textarea
            v-model="note.content"
            class="paper-note-editor"
            placeholder="在这里写下这条待办的补充信息..."
            @input="handleNoteInput(note.taskId)"
          />
          <div class="paper-note-footer">{{ note.saveHint }}</div>
        </article>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow, type Window as TauriWindow } from "@tauri-apps/api/window";
import { api } from "./api";
import { safeStorage } from "./safeStorage";
import type { StickyNote, Task } from "./types";

const NOTE_WIDTH = 284;
const NOTE_HEIGHT = 280;
const NOTE_PADDING = 12;

type BoardNote = StickyNote & {
  title: string;
  zIndex: number;
  saveHint: string;
};

type DraggingState = {
  taskId: string;
  startX: number;
  startY: number;
  baseX: number;
  baseY: number;
};

const tasks = ref<Task[]>([]);
const openNotes = ref<BoardNote[]>([]);
const taskKeyword = ref("");
const canvasRef = ref<HTMLElement | null>(null);
const dragging = ref<DraggingState | null>(null);
const zCounter = ref(100);
const appWindow = resolveWindow();
let storageHandler: ((event: StorageEvent) => void) | null = null;

const filteredTasks = computed(() => {
  const keyword = taskKeyword.value.trim().toLowerCase();
  if (!keyword) {
    return tasks.value;
  }
  return tasks.value.filter(task => task.description.toLowerCase().includes(keyword));
});

const sortedOpenNotes = computed(() => {
  return [...openNotes.value].sort((a, b) => a.zIndex - b.zIndex);
});

function resolveWindow(): TauriWindow | null {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

const applyThemeFromStorage = () => {
  const useLight = safeStorage.getItem("appTheme") === "light";
  document.documentElement.classList.toggle("light-theme", useLight);
  document.body.classList.toggle("light-theme", useLight);
};

const syncOpenNotes = (notes: StickyNote[]) => {
  const titleById = new Map(tasks.value.map(task => [task.id, task.description]));
  const visible = notes
    .filter(note => note.isOpen && titleById.has(note.taskId))
    .map((note, index) => ({
      ...note,
      title: titleById.get(note.taskId) || "未命名待办",
      zIndex: 100 + index,
      saveHint: "自动保存"
    }));
  zCounter.value = 100 + visible.length;
  openNotes.value = visible;
};

const refreshData = async () => {
  const [taskRows, noteRows] = await Promise.all([
    api.listActiveTasks(),
    api.listStickyNotes()
  ]);
  tasks.value = taskRows;
  syncOpenNotes(noteRows);
};

const isTaskOpen = (taskId: string) => {
  return openNotes.value.some(note => note.taskId === taskId);
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
  const canvas = canvasRef.value;
  if (!canvas) {
    return { x: Math.max(NOTE_PADDING, x), y: Math.max(NOTE_PADDING, y) };
  }
  const maxX = Math.max(NOTE_PADDING, canvas.clientWidth - NOTE_WIDTH - NOTE_PADDING);
  const maxY = Math.max(NOTE_PADDING, canvas.clientHeight - NOTE_HEIGHT - NOTE_PADDING);
  return {
    x: Math.min(maxX, Math.max(NOTE_PADDING, x)),
    y: Math.min(maxY, Math.max(NOTE_PADDING, y))
  };
};

const openTaskNote = async (task: Task) => {
  const existing = openNotes.value.find(note => note.taskId === task.id);
  if (existing) {
    bringToFront(task.id);
    return;
  }
  const offset = openNotes.value.length;
  const defaultPosition = clampPosition(
    NOTE_PADDING + (offset % 6) * 24,
    NOTE_PADDING + (offset % 6) * 16
  );
  const note = await api.openStickyNote({
    taskId: task.id,
    defaultX: defaultPosition.x,
    defaultY: defaultPosition.y
  });
  zCounter.value += 1;
  openNotes.value.push({
    ...note,
    title: task.description,
    zIndex: zCounter.value,
    saveHint: "自动保存"
  });
};

const closeTaskNote = async (taskId: string) => {
  await api.closeStickyNote(taskId);
  openNotes.value = openNotes.value.filter(note => note.taskId !== taskId);
  const timer = saveTimers.get(taskId);
  if (timer !== undefined) {
    clearTimeout(timer);
    saveTimers.delete(taskId);
  }
};

const saveTimers = new Map<string, number>();

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
  const nextX = state.baseX + (event.clientX - state.startX);
  const nextY = state.baseY + (event.clientY - state.startY);
  const clamped = clampPosition(nextX, nextY);
  note.posX = clamped.x;
  note.posY = clamped.y;
};

const onWindowMouseUp = () => {
  void stopDragging();
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

const startDragging = (event: MouseEvent, taskId: string) => {
  if ((event.target as HTMLElement).closest(".paper-note-close")) {
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
  if (!appWindow) {
    return;
  }
  await appWindow.hide();
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
});
</script>
