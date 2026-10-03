<template>
  <span
    v-if="progress"
    class="checklist-chip"
    :class="{ 'is-done': progress.done === progress.total }"
    :title="`清单已完成 ${progress.done} / ${progress.total} 项`"
    :aria-label="`清单已完成 ${progress.done} / ${progress.total} 项`"
  >
    <span class="checklist-bar" aria-hidden="true">
      <span class="checklist-bar-fill" :style="{ width: `${(progress.done / progress.total) * 100}%` }"></span>
    </span>
    {{ progress.done }}/{{ progress.total }}
  </span>
</template>

<script setup lang="ts">
// 待办正文（便签内容）中勾选清单的进度，如“2/5”；没有清单时不显示。
import { computed } from "vue";
import { checklistProgress } from "../markdown";

const props = defineProps<{ content?: string | null }>();
const progress = computed(() => checklistProgress(props.content));
</script>
