<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">统计</div>
      <div class="segmented" role="radiogroup" aria-label="统计范围">
        <button
          v-for="option in RANGE_OPTIONS"
          :key="option.days"
          class="segmented-item"
          :class="{ active: rangeDays === option.days }"
          type="button"
          role="radio"
          :aria-checked="rangeDays === option.days"
          @click="rangeDays = option.days"
        >
          {{ option.label }}
        </button>
      </div>
    </div>

    <div class="stats-scroll">
      <div class="stat-row">
        <div class="stat-tile">
          <span class="stat-label">提醒次数</span>
          <span class="stat-value">{{ stats.total }}</span>
        </div>
        <div class="stat-tile">
          <span class="stat-label">直接处理率</span>
          <span class="stat-value">{{ handledRateText }}</span>
          <span class="stat-hint">完成或关闭，未推迟</span>
        </div>
        <div class="stat-tile">
          <span class="stat-label">推迟次数</span>
          <span class="stat-value">{{ stats.counts.SNOOZED }}</span>
        </div>
        <div class="stat-tile">
          <span class="stat-label">完成待办</span>
          <span class="stat-value">{{ stats.completedTasks }}</span>
          <span v-if="rangeDays > 30" class="stat-hint">已完成任务保留 30 天</span>
        </div>
      </div>

      <section class="stats-card">
        <header class="stats-card-header">
          <div>
            <div class="stats-card-title">每日提醒</div>
            <div class="stats-card-subtitle">按处理结果统计每天弹出的提醒</div>
          </div>
          <button class="button ghost" type="button" @click="showTable = !showTable">
            {{ showTable ? "查看图表" : "查看数据表" }}
          </button>
        </header>
        <ul class="chart-legend">
          <li v-for="series in SERIES" :key="series.action">
            <span class="chart-swatch" :style="{ background: series.color }" aria-hidden="true"></span>{{ series.label }}
          </li>
        </ul>

        <div v-if="showTable" class="table-scroll stats-table-scroll">
          <table class="table stats-daily-table">
            <thead>
              <tr>
                <th>日期</th>
                <th v-for="series in SERIES" :key="series.action" class="is-number">{{ series.label }}</th>
                <th class="is-number">合计</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="day in [...stats.daily].reverse()" :key="day.date">
                <td class="cell-time">{{ formatDayLabel(day.date) }}</td>
                <td v-for="series in SERIES" :key="series.action" class="is-number cell-time">{{ day.counts[series.action] }}</td>
                <td class="is-number cell-title">{{ day.total }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div v-else class="daily-chart" :class="{ 'is-dense': rangeDays > 7 }" @pointerleave="hovered = null">
          <div class="chart-axis" aria-hidden="true">
            <span v-for="tick in ticks" :key="tick" :style="{ bottom: `${(tick / axisMax) * 100}%` }">{{ tick }}</span>
          </div>
          <div class="chart-plot">
            <span
              v-for="tick in ticks"
              :key="`grid-${tick}`"
              class="chart-grid"
              :style="{ bottom: `${(tick / axisMax) * 100}%` }"
              aria-hidden="true"
            ></span>
            <div
              v-for="(day, index) in stats.daily"
              :key="day.date"
              class="chart-column"
              :class="{ 'is-hovered': hovered === index }"
              tabindex="0"
              :aria-label="columnLabel(day)"
              @pointerenter="hovered = index"
              @focus="hovered = index"
              @blur="hovered = null"
            >
              <div class="chart-stack" :style="{ height: `${(day.total / axisMax) * 100}%` }">
                <span
                  v-for="series in SERIES"
                  v-show="day.counts[series.action] > 0"
                  :key="series.action"
                  class="chart-segment"
                  :style="{ flexGrow: day.counts[series.action], background: series.color }"
                ></span>
              </div>
              <span class="chart-x-label" :class="{ 'is-hidden': !showXLabel(index) }">{{ formatTick(day.date) }}</span>
            </div>
            <div
              v-if="hoveredDay"
              class="chart-tooltip"
              :class="{ 'is-right': hovered !== null && hovered > stats.daily.length / 2 }"
              :style="tooltipStyle"
              role="status"
            >
              <div class="chart-tooltip-title">{{ formatDayLabel(hoveredDay.date) }} · 共 {{ hoveredDay.total }} 次</div>
              <div v-for="series in SERIES" :key="series.action" class="chart-tooltip-row">
                <span class="chart-tooltip-key" :style="{ background: series.color }" aria-hidden="true"></span>
                <strong>{{ hoveredDay.counts[series.action] }}</strong>
                <span>{{ series.label }}</span>
              </div>
            </div>
          </div>
          <div v-if="!stats.total" class="chart-empty">这段时间还没有提醒记录</div>
        </div>
      </section>

      <div class="stats-grid">
        <section class="stats-card">
          <header class="stats-card-header">
            <div>
              <div class="stats-card-title">推迟最多</div>
              <div class="stats-card-subtitle">经常被推迟的提醒，也许该调整时间</div>
            </div>
          </header>
          <ol v-if="stats.topSnoozed.length" class="rank-list">
            <li v-for="item in stats.topSnoozed" :key="item.reminderId" class="rank-item">
              <span class="rank-title" :title="recordDescription(item.description)">{{ recordDescription(item.description) }}</span>
              <span class="rank-bar-track" aria-hidden="true">
                <span class="rank-bar" :style="{ width: `${(item.count / stats.topSnoozed[0].count) * 100}%` }"></span>
              </span>
              <span class="rank-value">{{ item.count }} 次</span>
            </li>
          </ol>
          <div v-else class="stats-empty">没有推迟过的提醒</div>
        </section>

        <section class="stats-card">
          <header class="stats-card-header">
            <div>
              <div class="stats-card-title">习惯打卡</div>
              <div class="stats-card-subtitle">循环提醒被完成或关闭即算打卡，推迟或未处理会中断连续天数</div>
            </div>
          </header>
          <table v-if="stats.habits.length" class="table habit-table">
            <thead>
              <tr>
                <th>循环提醒</th>
                <th class="is-number">当前连续</th>
                <th class="is-number">最长连续</th>
                <th class="is-number">打卡天数</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="habit in stats.habits" :key="habit.task.id">
                <td class="cell-title" :title="habit.task.description">{{ habit.task.description }}</td>
                <td class="is-number">
                  <span class="streak" :class="{ 'is-active': habit.current > 0 }">{{ habit.current }} 天</span>
                </td>
                <td class="is-number cell-time">{{ habit.best }} 天</td>
                <td class="is-number cell-time">{{ habit.checkedDays }} / {{ habit.remindedDays }}</td>
              </tr>
            </tbody>
          </table>
          <div v-else class="stats-empty">这段时间没有循环提醒的记录</div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { recordDescription } from "../format";
import { computeStats, type DailyBucket } from "../stats";
import type { UserAction } from "../types";
import { useAppData } from "../composables/useAppData";
import { useNow } from "../composables/useNow";
import { safeStorage } from "../safeStorage";

const RANGE_OPTIONS = [
  { days: 7, label: "近 7 天" },
  { days: 30, label: "近 30 天" },
];

// 颜色取自 styles.css 的图表令牌（深浅主题分别校验过色觉辨识度）；
// 从下往上堆叠：完成、关闭、推迟、待处理。
const SERIES: { action: UserAction; label: string; color: string }[] = [
  { action: "COMPLETED", label: "完成", color: "var(--chart-completed)" },
  { action: "DISMISSED", label: "关闭", color: "var(--chart-dismissed)" },
  { action: "SNOOZED", label: "推迟", color: "var(--chart-snoozed)" },
  { action: "PENDING", label: "未处理", color: "var(--chart-pending)" },
];

const WEEKDAY_NAMES = ["日", "一", "二", "三", "四", "五", "六"];

const { reminderRecords, completedTasks, recurringTasks } = useAppData();
const now = useNow(60_000);

const storedRange = Number(safeStorage.getItem("statsRangeDays"));
const rangeDays = ref(RANGE_OPTIONS.some(option => option.days === storedRange) ? storedRange : 7);
const showTable = ref(false);
const hovered = ref<number | null>(null);

watch(rangeDays, days => {
  safeStorage.setItem("statsRangeDays", String(days));
});

const stats = computed(() =>
  computeStats({
    now: now.value,
    days: rangeDays.value,
    records: reminderRecords.value,
    completedTasks: completedTasks.value,
    recurringTasks: recurringTasks.value,
  })
);

const handledRateText = computed(() =>
  stats.value.handledRate === null ? "—" : `${Math.round(stats.value.handledRate * 100)}%`
);

/** 纵轴取整：最大值向上取到 1/2/5×10ⁿ，分 4 格以内。 */
const niceStep = (max: number) => {
  const raw = Math.max(1, max) / 4;
  const power = 10 ** Math.floor(Math.log10(raw));
  const normalized = raw / power;
  const step = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
  return Math.max(1, step * power);
};
const axisStep = computed(() => niceStep(Math.max(0, ...stats.value.daily.map(day => day.total))));
const axisMax = computed(() => {
  const max = Math.max(0, ...stats.value.daily.map(day => day.total));
  return Math.max(axisStep.value, Math.ceil(max / axisStep.value) * axisStep.value);
});
const ticks = computed(() => {
  const list: number[] = [];
  for (let value = 0; value <= axisMax.value; value += axisStep.value) {
    list.push(value);
  }
  return list;
});

const hoveredDay = computed(() => (hovered.value === null ? null : stats.value.daily[hovered.value] ?? null));
const tooltipStyle = computed(() => {
  if (hovered.value === null) {
    return {};
  }
  const center = ((hovered.value + 0.5) / stats.value.daily.length) * 100;
  return { left: `${center}%` };
});

const parseDay = (date: string) => new Date(`${date}T00:00:00`);
const formatTick = (date: string) => {
  const day = parseDay(date);
  return rangeDays.value <= 7 ? `周${WEEKDAY_NAMES[day.getDay()]}` : `${day.getMonth() + 1}/${day.getDate()}`;
};
const formatDayLabel = (date: string) => {
  const day = parseDay(date);
  return `${day.getMonth() + 1}月${day.getDate()}日 周${WEEKDAY_NAMES[day.getDay()]}`;
};
const showXLabel = (index: number) => {
  const count = stats.value.daily.length;
  if (count <= 7) {
    return true;
  }
  // 30 天时每 5 天标一次，并保证最后一天（今天）有标签。
  return (count - 1 - index) % 5 === 0;
};
const columnLabel = (day: DailyBucket) =>
  `${formatDayLabel(day.date)}：共 ${day.total} 次，` +
  SERIES.map(series => `${series.label} ${day.counts[series.action]}`).join("，");
</script>
