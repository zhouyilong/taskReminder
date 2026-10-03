<template>
  <div v-if="error || hasMeta || !enabled" class="composer-parse">
    <template v-if="enabled && parsed && hasMeta">
      <span class="composer-parse-label">识别</span>
      <span v-if="parsed.recurring" class="chip is-recurring">循环 · {{ scheduleText }}</span>
      <span v-else-if="parsed.reminderTime" class="time-chip is-upcoming">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="8" />
          <path d="M12 8v4l2.5 1.5" />
        </svg>
        {{ scheduleText }}
      </span>
      <TaskBadges
        :task="{
          tags: parsed.tags,
          priority: parsed.recurring ? 0 : parsed.priority,
          project: parsed.recurring ? '' : parsed.project
        }"
      />
      <span v-if="parsed.recurring && unsavedText" class="composer-parse-hint">
        循环提醒不保存{{ unsavedText }}
      </span>
    </template>
    <span v-else-if="!enabled" class="composer-parse-hint">已关闭识别，按原文添加</span>
    <span v-if="error" class="composer-parse-hint is-error">{{ error }}</span>
    <button v-if="hasMeta || !enabled" type="button" class="composer-parse-toggle" @click="enabled = !enabled">
      {{ enabled ? "按原文添加" : "恢复识别" }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import TaskBadges from "./TaskBadges.vue";
import type { ParsedInput } from "../nlp";

const props = defineProps<{
  parsed: ParsedInput | null;
  hasMeta: boolean;
  scheduleText: string;
  error: string;
  hasNote?: boolean;
}>();

const enabled = defineModel<boolean>("enabled", { required: true });

// 循环提醒没有优先级、项目与描述，识别出来时提示不会保存。
const unsavedText = computed(() => {
  const parsed = props.parsed;
  if (!parsed?.recurring) return "";
  const parts = [
    parsed.priority ? "优先级" : "",
    parsed.project ? "项目" : "",
    props.hasNote ? "描述" : ""
  ].filter(Boolean);
  return parts.join("、");
});
</script>
