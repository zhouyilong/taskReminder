<template>
  <div class="sticky-note-item-root todo-sticky-root" :style="rootStyle">
    <article class="paper-note paper-note-window todo-sticky">
      <div class="resize-edge resize-top" @mousedown.stop.prevent="startResize('North')"></div>
      <div class="resize-edge resize-right" @mousedown.stop.prevent="startResize('East')"></div>
      <div class="resize-edge resize-bottom" @mousedown.stop.prevent="startResize('South')"></div>
      <div class="resize-edge resize-left" @mousedown.stop.prevent="startResize('West')"></div>
      <div class="resize-edge resize-top-left" @mousedown.stop.prevent="startResize('NorthWest')"></div>
      <div class="resize-edge resize-top-right" @mousedown.stop.prevent="startResize('NorthEast')"></div>
      <div class="resize-edge resize-bottom-left" @mousedown.stop.prevent="startResize('SouthWest')"></div>
      <div class="resize-edge resize-bottom-right" @mousedown.stop.prevent="startResize('SouthEast')"></div>

      <header
        class="paper-note-header todo-sticky-header"
        :data-tauri-drag-region="pinned ? null : ''"
        :class="{ 'paper-note-header-pinned': pinned }"
      >
        <div class="todo-sticky-heading" :data-tauri-drag-region="pinned ? null : ''">
          <span class="todo-sticky-title" :data-tauri-drag-region="pinned ? null : ''">待办清单</span>
          <span class="todo-sticky-count" :data-tauri-drag-region="pinned ? null : ''">{{ summary }}</span>
        </div>
        <div class="paper-note-actions">
          <button
            class="paper-note-action pin"
            :class="{ active: pinned }"
            type="button"
            :title="pinned ? '取消锚定（取消置顶并恢复可移动）' : '锚定清单（置顶并锁定位置）'"
            @mousedown.stop.prevent
            @click.stop="togglePin"
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
            title="添加待办"
            @mousedown.stop.prevent
            @click.stop="focusInput"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" stroke="currentColor" fill="none" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
          </button>
          <button
            class="paper-note-close"
            type="button"
            title="关闭清单"
            @mousedown.stop.prevent
            @click.stop="closeList"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" stroke="currentColor" fill="none" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </header>

      <ul class="todo-sticky-list" aria-label="待办">
        <li
          v-for="item in items"
          :key="item.task.id"
          class="todo-sticky-row"
          :class="[`is-${item.tone}`, { 'is-completing': completing.has(item.task.id) }]"
          :title="`${item.task.description}\n点击打开便签查看`"
          @click="openItem(item)"
        >
          <button
            class="todo-sticky-check"
            type="button"
            :aria-label="`完成：${item.title}`"
            title="标记完成"
            @click.stop="complete(item)"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" stroke="currentColor" fill="none" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
          </button>
          <span class="todo-sticky-text">{{ item.title }}</span>
          <span v-if="item.when" class="todo-sticky-when">{{ item.when }}</span>
        </li>
        <li v-if="loaded && !items.length" class="todo-sticky-empty">
          <span>没有待办</span>
          <span class="todo-sticky-empty-hint">在下方输入后回车添加</span>
        </li>
      </ul>

      <footer class="paper-note-footer todo-sticky-footer">
        <input
          ref="inputRef"
          v-model="draft"
          class="todo-sticky-input"
          type="text"
          placeholder="添加待办，回车保存（如“明天3点 交周报”）"
          :disabled="saving"
          @mousedown.stop
          @keydown.enter.prevent="submit"
          @keydown.esc.prevent="draft = ''"
        />
      </footer>
      <div v-if="message" class="todo-sticky-toast" :class="{ 'is-error': messageIsError }" role="status">{{ message }}</div>
    </article>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "./api";
import { errorMessage } from "./format";
import { safeStorage } from "./safeStorage";
import { parseTodoInput, todoStickyItems, todoStickySummary, type TodoStickyItem } from "./todoSticky";
import type { AppSettings, Task, TodoStickyState, UiStatePayload } from "./types";

// @tauri-apps/api 未导出该类型，这里按 startResizeDragging 的参数定义同名联合类型。
type ResizeDirection = "East" | "North" | "NorthEast" | "NorthWest" | "South" | "SouthEast" | "SouthWest" | "West";

/** 普通便签的默认宽度（与后端 STICKY_NOTE_ITEM_WIDTH 一致），打开时放在清单旁边。 */
const NOTE_WIDTH = 284;
const NOTE_GAP = 12;

const windowRef = getCurrentWindow();
const tasks = ref<Task[]>([]);
const loaded = ref(false);
const pinned = ref(false);
const draft = ref("");
const saving = ref(false);
const message = ref("");
const messageIsError = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);
const completing = ref(new Set<string>());
const now = ref(new Date());
const uiScale = ref(1);
const windowOpacity = ref(1);
let messageTimer = 0;
let clockTimer = 0;
const unlisteners: UnlistenFn[] = [];

const items = computed(() => todoStickyItems(tasks.value, now.value));
const summary = computed(() => (loaded.value ? todoStickySummary(items.value) : "载入中…"));
const rootStyle = computed(() => ({
  width: "100%",
  height: "100%",
  "--sticky-ui-scale": `${uiScale.value}`,
  opacity: windowOpacity.value,
}));

const clamp = (value: number, min: number, max: number, fallback: number) =>
  Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback;

const applyTheme = (light: boolean) => {
  document.documentElement.classList.toggle("light-theme", light);
  document.body.classList.toggle("light-theme", light);
};

const applyUiState = (payload: Partial<UiStatePayload> | null | undefined) => {
  if (!payload) return;
  if (payload.theme) applyTheme(payload.theme === "light");
  if (payload.uiScale !== undefined) uiScale.value = clamp(Number(payload.uiScale), 0.8, 1.2, 1);
  if (payload.windowOpacity !== undefined) windowOpacity.value = clamp(Number(payload.windowOpacity), 0.3, 1, 1);
};

const showMessage = (text: string, isError = false) => {
  message.value = text;
  messageIsError.value = isError;
  window.clearTimeout(messageTimer);
  messageTimer = window.setTimeout(() => {
    message.value = "";
  }, isError ? 4000 : 2200);
};

const loadTasks = async () => {
  try {
    tasks.value = await api.listActiveTasks();
    loaded.value = true;
  } catch (error) {
    console.error("[todo-sticky] 读取待办失败", error);
  }
};

const applyState = (state: TodoStickyState | null | undefined) => {
  if (state) pinned.value = Boolean(state.pinned);
};

const togglePin = async () => {
  try {
    applyState(await api.setTodoStickyPinned(!pinned.value));
  } catch (error) {
    console.warn("[todo-sticky] 切换锚定失败", error);
  }
};

const closeList = async () => {
  try {
    await api.closeTodoSticky();
  } catch (error) {
    console.error("[todo-sticky] 关闭清单失败", error);
    try {
      await windowRef.hide();
    } catch {
      // 浏览器预览时没有 Tauri 窗口。
    }
  }
};

const focusInput = () => {
  inputRef.value?.focus();
};

const startResize = async (edge: ResizeDirection) => {
  try {
    await windowRef.startResizeDragging(edge);
  } catch (error) {
    console.error("[todo-sticky] 调整大小失败", error);
  }
};

/** 在清单旁边（左侧放得下时放左侧，否则右侧）打开这条待办的便签。已摆放过的便签仍在原位置打开。 */
const notePosition = async () => {
  try {
    const [scale, position, size] = await Promise.all([
      windowRef.scaleFactor(),
      windowRef.outerPosition(),
      windowRef.outerSize(),
    ]);
    const x = position.x / scale;
    const y = position.y / scale;
    const width = size.width / scale;
    const left = x - NOTE_WIDTH - NOTE_GAP;
    return { defaultX: left >= 0 ? left : x + width + NOTE_GAP, defaultY: y };
  } catch {
    return {};
  }
};

const openItem = async (item: TodoStickyItem) => {
  try {
    await api.openStickyNote({ taskId: item.task.id, title: item.task.description, ...(await notePosition()) });
  } catch (error) {
    showMessage(`打开便签失败：${errorMessage(error)}`, true);
  }
};

const complete = async (item: TodoStickyItem) => {
  const id = item.task.id;
  if (completing.value.has(id)) return;
  completing.value = new Set(completing.value).add(id);
  try {
    await api.completeTask(id);
    // 先让划线动画播完，再从列表中移除。
    await new Promise(resolve => window.setTimeout(resolve, 260));
    tasks.value = tasks.value.filter(task => task.id !== id);
  } catch (error) {
    showMessage(`标记完成失败：${errorMessage(error)}`, true);
  } finally {
    const next = new Set(completing.value);
    next.delete(id);
    completing.value = next;
  }
};

const submit = async () => {
  if (saving.value) return;
  const result = parseTodoInput(draft.value);
  if (!result) return;
  if (result.kind === "error") {
    showMessage(result.message, true);
    return;
  }
  saving.value = true;
  try {
    if (result.kind === "recurring") {
      await api.createRecurringTask(result.payload);
    } else {
      await api.quickAddTask(result.payload);
    }
    draft.value = "";
    showMessage(result.hint);
    await loadTasks();
  } catch (error) {
    showMessage(errorMessage(error), true);
  } finally {
    saving.value = false;
    requestAnimationFrame(() => inputRef.value?.focus());
  }
};

const listenSafely = async <T,>(event: string, handler: (payload: T) => void) => {
  try {
    unlisteners.push(await listen<T>(event, item => handler(item.payload)));
  } catch (error) {
    console.warn(`[todo-sticky] 监听 ${event} 失败`, error);
  }
};

onMounted(async () => {
  applyTheme(safeStorage.getItem("appTheme") === "light");
  applyUiState({
    uiScale: Number(safeStorage.getItem("uiScale") ?? 1),
    windowOpacity: Number(safeStorage.getItem("windowOpacity") ?? 1),
  });
  clockTimer = window.setInterval(() => {
    now.value = new Date();
  }, 30000);
  await loadTasks();
  try {
    applyState(await api.getTodoStickyState());
  } catch (error) {
    console.warn("[todo-sticky] 读取清单状态失败", error);
  }
  try {
    applyUiState(await api.getUiState());
  } catch (error) {
    console.warn("[todo-sticky] 读取界面状态失败", error);
  }
  try {
    const settings = await api.getSettings();
    applyUiState({ windowOpacity: settings.windowOpacity ?? 1 });
  } catch (error) {
    console.warn("[todo-sticky] 读取设置失败", error);
  }
  // 任何窗口改动数据（含同步合并）后都会广播 data-updated。
  await listenSafely("data-updated", () => void loadTasks());
  await listenSafely<TodoStickyState>("todo-sticky-changed", applyState);
  await listenSafely<UiStatePayload>("ui-state-changed", applyUiState);
  await listenSafely<AppSettings>("settings-updated", settings =>
    applyUiState({ windowOpacity: settings.windowOpacity ?? 1 })
  );
});

onBeforeUnmount(() => {
  unlisteners.forEach(unlisten => unlisten());
  window.clearTimeout(messageTimer);
  window.clearInterval(clockTimer);
});
</script>

<style scoped>
.todo-sticky-heading {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: calc(8px * var(--sticky-ui-scale));
  overflow: hidden;
}

.todo-sticky-title {
  color: var(--sticky-title);
  font-size: calc(15px * var(--sticky-ui-scale));
  font-weight: 650;
  letter-spacing: 0.2px;
  white-space: nowrap;
}

.todo-sticky-count {
  color: var(--sticky-text-faint);
  font-size: calc(11px * var(--sticky-ui-scale));
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.todo-sticky-list {
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: calc(2px * var(--sticky-ui-scale)) calc(8px * var(--sticky-ui-scale)) calc(6px * var(--sticky-ui-scale));
  list-style: none;
  overflow-y: auto;
}

.todo-sticky-list::-webkit-scrollbar {
  width: 6px;
}

.todo-sticky-list::-webkit-scrollbar-thumb {
  background: var(--sticky-scrollbar);
  border-radius: 3px;
}

.todo-sticky-row {
  display: flex;
  align-items: center;
  gap: calc(8px * var(--sticky-ui-scale));
  min-height: calc(32px * var(--sticky-ui-scale));
  padding: 0 calc(8px * var(--sticky-ui-scale));
  border-radius: calc(8px * var(--sticky-ui-scale));
  color: var(--sticky-text);
  font-size: calc(13px * var(--sticky-ui-scale));
  cursor: pointer;
  transition: background-color 0.15s ease, opacity 0.25s ease;
}

.todo-sticky-row:hover {
  background: var(--sticky-control-hover);
}

.todo-sticky-check {
  flex-shrink: 0;
  width: calc(16px * var(--sticky-ui-scale));
  height: calc(16px * var(--sticky-ui-scale));
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1.5px solid var(--sticky-text-faint);
  border-radius: 50%;
  background: transparent;
  color: transparent;
  cursor: pointer;
  transition: border-color 0.15s ease, background-color 0.15s ease, color 0.15s ease;
}

.todo-sticky-check svg {
  width: 70%;
  height: 70%;
}

.todo-sticky-check:hover,
.todo-sticky-check:focus-visible {
  border-color: var(--success-text);
  color: var(--success-text);
  outline: none;
}

.todo-sticky-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.todo-sticky-when {
  flex-shrink: 0;
  color: var(--sticky-text-faint);
  font-size: calc(11px * var(--sticky-ui-scale));
  font-variant-numeric: tabular-nums;
}

.todo-sticky-row.is-today .todo-sticky-when {
  color: var(--primary-text);
}

.todo-sticky-row.is-overdue .todo-sticky-when {
  color: var(--danger-text);
}

.todo-sticky-row.is-completing {
  opacity: 0.45;
}

.todo-sticky-row.is-completing .todo-sticky-text {
  text-decoration: line-through;
}

.todo-sticky-row.is-completing .todo-sticky-check {
  border-color: var(--success-text);
  background: var(--success-soft);
  color: var(--success-text);
}

.todo-sticky-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: calc(28px * var(--sticky-ui-scale)) 0;
  color: var(--sticky-text-muted);
  font-size: calc(13px * var(--sticky-ui-scale));
}

.todo-sticky-empty-hint {
  color: var(--sticky-text-faint);
  font-size: calc(11px * var(--sticky-ui-scale));
}

.todo-sticky-footer {
  height: calc(40px * var(--sticky-ui-scale));
  padding: 0 calc(10px * var(--sticky-ui-scale));
}

.todo-sticky-input {
  width: 100%;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--sticky-text);
  font-family: var(--font-sans);
  font-size: calc(13px * var(--sticky-ui-scale));
  outline: none;
}

.todo-sticky-input::placeholder {
  color: var(--sticky-text-faint);
}

.todo-sticky-toast {
  position: absolute;
  left: 50%;
  bottom: calc(48px * var(--sticky-ui-scale));
  transform: translateX(-50%);
  max-width: calc(100% - 32px);
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--success-soft);
  color: var(--success-text);
  font-size: calc(12px * var(--sticky-ui-scale));
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  pointer-events: none;
}

.todo-sticky-toast.is-error {
  background: var(--danger-soft);
  color: var(--danger-text);
}
</style>
