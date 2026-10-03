<template>
  <div class="daily-chart" :class="{ 'is-dense': dense }" @pointerleave="hovered = null">
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
        v-for="(column, index) in columns"
        :key="column.key"
        class="chart-column"
        :class="{ 'is-hovered': hovered === index }"
        tabindex="0"
        :aria-label="columnLabel(column)"
        @pointerenter="hovered = index"
        @focus="hovered = index"
        @blur="hovered = null"
      >
        <div class="chart-stack" :style="{ height: `${(column.total / axisMax) * 100}%` }">
          <span
            v-for="item in series"
            v-show="column.values[item.key] > 0"
            :key="item.key"
            class="chart-segment"
            :style="{ flexGrow: column.values[item.key], background: item.color }"
          ></span>
        </div>
        <span class="chart-x-label" :class="{ 'is-hidden': !column.showTick }">{{ column.tick }}</span>
      </div>
      <div
        v-if="hoveredColumn"
        class="chart-tooltip"
        :class="{ 'is-right': hovered !== null && hovered > columns.length / 2 }"
        :style="tooltipStyle"
        role="status"
      >
        <div class="chart-tooltip-title">{{ hoveredColumn.label }} · 共 {{ hoveredColumn.total }} {{ unit }}</div>
        <div v-for="item in series" :key="item.key" class="chart-tooltip-row">
          <span class="chart-tooltip-key" :style="{ background: item.color }" aria-hidden="true"></span>
          <strong>{{ hoveredColumn.values[item.key] }}</strong>
          <span>{{ item.label }}</span>
        </div>
      </div>
    </div>
    <div v-if="columns.every(column => column.total === 0)" class="chart-empty">{{ emptyText }}</div>
  </div>
</template>

<script setup lang="ts">
// 统计面板的堆叠柱状图（每日 / 每周提醒、完成趋势共用）：纵轴取整、网格线、悬停或聚焦时显示明细。
import { computed, ref } from "vue";

export interface ChartSeries {
  key: string;
  label: string;
  color: string;
}

export interface ChartColumn {
  key: string;
  /** 提示与无障碍标签中的完整名称，如“9月26日 周六”。 */
  label: string;
  /** 横轴上的短标签。 */
  tick: string;
  showTick: boolean;
  values: Record<string, number>;
  total: number;
}

const props = defineProps<{
  columns: ChartColumn[];
  series: ChartSeries[];
  dense?: boolean;
  emptyText: string;
  unit: string;
}>();

const hovered = ref<number | null>(null);

/** 纵轴取整：最大值向上取到 1/2/5×10ⁿ，分 4 格以内。 */
const niceStep = (max: number) => {
  const raw = Math.max(1, max) / 4;
  const power = 10 ** Math.floor(Math.log10(raw));
  const normalized = raw / power;
  const step = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
  return Math.max(1, step * power);
};
const maxTotal = computed(() => Math.max(0, ...props.columns.map(column => column.total)));
const axisStep = computed(() => niceStep(maxTotal.value));
const axisMax = computed(() => Math.max(axisStep.value, Math.ceil(maxTotal.value / axisStep.value) * axisStep.value));
const ticks = computed(() => {
  const list: number[] = [];
  for (let value = 0; value <= axisMax.value; value += axisStep.value) {
    list.push(value);
  }
  return list;
});

const hoveredColumn = computed(() => (hovered.value === null ? null : props.columns[hovered.value] ?? null));
const tooltipStyle = computed(() => {
  if (hovered.value === null) {
    return {};
  }
  const center = ((hovered.value + 0.5) / props.columns.length) * 100;
  return { left: `${center}%` };
});

const columnLabel = (column: ChartColumn) =>
  `${column.label}：共 ${column.total} ${props.unit}，` +
  props.series.map(item => `${item.label} ${column.values[item.key]}`).join("，");
</script>
