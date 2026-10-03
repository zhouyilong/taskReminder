<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">待办事项</div>
      <span class="section-meta">{{ isFiltering ? `筛选后 ${visibleTasks.length} / ${tasks.length} 条` : `共 ${tasks.length} 条任务` }}</span>
    </div>
    <div class="composer-card">
      <div class="composer-title-row">
        <span class="composer-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </span>
        <input
          data-shortcut="new"
          class="composer-input"
          v-model="newTaskDescription"
          placeholder="添加新任务，如“明天下午3点 交周报 #工作 !高”，回车保存"
          @keydown.enter.exact.prevent="handleAddTask"
        />
        <button class="button" :disabled="!parsedTitle || saving" @click="handleAddTask">添加任务</button>
      </div>
      <SmartParseHint
        v-model:enabled="parseEnabled"
        :parsed="parsed"
        :has-meta="hasMeta"
        :schedule-text="scheduleText"
        :error="parseError"
        :has-note="!!newTaskStickyContent.trim()"
      />
      <MarkdownNoteEditor
        ref="composerEditor"
        v-model="newTaskStickyContent"
        class="composer-editor"
        variant="ghost"
        :theme="isLightTheme ? 'light' : 'dark'"
        placeholder="补充任务描述（可选），支持 Markdown 所见即所得"
      />
    </div>
    <div class="task-toolbar">
      <div class="search-field">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="6.5" />
          <path d="M16 16l4 4" />
        </svg>
        <input data-shortcut="search" class="input" v-model="filter.query" placeholder="搜索标题、描述、#标签或 @项目" />
        <button v-if="filter.query" class="search-clear" type="button" title="清空" @click="filter.query = ''">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M7 7l10 10M17 7L7 17" />
          </svg>
        </button>
      </div>
      <select v-if="allProjects.length || filter.project !== null" class="select" v-model="filter.project" title="按项目筛选">
        <option :value="null">全部项目</option>
        <option v-for="item in allProjects" :key="item.project" :value="item.project">{{ item.project }}（{{ item.count }}）</option>
        <option value="">未分组（{{ ungroupedCount }}）</option>
      </select>
      <select class="select" v-model="filter.tag" title="按标签筛选">
        <option value="">全部标签</option>
        <option v-for="item in allTags" :key="item.tag" :value="item.tag">#{{ item.tag }}（{{ item.count }}）</option>
      </select>
      <select class="select" v-model.number="filter.priority" title="按优先级筛选">
        <option :value="-1">全部优先级</option>
        <option v-for="option in priorityFilterOptions" :key="option.value" :value="option.value">
          {{ option.value ? `${option.label}优先级` : "无优先级" }}
        </option>
      </select>
      <select class="select" v-model="sortKey" title="排序">
        <option v-for="option in TASK_SORT_OPTIONS" :key="option.value" :value="option.value">按{{ option.label }}</option>
      </select>
      <button
        class="button secondary"
        :class="{ 'is-active': selectionMode }"
        title="多选后批量完成、删除、加标签、移到项目、改优先级或提醒时间（也可按住 Ctrl 点击行）"
        @click="selectionMode ? exitSelection() : (selectionMode = true)"
      >
        {{ selectionMode ? "退出多选" : "多选" }}
      </button>
    </div>
    <div v-if="canReorder" class="reorder-hint">拖动行调整顺序；切换到其他排序方式时不影响手动顺序。</div>
    <TaskBatchBar v-if="selectionMode" :ids="selectedIds" @done="exitSelection" @cancel="exitSelection" />
    <div class="table-card">
      <div class="table-scroll table-scroll-no-x">
        <table class="table tasks-table" :class="{ 'is-selecting': selectionMode }">
          <thead>
            <tr>
              <th class="col-select">
                <input
                  v-if="selectionMode"
                  type="checkbox"
                  title="全选本页"
                  :checked="pageAllSelected"
                  :indeterminate.prop="pageSomeSelected && !pageAllSelected"
                  @change="selection = toggleAll(selection, pageIds)"
                />
                <template v-else>完成</template>
              </th>
              <th class="col-desc">标题</th>
              <th class="col-note">描述</th>
              <th class="col-datetime">截止 / 提醒</th>
              <th class="col-datetime">创建时间</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(task, rowIndex) in tasksPage"
              :key="task.id"
              class="table-row"
              :class="{
                'is-selected': selection.ids.has(task.id),
                'is-draggable': canReorder,
                'is-dragging': draggingId === task.id,
                'is-drop-before': dropIndex === pageStart + rowIndex,
                'is-drop-after': rowIndex === tasksPage.length - 1 && dropIndex === pageStart + rowIndex + 1
              }"
              :draggable="canReorder"
              @dragstart="handleDragStart($event, task.id)"
              @dragover.prevent="handleDragOver($event, rowIndex)"
              @drop.prevent="handleDrop"
              @dragend="resetDrag"
              @click="handleRowClick($event, task.id)"
              @dblclick="openTaskEditor(task)"
              @contextmenu.prevent.stop="openTaskMenu($event, task)"
            >
              <td class="col-select">
                <input
                  v-if="selectionMode"
                  type="checkbox"
                  title="选择（Shift 连选）"
                  :checked="selection.ids.has(task.id)"
                  @click.stop="handleRowClick($event, task.id, true)"
                />
                <input
                  v-else
                  type="checkbox"
                  class="check-round"
                  title="标记完成"
                  :checked="task.status === 'COMPLETED'"
                  @change="toggleTask(task)"
                />
              </td>
              <td class="col-desc cell-title" :title="task.description">
                <div class="task-title-cell">
                  <span class="task-title-text">{{ task.description }}</span>
                  <TaskBadges
                    :task="task"
                    clickable
                    :active-tag="filter.tag"
                    :active-project="filter.project"
                    @select-tag="toggleTagFilter"
                    @select-project="toggleProjectFilter"
                  />
                </div>
              </td>
              <td class="col-note cell-muted" :class="{ 'cell-empty': !task.stickyContent?.trim() }" :title="taskStickyPreview(task.stickyContent)">
                <span class="note-cell-with-progress">
                  <ChecklistProgress :content="task.stickyContent" />
                  <span class="note-cell-text">{{ taskStickyPreview(task.stickyContent) }}</span>
                </span>
              </td>
              <td class="col-datetime" :title="timeTitle(task)">
                <div v-if="task.dueAt" class="due-cell">
                  <span class="time-chip is-due" :class="reminderTone(task.dueAt)">
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                      <path d="M6 4v16M6 5h11l-2.5 4L17 13H6" />
                    </svg>
                    截止 {{ formatDateTime(task.dueAt).slice(5, 16) }}
                  </span>
                  <span v-if="separateReminder(task)" class="due-reminder-note">
                    {{ task.reminderTime ? `提醒 ${formatDateTime(task.reminderTime).slice(5, 16)}` : "" }}
                  </span>
                </div>
                <span v-else-if="task.reminderTime" class="time-chip" :class="reminderTone(task.reminderTime)">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <circle cx="12" cy="12" r="8" />
                    <path d="M12 8v4l2.5 1.5" />
                  </svg>
                  {{ formatDateTime(task.reminderTime) }}
                </span>
                <span v-else class="cell-empty">未设置</span>
              </td>
              <td class="col-datetime cell-time" :title="formatDateTime(task.createdAt)">{{ formatDateTime(task.createdAt) }}</td>
            </tr>
            <tr v-if="!tasksPage.length" class="table-empty-row">
              <td colspan="5">
                <div class="table-empty">
                  <template v-if="isFiltering && tasks.length">
                    <span class="table-empty-title">没有符合条件的待办</span>
                    <span class="table-empty-hint">调整搜索或筛选条件试试</span>
                  </template>
                  <template v-else>
                    <span class="table-empty-title">暂无待办任务</span>
                    <span class="table-empty-hint">在上方输入标题即可快速添加，支持“明天9点”“每周五 17:30”等时间写法</span>
                  </template>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <Pagination :total="visibleTasks.length" :total-pages="tasksTotalPages" v-model:page-index="tasksPageIndex" v-model:page-size="tasksPageSize" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import ChecklistProgress from "../components/ChecklistProgress.vue";
import MarkdownNoteEditor from "../components/MarkdownNoteEditor.vue";
import Pagination from "../components/Pagination.vue";
import SmartParseHint from "../components/SmartParseHint.vue";
import TaskBatchBar from "../components/TaskBatchBar.vue";
import TaskBadges from "../components/TaskBadges.vue";
import { formatDateTime, reminderTone, taskStickyPreview } from "../format";
import { separateReminder } from "../due";
import type { Task } from "../types";
import { VIEW_SHORTCUT_EVENT, type ShortcutAction } from "../keyboard";
import { safeStorage } from "../safeStorage";
import { api } from "../api";
import { emptySelection, pruneSelection, selectRange, toggleAll, toggleSelected } from "../selection";
import { planReorder } from "../reorder";
import {
  PRIORITY_OPTIONS,
  TASK_SORT_OPTIONS,
  collectProjects,
  collectTags,
  filterAndSortTasks,
  type TaskFilter,
  type TaskSortKey
} from "../tasks";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";
import { useItemActions } from "../composables/useItemActions";
import { usePagination } from "../composables/usePagination";
import { useSmartAdd } from "../composables/useSmartAdd";
import { useUiPrefs } from "../composables/useUiPrefs";

const { tasks, refreshAll } = useAppData();
const { toggleTask, openTaskMenu, openTaskEditor } = useItemActions();
const { isLightTheme } = useUiPrefs();
const { confirmAction } = useDialogs();

const filter = reactive<TaskFilter>({ query: "", tag: "", priority: -1, project: null });
const storedSort = safeStorage.getItem("tasksSort");
const sortKey = ref<TaskSortKey>(
  TASK_SORT_OPTIONS.some(option => option.value === storedSort) ? (storedSort as TaskSortKey) : "created"
);
watch(sortKey, value => safeStorage.setItem("tasksSort", value));

const priorityFilterOptions = [...PRIORITY_OPTIONS].reverse();
const allTags = computed(() => collectTags(tasks.value));
// 项目（v2.2）：null 为全部项目，空字符串为未分组。
const allProjects = computed(() => collectProjects(tasks.value));
const ungroupedCount = computed(() => tasks.value.filter(task => !task.project).length);
const isFiltering = computed(() =>
  Boolean(filter.query.trim() || filter.tag || filter.priority >= 0 || filter.project !== null)
);
const visibleTasks = computed(() => filterAndSortTasks(tasks.value, filter, sortKey.value));
const filterKey = computed(
  () => `${filter.query}|${filter.tag}|${filter.priority}|${filter.project ?? "\u0000all"}|${sortKey.value}`
);

// 标签被删光后自动回到“全部标签”。
watch(allTags, list => {
  if (filter.tag && !list.some(item => item.tag.toLowerCase() === filter.tag.toLowerCase())) {
    filter.tag = "";
  }
});

const timeTitle = (task: Task) =>
  [
    task.dueAt ? `截止 ${formatDateTime(task.dueAt)}` : "",
    task.reminderTime ? `提醒 ${formatDateTime(task.reminderTime)}` : task.dueAt ? "不提醒" : ""
  ]
    .filter(Boolean)
    .join("，") || "未设置";

const toggleTagFilter = (tag: string) => {
  filter.tag = filter.tag.toLowerCase() === tag.toLowerCase() ? "" : tag;
};

const toggleProjectFilter = (project: string) => {
  filter.project = filter.project?.toLowerCase() === project.toLowerCase() ? null : project;
};

// 项目被清空后自动回到“全部项目”（“未分组”保留）。
watch(allProjects, list => {
  if (filter.project && !list.some(item => item.project.toLowerCase() === filter.project?.toLowerCase())) {
    filter.project = null;
  }
});

const {
  pageIndex: tasksPageIndex,
  pageSize: tasksPageSize,
  totalPages: tasksTotalPages,
  page: tasksPage
} = usePagination(visibleTasks, filterKey);

// 拖拽排序（v2.1）：仅在“手动顺序”且不在多选时可用；dropIndex 为在完整列表中的插入位置。
const canReorder = computed(() => sortKey.value === "manual" && !selectionMode.value);
const pageStart = computed(() => (tasksPageIndex.value - 1) * tasksPageSize.value);
const draggingId = ref("");
const dropIndex = ref<number | null>(null);

const resetDrag = () => {
  draggingId.value = "";
  dropIndex.value = null;
};

const handleDragStart = (event: DragEvent, id: string) => {
  if (!canReorder.value) return;
  draggingId.value = id;
  event.dataTransfer?.setData("text/plain", id);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
};

const handleDragOver = (event: DragEvent, rowIndex: number) => {
  if (!draggingId.value) return;
  const row = event.currentTarget as HTMLElement;
  const rect = row.getBoundingClientRect();
  const after = event.clientY > rect.top + rect.height / 2;
  dropIndex.value = pageStart.value + rowIndex + (after ? 1 : 0);
};

const handleDrop = async () => {
  const id = draggingId.value;
  const index = dropIndex.value;
  resetDrag();
  if (!id || index === null) return;
  const updates = planReorder(visibleTasks.value, id, index);
  if (!updates.length) return;
  try {
    await api.setTaskOrder(updates);
  } catch (error) {
    console.error("[tasks] 调整顺序失败", error);
  }
  await refreshAll();
};

// 多选：进入多选模式后点击行切换选择、Shift 连选；Ctrl / ⌘ 点击行可直接进入多选。
const selectionMode = ref(false);
const selection = ref(emptySelection());
const selectedIds = computed(() => [...selection.value.ids]);
const pageIds = computed(() => tasksPage.value.map(task => task.id));
const pageAllSelected = computed(
  () => pageIds.value.length > 0 && pageIds.value.every(id => selection.value.ids.has(id))
);
const pageSomeSelected = computed(() => pageIds.value.some(id => selection.value.ids.has(id)));

const exitSelection = () => {
  selectionMode.value = false;
  selection.value = emptySelection();
};

const handleRowClick = (event: MouseEvent, id: string, fromCheckbox = false) => {
  if (!selectionMode.value) {
    if (!(event.ctrlKey || event.metaKey)) return;
    selectionMode.value = true;
  } else if (!fromCheckbox && (event.target as HTMLElement | null)?.closest("a, button, .task-tag")) {
    return;
  }
  selection.value = event.shiftKey
    ? selectRange(selection.value, visibleTasks.value.map(task => task.id), id)
    : toggleSelected(selection.value, id);
};

watch(visibleTasks, list => {
  selection.value = pruneSelection(selection.value, list.map(task => task.id));
});

// 快捷键：Esc 退出多选；Delete 删除选中的待办（需确认）。
const handleViewShortcut = (event: Event) => {
  const action = (event as CustomEvent<ShortcutAction>).detail;
  if (action.type === "escape" && selectionMode.value) {
    exitSelection();
  } else if (action.type === "delete" && selectedIds.value.length) {
    const ids = [...selectedIds.value];
    confirmAction({
      message: `确定要删除选中的 ${ids.length} 项待办吗？删除后可在回收站中恢复。`,
      action: async () => {
        await api.batchUpdateTasks(ids, { action: "delete" });
        exitSelection();
        await refreshAll();
      }
    });
  }
};
onMounted(() => window.addEventListener(VIEW_SHORTCUT_EVENT, handleViewShortcut));
onBeforeUnmount(() => window.removeEventListener(VIEW_SHORTCUT_EVENT, handleViewShortcut));

const newTaskDescription = ref("");
const newTaskStickyContent = ref("");
const {
  enabled: parseEnabled,
  error: parseError,
  saving,
  parsed,
  hasMeta,
  scheduleText,
  title: parsedTitle,
  submit
} = useSmartAdd(newTaskDescription);

const composerEditor = ref<InstanceType<typeof MarkdownNoteEditor> | null>(null);

const handleAddTask = async () => {
  // 编辑器内容变更有 200ms 防抖：提交前先取当前内容，避免丢掉最后的输入。
  composerEditor.value?.flush();
  if (await submit({ stickyContent: newTaskStickyContent.value })) {
    newTaskStickyContent.value = "";
  }
};
</script>
