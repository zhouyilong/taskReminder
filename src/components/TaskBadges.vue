<template>
  <span v-if="priority || tags.length" class="task-badges">
    <span v-if="priority" class="priority-badge" :class="`is-p${priority}`" :title="`优先级：${priorityLabel(priority)}`">
      {{ priorityLabel(priority) }}
    </span>
    <button
      v-for="tag in tags"
      :key="tag"
      type="button"
      class="task-tag"
      :class="{ 'is-active': activeTag && activeTag.toLowerCase() === tag.toLowerCase() }"
      :title="clickable ? `只看 #${tag}` : `#${tag}`"
      :disabled="!clickable"
      @click.stop="emit('select-tag', tag)"
      @dblclick.stop
    >
      #{{ tag }}
    </button>
  </span>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { priorityLabel, priorityOf } from "../tasks";
import type { Task } from "../types";

const props = defineProps<{
  task: Pick<Task, "tags" | "priority">;
  clickable?: boolean;
  activeTag?: string;
}>();

const emit = defineEmits<{ (e: "select-tag", tag: string): void }>();

const priority = computed(() => priorityOf(props.task));
const tags = computed(() => props.task.tags ?? []);
</script>
