<template>
  <Modal :open="taskEditor.open" title="编辑任务" :showDelete="true" @close="close" @confirm="save" @delete="remove">
    <div class="form-row">
      <input class="input" v-model="description" placeholder="任务标题" style="flex: 1" />
    </div>
    <div class="form-row form-row-markdown">
      <MarkdownNoteEditor
        v-model="stickyContent"
        class="task-markdown-editor task-markdown-editor-modal"
        :theme="isLightTheme ? 'light' : 'dark'"
        placeholder="输入任务描述，支持 Markdown 所见即所得"
      />
    </div>
    <div class="form-row">
      <input
        ref="reminderInput"
        class="input"
        type="datetime-local"
        v-model="reminder"
        @change="handleReminderPicked"
      />
      <button class="button secondary" @click="reminder = ''">清除提醒</button>
    </div>
    <div class="form-row compact">
      <span class="form-row-label">优先级</span>
      <PriorityPicker v-model="priority" />
    </div>
    <div class="form-row compact">
      <span class="form-row-label">标签</span>
      <TagInput v-model="tags" :suggestions="tagSuggestions" />
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Modal from "./Modal.vue";
import MarkdownNoteEditor from "./MarkdownNoteEditor.vue";
import PriorityPicker from "./PriorityPicker.vue";
import TagInput from "./TagInput.vue";
import { collectTags, priorityOf } from "../tasks";
import { api } from "../api";
import { fromDatetimeLocal, isLinuxPlatform, toDatetimeLocal } from "../format";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";
import { useItemActions } from "../composables/useItemActions";
import { useUiPrefs } from "../composables/useUiPrefs";

const { taskEditor } = useDialogs();
const { tasks, completedTasks, refreshAll } = useAppData();
const { confirmDeleteTask } = useItemActions();
const { isLightTheme } = useUiPrefs();

const description = ref("");
const stickyContent = ref("");
const reminder = ref("");
const reminderInput = ref<HTMLInputElement | null>(null);
const priority = ref(0);
const tags = ref<string[]>([]);
const tagSuggestions = computed(() => collectTags([...tasks.value, ...completedTasks.value]).map(item => item.tag));

watch(
  () => taskEditor.open,
  open => {
    const task = taskEditor.task;
    if (!open || !task) {
      return;
    }
    description.value = task.description;
    stickyContent.value = task.stickyContent || "";
    reminder.value = toDatetimeLocal(task.reminderTime ?? null);
    priority.value = priorityOf(task);
    tags.value = [...(task.tags ?? [])];
  }
);

const close = () => {
  taskEditor.open = false;
};

// Linux WebKitGTK 的日期选择器选完后不会自动收起，其他平台主动失焦关闭。
const handleReminderPicked = () => {
  if (isLinuxPlatform) {
    return;
  }
  requestAnimationFrame(() => {
    reminderInput.value?.blur();
  });
};

const save = async () => {
  const task = taskEditor.task;
  if (!task) {
    close();
    return;
  }
  await api.updateTask({
    id: task.id,
    description: description.value,
    stickyContent: stickyContent.value.trim() ? stickyContent.value : null,
    reminderTime: fromDatetimeLocal(reminder.value),
    tags: tags.value,
    priority: priority.value
  });
  close();
  await refreshAll();
};

const remove = () => {
  const task = taskEditor.task;
  if (!task) {
    close();
    return;
  }
  confirmDeleteTask(task, close);
};
</script>
