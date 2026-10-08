<template>
  <input
    class="input"
    type="date"
    v-model="draft.endsOn"
    title="到这一天（含当天）为止，留空不限"
    style="width: 160px"
  />
  <button v-if="draft.endsOn" class="button secondary" type="button" @click="draft.endsOn = ''">不限日期</button>
  <label class="field-label">{{ editing ? "还剩" : "共" }}</label>
  <input
    class="input"
    type="number"
    min="0"
    :max="MAX_REPEAT_COUNT"
    step="1"
    v-model.number="draft.count"
    placeholder="不限"
    title="提醒这么多次后自动结束，留空不限"
    style="width: 90px"
  />
  <span class="field-unit">次</span>
  <span v-if="hint" class="field-hint">{{ hint }}</span>
</template>

<script setup lang="ts">
// 循环提醒的结束条件（v2.2）：结束日期与次数，都可留空；新建表单与编辑弹窗共用，直接修改传入的草稿。
// 行首的“结束”标签由调用方提供（两处的标签样式不同）。
import { computed } from "vue";
import { MAX_REPEAT_COUNT, type RecurringDraft } from "../recurring";

const props = defineProps<{ draft: RecurringDraft; editing?: boolean }>();

const hint = computed(() => {
  const hasCount = typeof props.draft.count === "number";
  if (props.draft.endsOn && hasCount) {
    return "先到的条件生效";
  }
  if (props.editing && props.draft.count === 0) {
    return "次数已用完，改为大于 0 后自动恢复";
  }
  return "";
});
</script>
