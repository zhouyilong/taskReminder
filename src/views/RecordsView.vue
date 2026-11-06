<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">提醒记录</div>
      <span class="section-meta">筛选后 {{ filteredRecords.length }} 条</span>
    </div>
    <div class="search-field">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="6.5" />
          <path d="M16 16l4 4" />
        </svg>
        <input data-shortcut="search" class="input" v-model="recordKeyword" placeholder="搜索提醒描述" />
        <button v-if="recordKeyword" class="search-clear" type="button" title="清空" @click="recordKeyword = ''">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M7 7l10 10M17 7L7 17" />
          </svg>
        </button>
      </div>
    <div class="form-row compact filter-bar">
      <label class="field-label">开始</label>
      <input class="input" type="date" v-model="recordFilterStart" @change="handleRecordDatePicked" />
      <label class="field-label">结束</label>
      <input class="input" type="date" v-model="recordFilterEnd" @change="handleRecordDatePicked" />
      <label class="field-label">类型</label>
      <select class="select" v-model="recordFilterType">
        <option value="all">全部</option>
        <option value="TASK">任务</option>
        <option value="RECURRING">循环</option>
      </select>
      <button class="button secondary" @click="applyRecordFilter">应用过滤</button>
      <button class="button secondary" @click="clearRecordFilter">清除过滤</button>
      <button
        class="button secondary"
        :disabled="!oldRecords.length"
        :title="oldRecords.length ? `删除 ${cleanupDateLabel} 之前的 ${oldRecords.length} 条提醒记录` : `没有 ${cleanupDateLabel} 之前的提醒记录`"
        @click="cleanupOldRecords"
      >
        清理半年前
      </button>
    </div>
    <ListBatchBar
      v-if="selectedCount"
      :count="selectedCount"
      :total="filteredRecords.length"
      @select-all="selectAllVisible"
      @cancel="clearSelection"
    >
      <button class="button danger" type="button" @click="deleteSelectedRecords">删除所选</button>
    </ListBatchBar>
    <div class="table-card">
      <div class="table-scroll">
        <table class="table records-table" :class="{ 'is-selecting': selectedCount > 0 }">
          <thead>
            <tr>
              <th class="col-select">
                <input
                  type="checkbox"
                  title="全选本页"
                  aria-label="全选本页"
                  :checked="pageAllSelected"
                  :indeterminate.prop="pageSomeSelected && !pageAllSelected"
                  :disabled="!recordPage.length"
                  @change="togglePage"
                />
              </th>
              <th class="col-desc">描述</th>
              <th class="col-type">类型</th>
              <th class="col-datetime">触发时间</th>
              <th class="col-datetime">关闭时间</th>
              <th class="col-action">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="record in recordPage"
              :key="record.id"
              class="table-row"
              :class="{ 'is-selected': isSelected(record.id) }"
              @click="handleRowClick($event, record.id)"
              @dblclick="openRecordDetail(record)"
              @contextmenu.prevent.stop="openRecordMenu($event, record)"
            >
              <td class="col-select">
                <input
                  type="checkbox"
                  title="选择（Shift 连选）"
                  :checked="isSelected(record.id)"
                  @click.stop="toggleSelection($event, record.id)"
                />
              </td>
              <td class="col-desc cell-title" :title="recordDescription(record.description)">{{ recordDescription(record.description) }}</td>
              <td class="col-type" :title="record.type === 'TASK' ? '任务' : '循环'">
                <span class="chip" :class="record.type === 'TASK' ? 'is-task' : 'is-recurring'">{{ record.type === 'TASK' ? '任务' : '循环' }}</span>
              </td>
              <td class="col-datetime cell-time" :title="formatDateTime(record.triggerTime)">{{ formatDateTime(record.triggerTime) }}</td>
              <td class="col-datetime cell-time" :title="formatDateTime(record.closeTime)">
                <span v-if="record.closeTime">{{ formatDateTime(record.closeTime) }}</span>
                <span v-else class="cell-empty">—</span>
              </td>
              <td class="col-action" :title="formatAction(record.action)">
                <span class="status-pill" :class="`action-${record.action.toLowerCase()}`">{{ formatAction(record.action) }}</span>
              </td>
            </tr>
            <tr v-if="!recordPage.length" class="table-empty-row">
              <td colspan="6">
                <div class="table-empty">
                  <span class="table-empty-title">暂无提醒记录</span>
                  <span class="table-empty-hint">提醒弹出后会自动记录在这里</span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <Pagination :total="filteredRecords.length" :total-pages="recordTotalPages" v-model:page-index="recordPageIndex" v-model:page-size="recordPageSize" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { api } from "../api";
import ListBatchBar from "../components/ListBatchBar.vue";
import Pagination from "../components/Pagination.vue";
import { formatAction, formatDateTime, isLinuxPlatform, recordDescription } from "../format";
import { VIEW_SHORTCUT_EVENT, type ShortcutAction } from "../keyboard";
import { monthsAgoCutoff, recordsBefore } from "../records";
import { matchesKeyword, parseQuery } from "../search";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";
import { useItemActions } from "../composables/useItemActions";
import { useListSelection } from "../composables/useListSelection";
import { useNow } from "../composables/useNow";
import { usePagination } from "../composables/usePagination";

const { reminderRecords, refreshAll } = useAppData();
const { openRecordDetail, openRecordMenu, confirmDeleteRecords } = useItemActions();
const { confirmAction } = useDialogs();

const recordFilterStart = ref("");
const recordFilterEnd = ref("");
const recordFilterType = ref("all");

const recordKeyword = ref("");
const filteredRecords = computed(() => {
  const query = parseQuery(recordKeyword.value);
  return reminderRecords.value.filter(record => {
    if (!matchesKeyword({ text: [recordDescription(record.description)] }, query)) {
      return false;
    }
    if (recordFilterType.value !== "all" && record.type !== recordFilterType.value) {
      return false;
    }
    if (recordFilterStart.value && record.triggerTime < `${recordFilterStart.value}T00:00:00`) {
      return false;
    }
    if (recordFilterEnd.value && record.triggerTime > `${recordFilterEnd.value}T23:59:59`) {
      return false;
    }
    return true;
  });
});

const {
  pageIndex: recordPageIndex,
  pageSize: recordPageSize,
  totalPages: recordTotalPages,
  page: recordPage
} = usePagination(filteredRecords, recordKeyword);

// 多选（v2.2.8）：勾选框切换、Shift 连选、表头全选本页、批量栏中全选筛选结果；
// 已有选中的行时点击行也会切换选择。
const {
  selectedIds,
  count: selectedCount,
  pageAllSelected,
  pageSomeSelected,
  isSelected,
  toggle: toggleSelection,
  togglePage,
  selectAllVisible,
  clear: clearSelection
} = useListSelection(
  computed(() => filteredRecords.value.map(record => record.id)),
  computed(() => recordPage.value.map(record => record.id))
);

const handleRowClick = (event: MouseEvent, id: string) => {
  if (!selectedCount.value) return;
  if ((event.target as HTMLElement | null)?.closest("a, button, input")) return;
  toggleSelection(event, id);
};

const handleRecordDatePicked = (event: Event) => {
  if (!isLinuxPlatform) {
    return;
  }
  const target = event.target;
  if (!(target instanceof HTMLInputElement)) {
    return;
  }
  requestAnimationFrame(() => {
    target.blur();
  });
};

const applyRecordFilter = () => {
  recordPageIndex.value = 1;
};

const clearRecordFilter = () => {
  recordFilterStart.value = "";
  recordFilterEnd.value = "";
  recordFilterType.value = "all";
  recordKeyword.value = "";
  recordPageIndex.value = 1;
};

const deleteSelectedRecords = () => {
  confirmDeleteRecords([...selectedIds.value], clearSelection);
};

// 一键清理半年前的记录（v2.2.8）：截止时间为半年前当天零点，不受当前筛选影响。
const now = useNow();
const cleanupCutoff = computed(() => monthsAgoCutoff(now.value));
const cleanupDateLabel = computed(() => cleanupCutoff.value.slice(0, 10));
const oldRecords = computed(() => recordsBefore(reminderRecords.value, cleanupCutoff.value));

const cleanupOldRecords = () => {
  const cutoff = cleanupCutoff.value;
  const count = recordsBefore(reminderRecords.value, cutoff).length;
  if (!count) return;
  confirmAction({
    title: "清理半年前的提醒记录",
    message: `确定要删除 ${cutoff.slice(0, 10)} 之前的 ${count} 条提醒记录吗？`,
    action: async () => {
      await api.deleteReminderRecordsBefore(cutoff);
      await refreshAll();
    }
  });
};

// 快捷键：Esc 取消选择；Delete 删除选中的记录（需确认）。
const handleViewShortcut = (event: Event) => {
  const action = (event as CustomEvent<ShortcutAction>).detail;
  if (action.type === "escape" && selectedCount.value) {
    clearSelection();
  } else if (action.type === "delete" && selectedCount.value) {
    deleteSelectedRecords();
  }
};
onMounted(() => window.addEventListener(VIEW_SHORTCUT_EVENT, handleViewShortcut));
onBeforeUnmount(() => window.removeEventListener(VIEW_SHORTCUT_EVENT, handleViewShortcut));
</script>
