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
          <span v-if="retentionLimited" class="stat-hint">
            已完成只保留 {{ retentionLabel }}，
            <button class="link-button" type="button" @click="openSettings">调整</button>
          </span>
        </div>
      </div>

      <section class="stats-card">
        <header class="stats-card-header">
          <div>
            <div class="stats-card-title">{{ isWeekly ? "每周提醒" : "每日提醒" }}</div>
            <div class="stats-card-subtitle">按处理结果统计{{ isWeekly ? "每周" : "每天" }}弹出的提醒</div>
          </div>
          <button class="button ghost" type="button" @click="showTable = !showTable">
            {{ showTable ? "查看图表" : "查看数据表" }}
          </button>
        </header>
        <ul class="chart-legend">
          <li v-for="series in SERIES" :key="series.key">
            <span class="chart-swatch" :style="{ background: series.color }" aria-hidden="true"></span>{{ series.label }}
          </li>
        </ul>

        <div v-if="showTable" class="table-scroll stats-table-scroll">
          <table class="table stats-daily-table">
            <thead>
              <tr>
                <th>{{ isWeekly ? "周" : "日期" }}</th>
                <th v-for="series in SERIES" :key="series.key" class="is-number">{{ series.label }}</th>
                <th class="is-number">合计</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="bucket in [...stats.buckets].reverse()" :key="bucket.date">
                <td class="cell-time">{{ bucketLabel(bucket) }}</td>
                <td v-for="series in SERIES" :key="series.key" class="is-number cell-time">{{ bucket.counts[series.key] }}</td>
                <td class="is-number cell-title">{{ bucket.total }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <StatsBarChart
          v-else
          :columns="reminderColumns"
          :series="SERIES"
          :dense="stats.buckets.length > 12"
          unit="次"
          empty-text="这段时间还没有提醒记录"
        />
      </section>

      <div class="stats-grid">
        <section class="stats-card">
          <header class="stats-card-header">
            <div>
              <div class="stats-card-title">完成趋势</div>
              <div class="stats-card-subtitle">{{ isWeekly ? "每周" : "每天" }}完成的待办与循环提醒</div>
            </div>
            <button class="button ghost" type="button" @click="showCompletionTable = !showCompletionTable">
              {{ showCompletionTable ? "查看图表" : "查看数据表" }}
            </button>
          </header>
          <ul class="chart-legend">
            <li>
              <span class="chart-swatch" :style="{ background: COMPLETION_SERIES[0].color }" aria-hidden="true"></span>完成
            </li>
          </ul>
          <div v-if="showCompletionTable" class="table-scroll stats-table-scroll">
            <table class="table stats-daily-table">
              <thead>
                <tr>
                  <th>{{ isWeekly ? "周" : "日期" }}</th>
                  <th class="is-number">完成</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="bucket in [...stats.buckets].reverse()" :key="bucket.date">
                  <td class="cell-time">{{ bucketLabel(bucket) }}</td>
                  <td class="is-number cell-title">{{ bucket.completions }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <StatsBarChart
            v-else
            :columns="completionColumns"
            :series="COMPLETION_SERIES"
            :dense="stats.buckets.length > 12"
            unit="项"
            empty-text="这段时间没有完成的事项"
          />
        </section>

        <section class="stats-card">
          <header class="stats-card-header">
            <div>
              <div class="stats-card-title">按标签</div>
              <div class="stats-card-subtitle">提醒按所属待办或循环提醒的标签汇总，多个标签各计一次</div>
            </div>
          </header>
          <div v-if="stats.tags.length" class="table-scroll stats-table-scroll">
            <table class="table tag-stats-table">
              <thead>
                <tr>
                  <th>标签</th>
                  <th class="is-number">提醒</th>
                  <th class="is-number">完成</th>
                  <th class="is-number">推迟</th>
                  <th class="is-number" title="完成或关闭，未推迟">处理率</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="item in stats.tags" :key="item.tag || '__untagged'">
                  <td>
                    <span v-if="item.tag" class="cell-title">#{{ item.tag }}</span>
                    <span v-else class="cell-empty">未加标签</span>
                  </td>
                  <td class="is-number cell-time">{{ item.total }}</td>
                  <td class="is-number cell-time">{{ item.counts.COMPLETED }}</td>
                  <td class="is-number cell-time">{{ item.counts.SNOOZED }}</td>
                  <td class="is-number cell-time">{{ Math.round(item.handledRate * 100) }}%</td>
                </tr>
              </tbody>
            </table>
          </div>
          <div v-else class="stats-empty">这段时间还没有提醒记录</div>
        </section>
      </div>

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
            <select v-if="habitTagOptions.length" class="select habit-tag-filter" v-model="habitTag" title="按标签筛选">
              <option value="">全部标签</option>
              <option v-for="item in habitTagOptions" :key="item.tag" :value="item.tag">#{{ item.tag }}</option>
            </select>
          </header>
          <table v-if="habits.length" class="table habit-table">
            <thead>
              <tr>
                <th>循环提醒</th>
                <th class="is-number">当前连续</th>
                <th class="is-number">最长连续</th>
                <th class="is-number">打卡天数</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="habit in habits" :key="habit.task.id">
                <td class="cell-title" :title="habit.task.description">{{ habit.task.description }}</td>
                <td class="is-number">
                  <span class="streak" :class="{ 'is-active': habit.current > 0 }">{{ habit.current }} 天</span>
                </td>
                <td class="is-number cell-time">{{ habit.best }} 天</td>
                <td class="is-number cell-time">{{ habit.checkedDays }} / {{ habit.remindedDays }}</td>
              </tr>
            </tbody>
          </table>
          <div v-else class="stats-empty">{{ habitTag ? "这个标签下没有循环提醒的记录" : "这段时间没有循环提醒的记录" }}</div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import StatsBarChart, { type ChartColumn } from "../components/StatsBarChart.vue";
import { recordDescription } from "../format";
import { computeStats, filterHabitsByTag, type StatsBucket, type StatsGranularity } from "../stats";
import { collectTags } from "../tasks";
import { completedRetentionLabel, retentionCoversRange } from "../retention";
import type { UserAction } from "../types";
import { useAppData } from "../composables/useAppData";
import { useNow } from "../composables/useNow";
import { useSettings } from "../composables/useSettings";
import { safeStorage } from "../safeStorage";

const RANGE_OPTIONS: { days: number; label: string; granularity: StatsGranularity }[] = [
  { days: 7, label: "近 7 天", granularity: "day" },
  { days: 30, label: "近 30 天", granularity: "day" },
  { days: 84, label: "近 12 周", granularity: "week" },
];

// 颜色取自 styles.css 的图表令牌（深浅主题分别校验过色觉辨识度）；
// 从下往上堆叠：完成、关闭、推迟、待处理。
const SERIES: { key: UserAction; label: string; color: string }[] = [
  { key: "COMPLETED", label: "完成", color: "var(--chart-completed)" },
  { key: "DISMISSED", label: "关闭", color: "var(--chart-dismissed)" },
  { key: "SNOOZED", label: "推迟", color: "var(--chart-snoozed)" },
  { key: "PENDING", label: "未处理", color: "var(--chart-pending)" },
];
const COMPLETION_SERIES = [{ key: "completions", label: "完成", color: "var(--chart-completed)" }];

const WEEKDAY_NAMES = ["日", "一", "二", "三", "四", "五", "六"];

const { tasks, reminderRecords, completedTasks, recurringTasks } = useAppData();
const { settingsDraft, openSettings } = useSettings();
const now = useNow(60_000);

const storedRange = Number(safeStorage.getItem("statsRangeDays"));
const rangeDays = ref(RANGE_OPTIONS.some(option => option.days === storedRange) ? storedRange : 7);
const showTable = ref(false);
const showCompletionTable = ref(false);

watch(rangeDays, days => {
  safeStorage.setItem("statsRangeDays", String(days));
});

const range = computed(() => RANGE_OPTIONS.find(option => option.days === rangeDays.value) ?? RANGE_OPTIONS[0]);
const isWeekly = computed(() => range.value.granularity === "week");

const habitTag = ref("");
const habitTagOptions = computed(() => collectTags(recurringTasks.value));
const habits = computed(() => filterHabitsByTag(stats.value.habits, habitTag.value));

const stats = computed(() =>
  computeStats({
    now: now.value,
    days: rangeDays.value,
    granularity: range.value.granularity,
    records: reminderRecords.value,
    completedTasks: completedTasks.value,
    recurringTasks: recurringTasks.value,
    activeTasks: tasks.value,
  })
);

const handledRateText = computed(() =>
  stats.value.handledRate === null ? "—" : `${Math.round(stats.value.handledRate * 100)}%`
);

const retentionLimited = computed(() => !retentionCoversRange(settingsDraft.completedRetentionDays, rangeDays.value));
const retentionLabel = computed(() => completedRetentionLabel(settingsDraft.completedRetentionDays));

const parseDay = (date: string) => new Date(`${date}T00:00:00`);
const monthDay = (date: string) => {
  const day = parseDay(date);
  return `${day.getMonth() + 1}月${day.getDate()}日`;
};
const formatTick = (date: string) => {
  const day = parseDay(date);
  return rangeDays.value <= 7 ? `周${WEEKDAY_NAMES[day.getDay()]}` : `${day.getMonth() + 1}/${day.getDate()}`;
};
const bucketLabel = (bucket: StatsBucket) => {
  if (isWeekly.value) {
    return `${monthDay(bucket.date)} – ${monthDay(bucket.endDate)}`;
  }
  return `${monthDay(bucket.date)} 周${WEEKDAY_NAMES[parseDay(bucket.date).getDay()]}`;
};
const showTick = (index: number) => {
  const count = stats.value.buckets.length;
  if (count <= 12) {
    return true;
  }
  // 30 天时每 5 天标一次，并保证最后一天（今天）有标签。
  return (count - 1 - index) % 5 === 0;
};
const toColumns = (values: (bucket: StatsBucket) => Record<string, number>, total: (bucket: StatsBucket) => number) =>
  stats.value.buckets.map<ChartColumn>((bucket, index) => ({
    key: bucket.date,
    label: bucketLabel(bucket),
    tick: formatTick(bucket.date),
    showTick: showTick(index),
    values: values(bucket),
    total: total(bucket),
  }));
const reminderColumns = computed(() => toColumns(bucket => bucket.counts, bucket => bucket.total));
const completionColumns = computed(() =>
  toColumns(bucket => ({ completions: bucket.completions }), bucket => bucket.completions)
);
</script>

<style scoped>
.habit-tag-filter {
  height: 32px;
  min-width: 112px;
}
</style>
