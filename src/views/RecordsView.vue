<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">提醒记录</div>
      <span class="section-meta">筛选后 {{ filteredRecords.length }} 条</span>
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
      <button class="button danger" @click="deleteSelectedRecords">批量删除</button>
    </div>
    <div class="table-card">
      <div class="table-scroll">
        <table class="table records-table">
          <thead>
            <tr>
              <th class="col-select">选择</th>
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
              @dblclick="openRecordDetail(record)"
              @contextmenu.prevent.stop="openRecordMenu($event, record)"
            >
              <td class="col-select">
                <input type="checkbox" v-model="selectedRecords" :value="record.id" />
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
import { computed, ref } from "vue";
import Pagination from "../components/Pagination.vue";
import { formatAction, formatDateTime, isLinuxPlatform, recordDescription } from "../format";
import { useAppData } from "../composables/useAppData";
import { useItemActions } from "../composables/useItemActions";
import { usePagination } from "../composables/usePagination";

const { reminderRecords } = useAppData();
const { openRecordDetail, openRecordMenu, confirmDeleteRecords } = useItemActions();

const recordFilterStart = ref("");
const recordFilterEnd = ref("");
const recordFilterType = ref("all");
const selectedRecords = ref<string[]>([]);

const filteredRecords = computed(() => {
  return reminderRecords.value.filter(record => {
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
} = usePagination(filteredRecords);

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
  recordPageIndex.value = 1;
};

const deleteSelectedRecords = () => {
  confirmDeleteRecords([...selectedRecords.value], () => {
    selectedRecords.value = [];
  });
};
</script>
