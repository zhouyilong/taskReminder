<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">循环提醒</div>
      <span class="section-meta">共 {{ recurringTasks.length }} 条配置</span>
    </div>
    <div class="form-card">
      <div class="form-row compact">
        <label class="field-label">描述</label>
        <input data-shortcut="new" class="input" v-model="newRecurring.description" placeholder="输入提醒描述" style="flex: 1" />
        <label class="field-label">模式</label>
        <select class="select" v-model="newRecurring.mode" style="width: 140px">
          <option v-for="mode in recurringModeOptions" :key="mode.value" :value="mode.value">{{ mode.label }}</option>
        </select>
        <button class="button" @click="handleAddRecurring">添加提醒</button>
      </div>
      <div class="form-row compact">
        <RecurringFields :draft="newRecurring" />
      </div>
      <div class="form-row compact">
        <label class="field-label">标签</label>
        <TagInput v-model="newRecurring.tags" :suggestions="tagSuggestions" />
      </div>
    </div>
    <div class="task-toolbar">
      <div class="search-field">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="6.5" />
          <path d="M16 16l4 4" />
        </svg>
        <input data-shortcut="search" class="input" v-model="keyword" placeholder="搜索描述、规则或 #标签" />
        <button v-if="keyword" class="search-clear" type="button" title="清空" @click="keyword = ''">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M7 7l10 10M17 7L7 17" />
          </svg>
        </button>
      </div>
      <select v-if="recurringTagOptions.length" class="select" v-model="tagFilter" title="按标签筛选">
        <option value="">全部标签</option>
        <option v-for="item in recurringTagOptions" :key="item.tag" :value="item.tag">#{{ item.tag }}（{{ item.count }}）</option>
      </select>
      <span v-if="tagFilter || keyword.trim()" class="section-meta">筛选后 {{ visibleRecurring.length }} / {{ recurringTasks.length }} 条</span>
    </div>
    <div class="table-card">
      <div class="table-scroll">
        <table class="table recurring-table">
          <thead>
            <tr>
              <th class="col-desc">描述</th>
              <th class="col-mode">模式</th>
              <th class="col-rule">规则</th>
              <th class="col-datetime col-next-trigger">下次触发</th>
              <th class="col-status">状态</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="task in recurringPage"
              :key="task.id"
              class="table-row"
              @dblclick="openRecurringEditor(task)"
              @contextmenu.prevent.stop="openRecurringMenu($event, task)"
            >
              <td class="col-desc cell-title" :title="task.description">
                <div class="task-title-cell">
                  <span class="task-title-text">{{ task.description }}</span>
                  <TaskBadges :task="task" clickable :active-tag="tagFilter" @select-tag="toggleTagFilter" />
                </div>
              </td>
              <td class="col-mode" :title="formatRecurringMode(task.repeatMode)">
                <span class="chip">{{ formatRecurringMode(task.repeatMode) }}</span>
              </td>
              <td class="col-rule cell-muted" :title="workdayHolidayWarning(task, holidayYears) || formatRecurringRule(task)">
                {{ formatRecurringRule(task) }}
                <span v-if="workdayHolidayWarning(task, holidayYears)" class="chip is-warning">节假日待更新</span>
              </td>
              <td class="col-datetime col-next-trigger cell-time" :title="formatDateTime(task.nextTrigger)">{{ formatDateTime(task.nextTrigger) }}</td>
              <td class="col-status" :title="task.isPaused ? '已暂停' : '运行中'">
                <span
                  v-if="!isSupportedRecurringMode(task.repeatMode)"
                  class="status-pill is-unsupported"
                  title="来自更新版本的循环模式，本机不会提醒；升级应用后恢复"
                >需升级</span>
                <span v-else class="status-pill" :class="task.isPaused ? 'is-paused' : 'is-running'">{{ task.isPaused ? "已暂停" : "运行中" }}</span>
              </td>
            </tr>
            <tr v-if="!recurringPage.length" class="table-empty-row">
              <td colspan="5">
                <div class="table-empty">
                  <span class="table-empty-title">暂无循环提醒</span>
                  <span class="table-empty-hint">在上方设置描述与规则后添加</span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <Pagination :total="visibleRecurring.length" :total-pages="recurringTotalPages" v-model:page-index="recurringPageIndex" v-model:page-size="recurringPageSize" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import Pagination from "../components/Pagination.vue";
import RecurringFields from "../components/RecurringFields.vue";
import TagInput from "../components/TagInput.vue";
import TaskBadges from "../components/TaskBadges.vue";
import { collectTags } from "../tasks";
import { api } from "../api";
import { formatDateTime } from "../format";
import {
  buildRecurringPayload,
  createRecurringDraft,
  formatRecurringMode,
  formatRecurringRule,
  isSupportedRecurringMode,
  recurringModeOptions,
  validateRecurringDraft,
  workdayHolidayWarning
} from "../recurring";
import { matchesKeyword, parseQuery } from "../search";
import { useAppData } from "../composables/useAppData";
import { useItemActions } from "../composables/useItemActions";
import { usePagination } from "../composables/usePagination";

const { tasks, completedTasks, recurringTasks, holidayYears, refreshAll } = useAppData();

// 标签（v2.1）：筛选与新建时的建议（同时包含待办的标签）。
const tagFilter = ref("");
const recurringTagOptions = computed(() => collectTags(recurringTasks.value));
const tagSuggestions = computed(() =>
  collectTags([...recurringTasks.value, ...tasks.value, ...completedTasks.value]).map(item => item.tag)
);
const keyword = ref("");
const visibleRecurring = computed(() => {
  const tag = tagFilter.value.toLowerCase();
  const query = parseQuery(keyword.value);
  return recurringTasks.value.filter(task => {
    if (tag && !(task.tags ?? []).some(item => item.toLowerCase() === tag)) return false;
    return matchesKeyword(
      { text: [task.description, formatRecurringMode(task.repeatMode), formatRecurringRule(task)], tags: task.tags },
      query
    );
  });
});
const paginationKey = computed(() => `${tagFilter.value}\n${keyword.value}`);
const toggleTagFilter = (tag: string) => {
  tagFilter.value = tagFilter.value.toLowerCase() === tag.toLowerCase() ? "" : tag;
};
watch(recurringTagOptions, list => {
  if (tagFilter.value && !list.some(item => item.tag.toLowerCase() === tagFilter.value.toLowerCase())) {
    tagFilter.value = "";
  }
});
const { openRecurringMenu, openRecurringEditor } = useItemActions();
const {
  pageIndex: recurringPageIndex,
  pageSize: recurringPageSize,
  totalPages: recurringTotalPages,
  page: recurringPage
} = usePagination(visibleRecurring, paginationKey);

const newRecurring = reactive(createRecurringDraft());

const handleAddRecurring = async () => {
  if (!newRecurring.description.trim()) {
    return;
  }
  const error = validateRecurringDraft(newRecurring);
  if (error) {
    alert(error);
    return;
  }
  await api.createRecurringTask(buildRecurringPayload(newRecurring));
  Object.assign(newRecurring, createRecurringDraft());
  await refreshAll();
};
</script>
