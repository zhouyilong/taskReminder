<template>
  <template v-if="draft.mode === 'INTERVAL_RANGE'">
    <label class="field-label">间隔</label>
    <input class="input" type="number" v-model.number="draft.intervalMinutes" min="1" placeholder="分钟" style="width: 100px" />
    <label class="field-label">开始</label>
    <input class="input" type="time" v-model="draft.startTime" style="width: 120px" />
    <label class="field-label">结束</label>
    <input class="input" type="time" v-model="draft.endTime" style="width: 120px" />
  </template>
  <template v-else-if="draft.mode === 'DAILY'">
    <label class="field-label">每天</label>
    <input class="input" type="time" v-model="draft.scheduleTime" style="width: 140px" />
  </template>
  <template v-else-if="draft.mode === 'WEEKLY'">
    <label class="field-label">周几</label>
    <WeekdayPicker v-model="draft.scheduleWeekdays" />
    <label class="field-label">时间</label>
    <input class="input" type="time" v-model="draft.scheduleTime" style="width: 140px" />
  </template>
  <template v-else-if="draft.mode === 'WORKDAY'">
    <label class="field-label">时间</label>
    <input class="input" type="time" v-model="draft.scheduleTime" style="width: 140px" />
    <span class="field-hint">{{ workdayHint }}</span>
    <span v-if="workdayWarning" class="field-hint is-warning">{{ workdayWarning }}</span>
  </template>
  <template v-else-if="draft.mode === 'MONTHLY'">
    <label class="field-label">每月几号</label>
    <input class="input" type="number" min="1" max="31" v-model.number="draft.scheduleDay" style="width: 120px" />
    <label class="field-label">时间</label>
    <input class="input" type="time" v-model="draft.scheduleTime" style="width: 140px" />
  </template>
  <template v-else>
    <label class="field-label">Cron</label>
    <input class="input" v-model="draft.cronExpression" placeholder="分 时 日 月 周，如 0 9 * * 1-5（周一至周五 9 点）" style="flex: 1" />
  </template>
</template>

<script setup lang="ts">
// 循环提醒各模式的规则字段，新建表单与编辑弹窗共用；直接修改传入的草稿对象。
import { computed } from "vue";
import WeekdayPicker from "./WeekdayPicker.vue";
import { formatWorkdayHint, workdayDraftWarning, type RecurringDraft } from "../recurring";
import { useAppData } from "../composables/useAppData";

defineProps<{ draft: RecurringDraft }>();

const { holidayYears } = useAppData();
const workdayHint = computed(() => formatWorkdayHint(holidayYears.value));
const workdayWarning = computed(() => workdayDraftWarning(holidayYears.value));
</script>
