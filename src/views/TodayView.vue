<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">今天</div>
      <span class="section-meta">{{ formatMonthDay(now) }}</span>
    </div>

    <div class="stat-row">
      <div class="stat-tile">
        <span class="stat-label">今日提醒</span>
        <span class="stat-value">{{ today.summary.total }}</span>
      </div>
      <div class="stat-tile">
        <span class="stat-label">待提醒</span>
        <span class="stat-value">{{ today.summary.upcoming }}</span>
      </div>
      <div class="stat-tile" :class="{ 'is-warning': today.summary.overdue > 0 }">
        <span class="stat-label">逾期未完成</span>
        <span class="stat-value">{{ today.summary.overdue }}</span>
      </div>
      <div class="stat-tile">
        <span class="stat-label">今日完成</span>
        <span class="stat-value">{{ today.summary.completedToday }}</span>
      </div>
    </div>

    <div class="table-card">
      <div class="table-scroll table-scroll-no-x today-scroll">
        <div v-if="today.overdueTasks.length" class="today-group">
          <div class="today-group-title">更早逾期<span>{{ today.overdueTasks.length }}</span></div>
          <div
            v-for="task in today.overdueTasks"
            :key="task.id"
            class="timeline-item is-overdue"
            @dblclick="openTaskEditor(task)"
            @contextmenu.prevent.stop="openTaskMenu($event, task)"
          >
            <span class="timeline-time is-date">{{ formatShortDate(taskAnchorTime(task)) }}</span>
            <span class="timeline-dot" aria-hidden="true"></span>
            <div class="timeline-body">
              <input type="checkbox" class="check-round" title="标记完成" @change="toggleTask(task)" />
              <span class="timeline-title" :title="task.description">{{ task.description }}</span>
              <span class="status-pill action-overdue">未完成</span>
            </div>
          </div>
        </div>

        <div class="today-group">
          <div v-if="today.overdueTasks.length" class="today-group-title">今天<span>{{ today.entries.length }}</span></div>
          <template v-for="(entry, index) in today.entries" :key="entry.key">
            <div v-if="index === markerIndex" class="timeline-now">
              <span class="timeline-time">{{ formatClock(nowString) }}</span>
              <span class="timeline-now-line">现在</span>
            </div>
            <div
              class="timeline-item"
              :class="[`is-${entry.state.toLowerCase()}`, { 'is-past': entry.isPast }]"
              @dblclick="openEntry(entry)"
              @contextmenu.prevent.stop="openEntryMenu($event, entry)"
            >
              <span class="timeline-time">{{ formatClock(entry.time) }}</span>
              <span class="timeline-dot" aria-hidden="true"></span>
              <div class="timeline-body">
                <input
                  v-if="entry.task"
                  type="checkbox"
                  class="check-round"
                  :title="entry.state === 'done' ? '取消完成' : '标记完成'"
                  :checked="entry.state === 'done'"
                  @change="toggleTask(entry.task)"
                />
                <span class="chip" :class="entry.kind === 'task' ? 'is-task' : 'is-recurring'">
                  {{ entry.kind === "task" ? "待办" : "循环" }}
                </span>
                <span class="timeline-title" :class="{ 'is-done': entry.state === 'done' }" :title="entry.title">
                  {{ entry.kind === "task" ? entry.title : recordDescription(entry.title) }}
                </span>
                <span v-if="entry.collapsedCount" class="timeline-note">
                  {{ entry.isPast ? `今日已提醒 ${entry.collapsedCount + 1} 次` : `今日还有 ${entry.collapsedCount + 1} 次` }}
                </span>
                <span class="status-pill" :class="stateClass(entry)">{{ stateLabel(entry) }}</span>
              </div>
            </div>
          </template>
          <div v-if="today.entries.length && markerIndex === today.entries.length" class="timeline-now">
            <span class="timeline-time">{{ formatClock(nowString) }}</span>
            <span class="timeline-now-line">现在 · 今天的提醒都已过去</span>
          </div>
          <div v-if="!today.entries.length" class="table-empty">
            <span class="table-empty-title">今天没有安排提醒</span>
            <span class="table-empty-hint">在待办中设置提醒时间，或添加循环提醒</span>
            <div class="today-empty-actions">
              <button class="button secondary" type="button" @click="emit('navigate', 'tasks')">去添加待办</button>
              <button class="button secondary" type="button" @click="emit('navigate', 'recurring')">去添加循环提醒</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { taskAnchorTime } from "../due";
import { computed, ref, watch } from "vue";
import { api } from "../api";
import { formatAction, formatClock, formatMonthDay, recordDescription, toLocalDateTimeString, dateKey } from "../format";
import { buildTodayData, nowMarkerIndex, type TimelineEntry } from "../timeline";
import type { TabKey } from "../navigation";
import type { RecurringPreview } from "../types";
import { useAppData } from "../composables/useAppData";
import { useItemActions } from "../composables/useItemActions";
import { useNow } from "../composables/useNow";

const emit = defineEmits<{ (e: "navigate", tab: TabKey): void }>();

const { tasks, completedTasks, recurringTasks, reminderRecords, dataVersion } = useAppData();
const {
  toggleTask,
  openTaskMenu,
  openTaskEditor,
  openRecurringMenu,
  openRecurringEditor,
  openRecordDetail,
  openRecordMenu
} = useItemActions();

const now = useNow();
const nowString = computed(() => toLocalDateTimeString(now.value));
const previews = ref<RecurringPreview[]>([]);

const loadPreviews = async () => {
  try {
    previews.value = await api.previewRecurringTriggers(`${dateKey(now.value)}T23:59:59`);
  } catch (error) {
    console.error("[today] 读取循环提醒预估失败", error);
  }
};

// 数据变化或跨天时重新预估；循环提醒触发后 next_trigger 会前移，也会带来 data 刷新。
watch([dataVersion, () => dateKey(now.value)], loadPreviews, { immediate: true });

const today = computed(() =>
  buildTodayData({
    now: now.value,
    tasks: tasks.value,
    completedTasks: completedTasks.value,
    recurringTasks: recurringTasks.value,
    records: reminderRecords.value,
    previews: previews.value,
  })
);

const markerIndex = computed(() => nowMarkerIndex(today.value.entries, now.value));

const formatShortDate = (value?: string | null) => (value ? `${Number(value.slice(5, 7))}/${Number(value.slice(8, 10))}` : "");

const stateLabel = (entry: TimelineEntry) => {
  switch (entry.state) {
    case "upcoming":
      return "待提醒";
    case "overdue":
      return "未完成";
    case "done":
      return "已完成";
    default:
      return formatAction(entry.state);
  }
};

const stateClass = (entry: TimelineEntry) => {
  switch (entry.state) {
    case "upcoming":
      return "action-upcoming";
    case "overdue":
      return "action-overdue";
    case "done":
      return "action-completed";
    default:
      return `action-${entry.state.toLowerCase()}`;
  }
};

const openEntry = (entry: TimelineEntry) => {
  if (entry.task) {
    openTaskEditor(entry.task);
  } else if (entry.recurring) {
    openRecurringEditor(entry.recurring);
  } else if (entry.record) {
    openRecordDetail(entry.record);
  }
};

const openEntryMenu = (event: MouseEvent, entry: TimelineEntry) => {
  if (entry.task) {
    openTaskMenu(event, entry.task);
  } else if (entry.recurring) {
    openRecurringMenu(event, entry.recurring);
  } else if (entry.record) {
    openRecordMenu(event, entry.record);
  }
};
</script>
