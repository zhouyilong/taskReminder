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
        data-tauri-drag-region
        :class="{ 'paper-note-header-pinned': pinned }"
      >
        <div class="todo-sticky-heading" data-tauri-drag-region>
          <span class="todo-sticky-title" data-tauri-drag-region>待办清单</span>
          <span class="todo-sticky-count" data-tauri-drag-region>{{ summary }}</span>
        </div>
        <div class="paper-note-actions">
          <div class="paper-note-color todo-sticky-color">
            <button
              class="paper-note-action todo-sticky-color-trigger"
              :class="{ 'is-open': colorMenuOpen }"
              type="button"
              title="清单颜色"
              @mousedown.stop.prevent
              @click.stop="colorMenuOpen = !colorMenuOpen"
            >
              <span class="sticky-color-dot" :class="dotClass(color)"></span>
            </button>
            <div v-if="colorMenuOpen" class="paper-note-color-menu todo-sticky-color-menu" role="menu" @mousedown.stop>
              <button
                v-for="option in STICKY_COLOR_OPTIONS"
                :key="option.value"
                class="paper-note-color-option"
                :class="{ active: resolveStickyColor(color) === option.value }"
                type="button"
                role="menuitemradio"
                :aria-checked="resolveStickyColor(color) === option.value"
                :title="option.label"
                @click.stop="chooseColor(option.value)"
              >
                <span class="sticky-color-dot" :class="dotClass(option.value)"></span>
              </button>
            </div>
          </div>
          <button
            class="paper-note-action pin"
            :class="{ active: pinned }"
            type="button"
            :title="pinned ? '取消锚定（取消置顶）' : '锚定清单（置顶，仍可拖动）'"
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

      <TransitionGroup
        tag="ul"
        name="todo-row"
        class="todo-sticky-list"
        :class="{ 'is-dragging': drag.active }"
        aria-label="待办"
      >
        <li
          v-for="(item, index) in items"
          :key="item.task.id"
          class="todo-sticky-row"
          :class="[
            `is-${item.tone}`,
            {
              'is-urgent': item.urgent,
              'is-checked': checked.has(item.task.id),
              'is-drag-source': drag.active && drag.id === item.task.id,
              'is-drop-before': dropMarker === index,
              'is-drop-after': dropMarker === items.length && index === items.length - 1,
            },
          ]"
          :title="`${item.task.description}\n双击打开便签，拖动调整顺序`"
          @mousedown="startDrag($event, item)"
          @dblclick="openItem(item)"
        >
          <button
            class="todo-sticky-check"
            type="button"
            role="checkbox"
            :aria-checked="checked.has(item.task.id)"
            :aria-label="`完成：${item.title}`"
            :title="checked.has(item.task.id) ? '撤销完成' : '标记完成'"
            @mousedown.stop
            @dblclick.stop
            @click.stop="toggleCheck(item)"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" stroke="currentColor" fill="none" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
          </button>
          <span v-if="item.urgent" class="todo-sticky-urgent-badge" title="加急">急</span>
          <span class="todo-sticky-text">{{ item.title }}</span>
          <span v-if="item.when" class="todo-sticky-when">{{ item.when }}</span>
          <button
            class="todo-sticky-urgent-toggle"
            :class="{ active: item.urgent }"
            type="button"
            :aria-pressed="item.urgent"
            :aria-label="item.urgent ? `取消加急：${item.title}` : `加急：${item.title}`"
            :title="item.urgent ? '取消加急' : '加急'"
            @mousedown.stop
            @dblclick.stop
            @click.stop="toggleUrgent(item)"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" stroke="currentColor" fill="none" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 3c.5 3.2 4.5 5.4 4.5 10a4.5 4.5 0 0 1-9 0c0-1.9.8-3.3 1.8-4.4.2 1.5 1 2.6 2.2 3.1C11 9.6 10.6 6.2 12 3z"></path>
            </svg>
          </button>
        </li>
        <li v-if="loaded && !items.length" key="empty" class="todo-sticky-empty">
          <span>没有待办</span>
          <span class="todo-sticky-empty-hint">在下方输入后回车添加</span>
        </li>
      </TransitionGroup>

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
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "./api";
import { errorMessage } from "./format";
import { safeStorage } from "./safeStorage";
import { planReorder } from "./reorder";
import { STICKY_COLOR_OPTIONS, resolveStickyColor, stickyColorClass, stickyDotClass as dotClass } from "./stickies";
import { dropIndexAt, parseTodoInput, todoStickyItems, todoStickySummary, type TodoStickyItem } from "./todoSticky";
import type { AppSettings, Task, TodoStickyState, UiStatePayload } from "./types";

// @tauri-apps/api 未导出该类型，这里按 startResizeDragging 的参数定义同名联合类型。
type ResizeDirection = "East" | "North" | "NorthEast" | "NorthWest" | "South" | "SouthEast" | "SouthWest" | "West";

/** 清单的窗口标签（与后端 TODO_STICKY_LABEL 一致）：打开便签时后端据此把便签放在清单旁边。 */
const TODO_STICKY_LABEL = "sticky-todo-list";
/** 勾选后先显示勾与划线，过这么久再标记完成并从清单中淡出（期间再点一次可撤销）。 */
const COMPLETE_DELAY_MS = 900;
/** 与样式中 .todo-row-leave-active 的时长一致。 */
const LEAVE_MS = 360;
/** 指针移动超过这个距离才算拖动（否则是点击、双击）。 */
const DRAG_THRESHOLD = 5;
/** 拖到列表上下边缘这个范围内时自动滚动。 */
const AUTO_SCROLL_EDGE = 24;

const windowRef = getCurrentWindow();
const tasks = ref<Task[]>([]);
const loaded = ref(false);
const pinned = ref(false);
const draft = ref("");
const saving = ref(false);
const message = ref("");
const messageIsError = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);
const checked = ref(new Set<string>());
const completeTimers = new Map<string, number>();
const submitting = new Set<string>();
const color = ref("");
const colorMenuOpen = ref(false);
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
  if (!state) return;
  pinned.value = Boolean(state.pinned);
  color.value = state.color ?? "";
};

// 清单颜色（v2.2.5）：与便签相同，在 html/body 上加 sticky-color-<颜色> 覆盖纸面颜色令牌。
const applyColorClass = (value: string) => {
  for (const element of [document.documentElement, document.body]) {
    for (const option of STICKY_COLOR_OPTIONS) {
      element.classList.remove(`sticky-color-${option.value}`);
    }
    element.classList.add(stickyColorClass(value));
  }
};
watch(color, value => applyColorClass(value), { immediate: true });

const chooseColor = async (value: string) => {
  colorMenuOpen.value = false;
  if (resolveStickyColor(color.value) === value) return;
  const previous = color.value;
  color.value = value;
  try {
    applyState(await api.setTodoStickyColor(value));
  } catch (error) {
    color.value = previous;
    showMessage(`设置颜色失败：${errorMessage(error)}`, true);
  }
};

const closeColorMenu = () => {
  colorMenuOpen.value = false;
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

/**
 * 打开待办的便签；focus 为真时取得焦点并把光标放进正文（添加后补充内容）。
 * 便签总是放在清单旁边（v2.2.7，后端按右、左、下、上找有空间的一侧），清单锚定时便签也锚定。
 */
const openTask = async (task: Task, focus = false) => {
  try {
    await api.openStickyNote({
      taskId: task.id,
      title: task.description,
      besideWindow: TODO_STICKY_LABEL,
      ...(focus ? { focus: true } : {}),
    });
  } catch (error) {
    showMessage(`打开便签失败：${errorMessage(error)}`, true);
  }
};

/** 双击打开（v2.2.5 起单击不再打开，避免拖动、勾选时误开）。 */
const openItem = (item: TodoStickyItem) => {
  if (drag.active) return;
  void openTask(item.task);
};

const setChecked = (id: string, value: boolean) => {
  const next = new Set(checked.value);
  if (value) next.add(id);
  else next.delete(id);
  checked.value = next;
};

/**
 * 勾选完成（v2.2.5）：先显示勾与划线，COMPLETE_DELAY_MS 后再标记完成并淡出；
 * 这期间再点一次撤销。
 */
const toggleCheck = (item: TodoStickyItem) => {
  const id = item.task.id;
  if (submitting.has(id)) return;
  const timer = completeTimers.get(id);
  if (timer !== undefined) {
    window.clearTimeout(timer);
    completeTimers.delete(id);
    setChecked(id, false);
    return;
  }
  setChecked(id, true);
  completeTimers.set(id, window.setTimeout(() => void finishComplete(id), COMPLETE_DELAY_MS));
};

const finishComplete = async (id: string) => {
  completeTimers.delete(id);
  submitting.add(id);
  try {
    await api.completeTask(id);
    // 从列表中移除后由 TransitionGroup 播放淡出、收起动画。
    tasks.value = tasks.value.filter(task => task.id !== id);
    window.setTimeout(() => setChecked(id, false), LEAVE_MS);
  } catch (error) {
    setChecked(id, false);
    showMessage(`标记完成失败：${errorMessage(error)}`, true);
  } finally {
    submitting.delete(id);
  }
};

/** 加急 / 取消加急（v2.2.5）。 */
const toggleUrgent = async (item: TodoStickyItem) => {
  const id = item.task.id;
  const urgent = !item.urgent;
  const patch = (value: boolean) => {
    tasks.value = tasks.value.map(task => (task.id === id ? { ...task, urgent: value } : task));
  };
  patch(urgent);
  try {
    await api.setTaskUrgent(id, urgent);
  } catch (error) {
    patch(!urgent);
    showMessage(`${urgent ? "加急" : "取消加急"}失败：${errorMessage(error)}`, true);
  }
};

// 拖拽排序（v2.2.5）：用鼠标事件实现（Windows 上 WebView 的原生拖放会被窗口的文件拖放接管），
// 写入与待办页“手动顺序”共用的 sortOrder。
const drag = reactive({ id: "", startY: 0, active: false, dropIndex: -1 });

/** 插入线的位置；插回原处（自己前后）时不显示。 */
const dropMarker = computed(() => {
  if (!drag.active || drag.dropIndex < 0) return -1;
  const from = items.value.findIndex(item => item.task.id === drag.id);
  return drag.dropIndex === from || drag.dropIndex === from + 1 ? -1 : drag.dropIndex;
});

const listElement = () => document.querySelector<HTMLElement>(".todo-sticky-list");

const rowRects = () =>
  Array.from(listElement()?.querySelectorAll<HTMLElement>(".todo-sticky-row:not(.todo-row-leave-active)") ?? []).map(row => {
    const rect = row.getBoundingClientRect();
    return { top: rect.top, height: rect.height };
  });

const onDragMove = (event: MouseEvent) => {
  if (!drag.active) {
    if (Math.abs(event.clientY - drag.startY) < DRAG_THRESHOLD) return;
    drag.active = true;
  }
  event.preventDefault();
  const list = listElement();
  if (list) {
    const rect = list.getBoundingClientRect();
    if (event.clientY < rect.top + AUTO_SCROLL_EDGE) list.scrollTop -= 8;
    else if (event.clientY > rect.bottom - AUTO_SCROLL_EDGE) list.scrollTop += 8;
  }
  drag.dropIndex = dropIndexAt(rowRects(), event.clientY);
};

const stopDragListeners = () => {
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragEnd);
};

const onDragEnd = () => {
  stopDragListeners();
  const { id, active } = drag;
  const target = dropMarker.value;
  // 松开后的 dblclick / click 仍能看到 active，下一帧再清除。
  requestAnimationFrame(() => {
    drag.active = false;
    drag.id = "";
    drag.dropIndex = -1;
  });
  if (active && target >= 0) void reorder(id, target);
};

const startDrag = (event: MouseEvent, item: TodoStickyItem) => {
  if (event.button !== 0 || checked.value.has(item.task.id)) return;
  stopDragListeners();
  drag.id = item.task.id;
  drag.startY = event.clientY;
  drag.active = false;
  drag.dropIndex = -1;
  window.addEventListener("mousemove", onDragMove);
  window.addEventListener("mouseup", onDragEnd);
};

const reorder = async (id: string, toIndex: number) => {
  const plan = planReorder(
    items.value.map(item => ({ id: item.task.id, sortOrder: item.task.sortOrder })),
    id,
    toIndex
  );
  if (!plan.length) return;
  const orders = new Map(plan.map(entry => [entry.id, entry.sortOrder]));
  const previous = tasks.value;
  tasks.value = tasks.value.map(task => (orders.has(task.id) ? { ...task, sortOrder: orders.get(task.id) } : task));
  try {
    await api.setTaskOrder(plan);
  } catch (error) {
    tasks.value = previous;
    showMessage(`调整顺序失败：${errorMessage(error)}`, true);
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
  let opened = false;
  try {
    let created: Task | null = null;
    if (result.kind === "recurring") {
      await api.createRecurringTask(result.payload);
    } else {
      created = await api.quickAddTask(result.payload);
    }
    draft.value = "";
    showMessage(result.hint);
    await loadTasks();
    // v2.2.5：添加后打开这条待办的便签，光标在正文中，方便补充内容（循环提醒没有便签）。
    if (created) {
      opened = true;
      await openTask(created, true);
    }
  } catch (error) {
    showMessage(errorMessage(error), true);
  } finally {
    saving.value = false;
    if (!opened) requestAnimationFrame(() => inputRef.value?.focus());
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
  window.addEventListener("mousedown", closeColorMenu);
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
  window.removeEventListener("mousedown", closeColorMenu);
  stopDragListeners();
  completeTimers.forEach(timer => window.clearTimeout(timer));
  window.clearTimeout(messageTimer);
  window.clearInterval(clockTimer);
});
</script>

<style scoped>
/* 标题栏（含颜色菜单）盖在列表行之上：行带 position: relative（插入线）。 */
.todo-sticky-header {
  z-index: 3;
}

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
  position: relative;
  display: flex;
  align-items: center;
  gap: calc(8px * var(--sticky-ui-scale));
  min-height: calc(32px * var(--sticky-ui-scale));
  padding: 0 calc(6px * var(--sticky-ui-scale)) 0 calc(8px * var(--sticky-ui-scale));
  border-radius: calc(8px * var(--sticky-ui-scale));
  color: var(--sticky-text);
  font-size: calc(13px * var(--sticky-ui-scale));
  cursor: default;
  user-select: none;
  transition: background-color 0.15s ease, opacity 0.25s ease, box-shadow 0.15s ease;
}

.todo-sticky-row:hover {
  background: var(--sticky-control-hover);
}

.todo-sticky-list.is-dragging,
.todo-sticky-list.is-dragging .todo-sticky-row {
  cursor: grabbing;
}

.todo-sticky-list.is-dragging .todo-sticky-row:not(.is-drag-source):hover {
  background: transparent;
}

.todo-sticky-row.is-drag-source {
  opacity: 0.45;
  background: var(--sticky-control-hover);
}

/* 拖动时的插入线。 */
.todo-sticky-row.is-drop-before::before,
.todo-sticky-row.is-drop-after::after {
  content: "";
  position: absolute;
  left: calc(6px * var(--sticky-ui-scale));
  right: calc(6px * var(--sticky-ui-scale));
  height: 2px;
  border-radius: 1px;
  background: var(--primary);
  pointer-events: none;
}

.todo-sticky-row.is-drop-before::before {
  top: -1px;
}

.todo-sticky-row.is-drop-after::after {
  bottom: -1px;
}

/* 方形勾选框（v2.2.5）：勾选后先显示勾与划线，再淡出。 */
.todo-sticky-check {
  flex-shrink: 0;
  width: calc(16px * var(--sticky-ui-scale));
  height: calc(16px * var(--sticky-ui-scale));
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1.5px solid var(--sticky-text-faint);
  border-radius: calc(4px * var(--sticky-ui-scale));
  background: transparent;
  color: transparent;
  cursor: pointer;
  transition: border-color 0.15s ease, background-color 0.15s ease, color 0.15s ease;
}

.todo-sticky-check svg {
  width: 72%;
  height: 72%;
  stroke-dasharray: 24;
  stroke-dashoffset: 24;
  transition: stroke-dashoffset 0.28s ease 0.05s;
}

.todo-sticky-check:hover,
.todo-sticky-check:focus-visible {
  border-color: var(--success);
  outline: none;
}

.todo-sticky-check:hover svg {
  color: var(--success-text);
  stroke-dashoffset: 12;
  opacity: 0.5;
}

.todo-sticky-row.is-checked .todo-sticky-check {
  border-color: var(--success);
  background: var(--success);
  color: var(--urgent-on);
  animation: todo-check-pop 0.32s ease;
}

.todo-sticky-row.is-checked .todo-sticky-check svg {
  stroke-dashoffset: 0;
  opacity: 1;
}

.todo-sticky-text {
  /* 只占文字宽度，划线不会延伸到时间前；时间与加急按钮由 margin-left: auto 推到右侧。 */
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  /* 划线用背景画出，勾选后从左到右划过。 */
  background-image: linear-gradient(currentColor, currentColor);
  background-repeat: no-repeat;
  background-position: left 55%;
  background-size: 0 1px;
  transition: background-size 0.4s ease, color 0.3s ease;
}

.todo-sticky-row.is-checked .todo-sticky-text {
  background-size: 100% 1px;
  color: var(--sticky-text-faint);
}

.todo-sticky-row.is-checked .todo-sticky-when,
.todo-sticky-row.is-checked .todo-sticky-urgent-badge {
  opacity: 0.5;
}

.todo-sticky-text + .todo-sticky-when,
.todo-sticky-text + .todo-sticky-urgent-toggle {
  margin-left: auto;
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

/* 加急（v2.2.5）：红色底、左侧色条、“急”字徽标与加粗标题。 */
.todo-sticky-row.is-urgent {
  background: var(--urgent-soft);
  box-shadow: inset calc(3px * var(--sticky-ui-scale)) 0 0 var(--urgent);
}

.todo-sticky-row.is-urgent:hover {
  background: var(--urgent-soft);
  box-shadow: inset calc(3px * var(--sticky-ui-scale)) 0 0 var(--urgent), inset 0 0 0 1px var(--urgent-glow);
}

.todo-sticky-row.is-urgent + .todo-sticky-row.is-urgent {
  margin-top: 2px;
}

.todo-sticky-row.is-urgent:not(.is-checked) .todo-sticky-text {
  color: var(--urgent-text);
  font-weight: 650;
}

.todo-sticky-urgent-badge {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: calc(18px * var(--sticky-ui-scale));
  height: calc(18px * var(--sticky-ui-scale));
  padding: 0 calc(4px * var(--sticky-ui-scale));
  border-radius: calc(5px * var(--sticky-ui-scale));
  background: var(--urgent);
  color: var(--urgent-on);
  font-size: calc(11px * var(--sticky-ui-scale));
  font-weight: 700;
  line-height: 1;
  animation: todo-urgent-pulse 2.4s ease-in-out infinite;
}

.todo-sticky-urgent-toggle {
  flex-shrink: 0;
  width: calc(22px * var(--sticky-ui-scale));
  height: calc(22px * var(--sticky-ui-scale));
  margin-left: calc(-4px * var(--sticky-ui-scale));
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: calc(6px * var(--sticky-ui-scale));
  background: transparent;
  color: var(--sticky-text-faint);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s ease, color 0.15s ease, background-color 0.15s ease;
}

.todo-sticky-urgent-toggle svg {
  width: calc(14px * var(--sticky-ui-scale));
  height: calc(14px * var(--sticky-ui-scale));
}

.todo-sticky-row:hover .todo-sticky-urgent-toggle,
.todo-sticky-urgent-toggle:focus-visible {
  opacity: 1;
}

.todo-sticky-urgent-toggle:hover,
.todo-sticky-urgent-toggle:focus-visible {
  color: var(--urgent-text);
  background: var(--urgent-soft);
  outline: none;
}

.todo-sticky-urgent-toggle.active {
  color: var(--urgent-text);
}

.todo-sticky-urgent-toggle.active svg {
  fill: currentColor;
}

.todo-sticky-list.is-dragging .todo-sticky-urgent-toggle {
  opacity: 0;
}

/* 行的进出与换位动画：完成后淡出并收起，新添加的淡入，拖动排序后平移到新位置。 */
.todo-row-move {
  transition: transform 0.22s ease;
}

.todo-row-enter-active {
  transition: opacity 0.22s ease, transform 0.22s ease;
}

.todo-row-enter-from {
  opacity: 0;
  transform: translateY(-4px);
}

.todo-row-leave-active {
  overflow: hidden;
  transition: opacity 0.36s ease, transform 0.36s ease, max-height 0.36s ease, min-height 0.36s ease,
    margin 0.36s ease;
}

.todo-row-leave-from {
  max-height: calc(48px * var(--sticky-ui-scale));
}

.todo-row-leave-to {
  opacity: 0;
  transform: translateX(calc(24px * var(--sticky-ui-scale)));
  max-height: 0;
  min-height: 0;
  margin: 0;
}

@keyframes todo-check-pop {
  0% {
    transform: scale(1);
  }
  45% {
    transform: scale(1.22);
  }
  100% {
    transform: scale(1);
  }
}

@keyframes todo-urgent-pulse {
  0%,
  100% {
    box-shadow: 0 0 0 0 var(--urgent-glow);
  }
  50% {
    box-shadow: 0 0 0 calc(3px * var(--sticky-ui-scale)) transparent;
  }
}

@media (prefers-reduced-motion: reduce) {
  .todo-sticky-urgent-badge,
  .todo-sticky-row.is-checked .todo-sticky-check {
    animation: none;
  }
}

/* 清单颜色：选择菜单在标题栏下方展开，靠右对齐。 */
.todo-sticky-color {
  margin-right: 0;
}

.todo-sticky-color-trigger.is-open {
  opacity: 1;
  background: var(--sticky-control-hover);
}

.todo-sticky-color-menu {
  top: calc(100% + 6px);
  bottom: auto;
  left: auto;
  right: 0;
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
