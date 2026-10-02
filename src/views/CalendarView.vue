<template>
  <div class="tab-panel">
    <div class="section-heading calendar-heading">
      <div class="section-title">日历</div>
      <span class="section-meta">{{ mode === "week" ? weekLabel : monthLabel }}</span>
      <div class="calendar-nav">
        <div class="segmented" role="tablist" aria-label="日历视图">
          <button
            v-for="option in MODE_OPTIONS"
            :key="option.value"
            type="button"
            class="segmented-item"
            :class="{ active: mode === option.value }"
            role="tab"
            :aria-selected="mode === option.value"
            @click="setMode(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
        <button
          class="button secondary calendar-nav-button"
          type="button"
          :title="mode === 'week' ? '上一周' : '上个月'"
          @click="shiftPeriod(-1)"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14.5 6L8.5 12L14.5 18" /></svg>
        </button>
        <button class="button secondary" type="button" @click="goToday">今天</button>
        <button
          class="button secondary calendar-nav-button"
          type="button"
          :title="mode === 'week' ? '下一周' : '下个月'"
          @click="shiftPeriod(1)"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9.5 6L15.5 12L9.5 18" /></svg>
        </button>
      </div>
    </div>

    <div class="calendar-layout">
      <div v-if="mode === 'week'" class="table-card calendar-card calendar-week">
        <div ref="weekScroll" class="calendar-week-body">
          <div class="calendar-week-header">
            <span class="calendar-week-gutter"></span>
            <button
              v-for="day in weekDays"
              :key="day.key"
              type="button"
              class="calendar-week-day"
              :class="{
                'is-weekend': day.isWeekend,
                'is-today': day.key === todayKey,
                'is-selected': day.key === selectedKey
              }"
              @click="selectDay(day)"
            >
              <span class="calendar-week-day-name">周{{ WEEKDAY_HEADERS[(day.date.getDay() + 6) % 7] }}</span>
              <span class="calendar-week-day-date">{{ day.date.getDate() }}</span>
            </button>
          </div>
          <div v-for="hour in HOURS" :key="hour" class="calendar-week-row">
            <span class="calendar-week-hour">{{ String(hour).padStart(2, "0") }}:00</span>
            <div
              v-for="day in weekDays"
              :key="day.key"
              class="calendar-week-slot"
              :class="{
                'is-today': day.key === todayKey,
                'is-selected': day.key === selectedKey,
                'is-now': day.key === todayKey && hour === now.getHours(),
                'is-drop-target': dropKey === slotKey(day, hour)
              }"
              @click="selectDay(day)"
              @dragover.prevent="handleSlotDragOver(day, hour)"
              @dragleave="handleSlotDragLeave(day, hour)"
              @drop.prevent="handleSlotDrop(day, hour)"
            >
              <div
                v-for="item in slotItems(day, hour).slice(0, MAX_SLOT_ITEMS)"
                :key="item.key"
                class="calendar-pill"
                :class="[`is-${item.kind}`, `is-${item.state}`]"
                :title="pillTitle(item)"
                :draggable="isDraggable(item)"
                @dragstart="handleDragStart($event, item)"
                @dragend="dropKey = ''"
                @dblclick.stop="openItem(item)"
                @contextmenu.prevent.stop="openItemMenu($event, item)"
              >
                <span class="calendar-pill-time">{{ formatClock(item.time) }}</span>
                <span class="calendar-pill-title">{{ item.title }}</span>
                <span v-if="item.count > 1" class="calendar-pill-count">×{{ item.count }}</span>
              </div>
              <span v-if="slotItems(day, hour).length > MAX_SLOT_ITEMS" class="calendar-more">
                还有 {{ slotItems(day, hour).length - MAX_SLOT_ITEMS }} 项
              </span>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="table-card calendar-card">
        <div class="calendar-weekdays">
          <span v-for="label in WEEKDAY_HEADERS" :key="label">{{ label }}</span>
        </div>
        <div class="calendar-grid">
          <div
            v-for="day in grid"
            :key="day.key"
            class="calendar-cell"
            :class="{
              'is-outside': !day.inMonth,
              'is-weekend': day.isWeekend,
              'is-today': day.key === todayKey,
              'is-selected': day.key === selectedKey,
              'is-drop-target': day.key === dropKey
            }"
            role="button"
            tabindex="0"
            :aria-label="`${day.date.getMonth() + 1}月${day.date.getDate()}日，${(buckets.get(day.key) ?? []).length} 项`"
            @click="selectDay(day)"
            @keydown.enter.prevent="selectDay(day)"
            @dragover.prevent="handleDragOver(day)"
            @dragleave="handleDragLeave(day)"
            @drop.prevent="handleDrop(day)"
          >
            <span class="calendar-date">{{ day.date.getDate() }}</span>
            <div class="calendar-items">
              <div
                v-for="item in (buckets.get(day.key) ?? []).slice(0, MAX_CELL_ITEMS)"
                :key="item.key"
                class="calendar-pill"
                :class="[`is-${item.kind}`, `is-${item.state}`]"
                :title="pillTitle(item)"
                :draggable="isDraggable(item)"
                @dragstart="handleDragStart($event, item)"
                @dragend="dropKey = ''"
                @dblclick.stop="openItem(item)"
                @contextmenu.prevent.stop="openItemMenu($event, item)"
              >
                <span class="calendar-pill-time">{{ formatClock(item.time) }}</span>
                <span class="calendar-pill-title">{{ item.title }}</span>
                <span v-if="item.count > 1" class="calendar-pill-count">×{{ item.count }}</span>
              </div>
              <span v-if="(buckets.get(day.key) ?? []).length > MAX_CELL_ITEMS" class="calendar-more">
                还有 {{ (buckets.get(day.key) ?? []).length - MAX_CELL_ITEMS }} 项
              </span>
            </div>
          </div>
        </div>
      </div>

      <div class="table-card calendar-day-panel">
        <div class="calendar-day-header">
          <span class="calendar-day-title">{{ formatMonthDay(selectedDate) }}</span>
          <span class="section-meta">{{ selectedItems.length }} 项</span>
        </div>
        <div class="calendar-day-composer">
          <input
            data-shortcut="new"
            v-model="newTitle"
            class="input"
            :placeholder="composerPlaceholder"
            @keydown.enter.exact.prevent="handleAdd"
          />
          <SmartParseHint
            v-model:enabled="parseEnabled"
            :parsed="parsed"
            :has-meta="hasMeta"
            :schedule-text="scheduleText"
            :error="parseError"
          />
        </div>
        <div class="calendar-day-list">
          <div
            v-for="item in selectedItems"
            :key="item.key"
            class="calendar-day-item"
            :class="[`is-${item.state}`]"
            @dblclick="openItem(item)"
            @contextmenu.prevent.stop="openItemMenu($event, item)"
          >
            <input
              v-if="item.task"
              type="checkbox"
              class="check-round"
              :checked="item.state === 'done'"
              :title="item.state === 'done' ? '取消完成' : '标记完成'"
              @change="toggleTask(item.task)"
            />
            <span v-else class="chip is-recurring">循环</span>
            <div class="calendar-day-item-body">
              <span class="calendar-day-item-title" :class="{ 'is-done': item.state === 'done' }" :title="item.title">
                {{ item.title }}
              </span>
              <span class="calendar-day-item-meta">
                {{ itemMeta(item) }}
                <TaskBadges v-if="item.task" :task="item.task" />
              </span>
            </div>
          </div>
          <div v-if="!selectedItems.length" class="table-empty">
            <span class="table-empty-title">这天没有安排</span>
            <span class="table-empty-hint">{{ selectedIsPast ? "已经过去的日期" : emptyHint }}</span>
          </div>
        </div>
        <div class="calendar-legend">
          <span class="calendar-legend-item is-task">待办</span>
          <span class="calendar-legend-item is-recurring">循环提醒</span>
          <span class="calendar-legend-item is-overdue">逾期</span>
          <span class="calendar-legend-item is-done">已完成</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import SmartParseHint from "../components/SmartParseHint.vue";
import TaskBadges from "../components/TaskBadges.vue";
import { api } from "../api";
import {
  bucketCalendarItems,
  buildMonthGrid,
  buildWeekDays,
  defaultReminderForDay,
  formatWeekRange,
  groupItemsByHour,
  moveReminderToDay,
  moveReminderToSlot,
  type CalendarDay,
  type CalendarItem
} from "../calendar";
import { addDays, dateKey, errorMessage, formatClock, formatMonthDay, toLocalDateTimeString } from "../format";
import { formatRecurringRule } from "../recurring";
import { safeStorage } from "../safeStorage";
import type { RecurringPreview, Task } from "../types";
import { useAppData } from "../composables/useAppData";
import { useItemActions } from "../composables/useItemActions";
import { useNow } from "../composables/useNow";
import { useSmartAdd } from "../composables/useSmartAdd";

const WEEKDAY_HEADERS = ["一", "二", "三", "四", "五", "六", "日"];
const MAX_CELL_ITEMS = 3;
const MAX_SLOT_ITEMS = 2;
const HOURS = Array.from({ length: 24 }, (_, hour) => hour);
/** 周视图每个时段的高度，与样式中的 .calendar-week-row 一致，用于滚动到当前时段。 */
const WEEK_ROW_HEIGHT = 44;
const MODE_STORAGE_KEY = "calendarMode";

type CalendarMode = "month" | "week";
const MODE_OPTIONS: { value: CalendarMode; label: string }[] = [
  { value: "month", label: "月" },
  { value: "week", label: "周" }
];
/** 月视图预估循环提醒的上限：高频的区间提醒只影响每天的次数显示。 */
const PREVIEW_LIMIT = 500;

const { tasks, completedTasks, recurringTasks, reminderRecords, dataVersion, refreshAll } = useAppData();
const { toggleTask, openTaskMenu, openTaskEditor, openRecurringMenu, openRecurringEditor } = useItemActions();

const now = useNow();
const todayKey = computed(() => dateKey(now.value));
const viewYear = ref(now.value.getFullYear());
const viewMonth = ref(now.value.getMonth());
const selectedDate = ref(new Date(now.value.getFullYear(), now.value.getMonth(), now.value.getDate()));
const selectedKey = computed(() => dateKey(selectedDate.value));
const selectedIsPast = computed(() => selectedKey.value < todayKey.value);

const mode = ref<CalendarMode>(safeStorage.getItem(MODE_STORAGE_KEY) === "week" ? "week" : "month");
const grid = computed(() => buildMonthGrid(viewYear.value, viewMonth.value));
const weekDays = computed(() => buildWeekDays(selectedDate.value));
const monthLabel = computed(() => `${viewYear.value} 年 ${viewMonth.value + 1} 月`);
const weekLabel = computed(() => formatWeekRange(weekDays.value));
const visibleDays = computed(() => (mode.value === "week" ? weekDays.value : grid.value));
const rangeStart = computed(() => visibleDays.value[0].key);
const rangeEnd = computed(() => visibleDays.value[visibleDays.value.length - 1].key);

const previews = ref<RecurringPreview[]>([]);
const loadPreviews = async () => {
  // 只需要预估未来：网格整体在过去时不必请求。
  if (rangeEnd.value < todayKey.value) {
    previews.value = [];
    return;
  }
  try {
    previews.value = await api.previewRecurringTriggers(`${rangeEnd.value}T23:59:59`, PREVIEW_LIMIT);
  } catch (error) {
    console.error("[calendar] 读取循环提醒预估失败", error);
  }
};
watch([dataVersion, rangeEnd, todayKey], loadPreviews, { immediate: true });

const calendarInput = computed(() => ({
  now: now.value,
  startKey: rangeStart.value,
  endKey: rangeEnd.value,
  tasks: tasks.value,
  completedTasks: completedTasks.value,
  recurringTasks: recurringTasks.value,
  records: reminderRecords.value,
  previews: previews.value,
}));
// 月视图与右侧当天列表按天合并循环提醒；周视图按小时合并，再按时段归类。
const buckets = computed(() => bucketCalendarItems(calendarInput.value));
const hourBuckets = computed(() => {
  const result = new Map<string, Map<number, CalendarItem[]>>();
  if (mode.value !== "week") {
    return result;
  }
  const byDay = bucketCalendarItems({ ...calendarInput.value, recurringGrouping: "hour" });
  for (const [key, items] of byDay) {
    result.set(key, groupItemsByHour(items));
  }
  return result;
});
const slotItems = (day: CalendarDay, hour: number) => hourBuckets.value.get(day.key)?.get(hour) ?? [];
const slotKey = (day: CalendarDay, hour: number) => `${day.key}|${hour}`;
const selectedItems = computed(() => buckets.value.get(selectedKey.value) ?? []);
const emptyHint = computed(() =>
  mode.value === "week"
    ? "在上方输入即可添加，也可以把待办拖到其他时段"
    : "在上方输入即可添加，也可以把待办拖到其他日期"
);

const syncMonthToSelection = () => {
  viewYear.value = selectedDate.value.getFullYear();
  viewMonth.value = selectedDate.value.getMonth();
};

const shiftPeriod = (delta: number) => {
  if (mode.value === "week") {
    selectedDate.value = addDays(selectedDate.value, delta * 7);
    syncMonthToSelection();
    return;
  }
  const target = new Date(viewYear.value, viewMonth.value + delta, 1);
  viewYear.value = target.getFullYear();
  viewMonth.value = target.getMonth();
};

// 周视图打开时滚动到当前时段附近（今天所在周）或早上 7 点。
const weekScroll = ref<HTMLElement | null>(null);
const scrollWeekToFocus = async () => {
  await nextTick();
  if (!weekScroll.value) {
    return;
  }
  const inThisWeek = weekDays.value.some(day => day.key === todayKey.value);
  const hour = inThisWeek ? Math.max(0, now.value.getHours() - 1) : 7;
  weekScroll.value.scrollTop = hour * WEEK_ROW_HEIGHT;
};

const setMode = (value: CalendarMode) => {
  if (mode.value === value) {
    return;
  }
  mode.value = value;
  safeStorage.setItem(MODE_STORAGE_KEY, value);
  if (value === "month") {
    syncMonthToSelection();
  } else {
    void scrollWeekToFocus();
  }
};

if (mode.value === "week") {
  void scrollWeekToFocus();
}

const goToday = () => {
  viewYear.value = now.value.getFullYear();
  viewMonth.value = now.value.getMonth();
  selectedDate.value = new Date(now.value.getFullYear(), now.value.getMonth(), now.value.getDate());
  if (mode.value === "week") {
    void scrollWeekToFocus();
  }
};

const selectDay = (day: CalendarDay) => {
  selectedDate.value = day.date;
  if (!day.inMonth) {
    viewYear.value = day.date.getFullYear();
    viewMonth.value = day.date.getMonth();
  }
};

const stateText: Record<CalendarItem["state"], string> = {
  upcoming: "待提醒",
  overdue: "逾期未完成",
  done: "已完成",
  fired: "已提醒",
};

const itemMeta = (item: CalendarItem) => {
  const parts = [formatClock(item.time), stateText[item.state]];
  if (item.count > 1) {
    parts.push(`共 ${item.count} 次`);
  }
  if (item.recurring) {
    parts.push(formatRecurringRule(item.recurring));
  }
  return parts.join(" · ");
};

const pillTitle = (item: CalendarItem) => `${item.title}\n${itemMeta(item)}`;

const openItem = (item: CalendarItem) => {
  if (item.task) {
    openTaskEditor(item.task);
  } else if (item.recurring) {
    openRecurringEditor(item.recurring);
  }
};

const openItemMenu = (event: MouseEvent, item: CalendarItem) => {
  if (item.task) {
    openTaskMenu(event, item.task);
  } else if (item.recurring) {
    openRecurringMenu(event, item.recurring);
  }
};

// 拖动未完成的待办到另一天：保留原钟点。
const draggingTaskId = ref("");
const dropKey = ref("");
const isDraggable = (item: CalendarItem) => item.kind === "task" && item.state !== "done";

const handleDragStart = (event: DragEvent, item: CalendarItem) => {
  if (!isDraggable(item) || !item.task) {
    event.preventDefault();
    return;
  }
  draggingTaskId.value = item.task.id;
  event.dataTransfer?.setData("text/plain", item.task.id);
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
  }
};

const handleDragOver = (day: CalendarDay) => {
  if (draggingTaskId.value) {
    dropKey.value = day.key;
  }
};

const handleDragLeave = (day: CalendarDay) => {
  if (dropKey.value === day.key) {
    dropKey.value = "";
  }
};

const handleSlotDragOver = (day: CalendarDay, hour: number) => {
  if (draggingTaskId.value) {
    dropKey.value = slotKey(day, hour);
  }
};

const handleSlotDragLeave = (day: CalendarDay, hour: number) => {
  if (dropKey.value === slotKey(day, hour)) {
    dropKey.value = "";
  }
};

const handleDrop = (day: CalendarDay) =>
  moveDraggedTask(day, task => moveReminderToDay(task.reminderTime, day.date));

// 周视图：拖到某个时段，改到该天该钟点（保留分钟）。
const handleSlotDrop = (day: CalendarDay, hour: number) =>
  moveDraggedTask(day, task => moveReminderToSlot(task.reminderTime, day.date, hour));

const moveDraggedTask = async (day: CalendarDay, target: (task: Task) => string) => {
  const taskId = draggingTaskId.value;
  draggingTaskId.value = "";
  dropKey.value = "";
  const task = tasks.value.find(item => item.id === taskId);
  if (!task) {
    return;
  }
  const reminderTime = target(task);
  if (reminderTime === task.reminderTime) {
    return;
  }
  if (new Date(reminderTime).getTime() <= Date.now()) {
    alert("提醒时间需晚于当前时间");
    return;
  }
  try {
    await api.updateTask({
      id: task.id,
      description: task.description,
      stickyContent: task.stickyContent ?? null,
      reminderTime,
    });
    selectedDate.value = day.date;
    await refreshAll();
  } catch (error) {
    alert(`调整提醒日期失败：${errorMessage(error)}`);
  }
};

const newTitle = ref("");
const {
  enabled: parseEnabled,
  error: parseError,
  parsed,
  hasMeta,
  scheduleText,
  submit
} = useSmartAdd(newTitle);

const composerPlaceholder = computed(() => {
  const fallback = defaultReminderForDay(selectedDate.value, now.value);
  return fallback
    ? `在这天添加待办（默认 ${formatClock(toLocalDateTimeString(fallback))} 提醒）`
    : "添加待办（不设提醒）";
});

const handleAdd = async () => {
  await submit({ fallbackReminder: defaultReminderForDay(selectedDate.value, new Date()) });
};
</script>
