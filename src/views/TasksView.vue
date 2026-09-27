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
        <input class="input" v-model="filter.query" placeholder="搜索标题、描述或 #标签" />
        <button v-if="filter.query" class="search-clear" type="button" title="清空" @click="filter.query = ''">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M7 7l10 10M17 7L7 17" />
          </svg>
        </button>
      </div>
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
    </div>
    <div class="table-card">
      <div class="table-scroll table-scroll-no-x">
        <table class="table tasks-table">
          <thead>
            <tr>
              <th class="col-select">完成</th>
              <th class="col-desc">标题</th>
              <th class="col-note">描述</th>
              <th class="col-datetime">提醒时间</th>
              <th class="col-datetime">创建时间</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="task in tasksPage"
              :key="task.id"
              class="table-row"
              @dblclick="openTaskEditor(task)"
              @contextmenu.prevent.stop="openTaskMenu($event, task)"
            >
              <td class="col-select">
                <input
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
                  <TaskBadges :task="task" clickable :active-tag="filter.tag" @select-tag="toggleTagFilter" />
                </div>
              </td>
              <td class="col-note cell-muted" :class="{ 'cell-empty': !task.stickyContent?.trim() }" :title="taskStickyPreview(task.stickyContent)">{{ taskStickyPreview(task.stickyContent) }}</td>
              <td class="col-datetime" :title="formatDateTime(task.reminderTime)">
                <span v-if="task.reminderTime" class="time-chip" :class="reminderTone(task.reminderTime)">
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
import { computed, reactive, ref, watch } from "vue";
import MarkdownNoteEditor from "../components/MarkdownNoteEditor.vue";
import Pagination from "../components/Pagination.vue";
import SmartParseHint from "../components/SmartParseHint.vue";
import TaskBadges from "../components/TaskBadges.vue";
import { formatDateTime, reminderTone, taskStickyPreview } from "../format";
import { safeStorage } from "../safeStorage";
import {
  PRIORITY_OPTIONS,
  TASK_SORT_OPTIONS,
  collectTags,
  filterAndSortTasks,
  type TaskFilter,
  type TaskSortKey
} from "../tasks";
import { useAppData } from "../composables/useAppData";
import { useItemActions } from "../composables/useItemActions";
import { usePagination } from "../composables/usePagination";
import { useSmartAdd } from "../composables/useSmartAdd";
import { useUiPrefs } from "../composables/useUiPrefs";

const { tasks } = useAppData();
const { toggleTask, openTaskMenu, openTaskEditor } = useItemActions();
const { isLightTheme } = useUiPrefs();

const filter = reactive<TaskFilter>({ query: "", tag: "", priority: -1 });
const storedSort = safeStorage.getItem("tasksSort");
const sortKey = ref<TaskSortKey>(
  TASK_SORT_OPTIONS.some(option => option.value === storedSort) ? (storedSort as TaskSortKey) : "created"
);
watch(sortKey, value => safeStorage.setItem("tasksSort", value));

const priorityFilterOptions = [...PRIORITY_OPTIONS].reverse();
const allTags = computed(() => collectTags(tasks.value));
const isFiltering = computed(() => Boolean(filter.query.trim() || filter.tag || filter.priority >= 0));
const visibleTasks = computed(() => filterAndSortTasks(tasks.value, filter, sortKey.value));
const filterKey = computed(() => `${filter.query}|${filter.tag}|${filter.priority}|${sortKey.value}`);

// 标签被删光后自动回到“全部标签”。
watch(allTags, list => {
  if (filter.tag && !list.some(item => item.tag.toLowerCase() === filter.tag.toLowerCase())) {
    filter.tag = "";
  }
});

const toggleTagFilter = (tag: string) => {
  filter.tag = filter.tag.toLowerCase() === tag.toLowerCase() ? "" : tag;
};

const {
  pageIndex: tasksPageIndex,
  pageSize: tasksPageSize,
  totalPages: tasksTotalPages,
  page: tasksPage
} = usePagination(visibleTasks, filterKey);

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

const handleAddTask = async () => {
  if (await submit({ stickyContent: newTaskStickyContent.value })) {
    newTaskStickyContent.value = "";
  }
};
</script>
