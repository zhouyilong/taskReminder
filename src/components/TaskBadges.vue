<template>
  <span v-if="project || priority || tags.length" class="task-badges">
    <button
      v-if="project"
      type="button"
      class="task-project"
      :class="{ 'is-active': activeProject !== undefined && activeProject !== null && activeProject.toLowerCase() === project.toLowerCase() }"
      :title="clickable ? `只看项目 ${project}` : `项目：${project}`"
      :disabled="!clickable"
      @click.stop="emit('select-project', project)"
      @dblclick.stop
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M4 7.5A1.5 1.5 0 0 1 5.5 6h4l2 2h7A1.5 1.5 0 0 1 20 9.5v7a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 16.5z" />
      </svg>
      <span class="task-project-name">{{ project }}</span>
    </button>
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
  task: Pick<Task, "tags" | "priority"> & { project?: string };
  clickable?: boolean;
  activeTag?: string;
  /** 当前筛选的项目（v2.2）；null / undefined 表示没有按项目筛选。 */
  activeProject?: string | null;
}>();

const emit = defineEmits<{
  (e: "select-tag", tag: string): void;
  (e: "select-project", project: string): void;
}>();

const priority = computed(() => priorityOf(props.task));
const tags = computed(() => props.task.tags ?? []);
const project = computed(() => props.task.project ?? "");
</script>
