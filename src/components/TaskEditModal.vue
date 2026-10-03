<template>
  <Modal :open="taskEditor.open" title="编辑任务" :showDelete="true" @close="close" @confirm="save" @delete="remove">
    <div class="form-row">
      <input class="input" v-model="description" placeholder="任务标题" style="flex: 1" />
    </div>
    <div class="form-row form-row-markdown">
      <MarkdownNoteEditor
        ref="noteEditor"
        v-model="stickyContent"
        class="task-markdown-editor task-markdown-editor-modal"
        :theme="isLightTheme ? 'light' : 'dark'"
        placeholder="输入任务描述，支持 Markdown 所见即所得"
      />
    </div>
    <div class="form-row compact">
      <span class="form-row-label">截止时间</span>
      <input
        ref="dueInput"
        class="input"
        type="datetime-local"
        v-model="due"
        @change="handlePicked(dueInput)"
      />
      <select class="select" v-model="lead" :disabled="!due" title="提醒时间 = 截止时间 - 提前量">
        <option v-for="option in LEAD_OPTIONS" :key="option.value" :value="option.value">{{ option.label }}</option>
        <option v-if="lead === 'custom'" value="custom">自定义提醒时间</option>
      </select>
      <button v-if="due" class="button secondary" @click="clearDue">清除截止</button>
    </div>
    <div class="form-row compact">
      <span class="form-row-label">提醒时间</span>
      <input
        ref="reminderInput"
        class="input"
        type="datetime-local"
        v-model="reminder"
        @input="syncLeadFromReminder"
        @change="handlePicked(reminderInput)"
      />
      <button class="button secondary" @click="clearReminder">清除提醒</button>
    </div>
    <div class="form-row compact">
      <span class="form-row-label">优先级</span>
      <PriorityPicker v-model="priority" />
    </div>
    <div class="form-row compact">
      <span class="form-row-label">标签</span>
      <TagInput v-model="tags" :suggestions="tagSuggestions" />
    </div>
    <div class="form-row compact">
      <span class="form-row-label">项目</span>
      <input
        class="input"
        v-model="project"
        list="task-project-suggestions"
        :maxlength="MAX_PROJECT_CHARS"
        placeholder="未分组"
        style="width: 220px"
      />
      <datalist id="task-project-suggestions">
        <option v-for="item in projectSuggestions" :key="item.project" :value="item.project" />
      </datalist>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import Modal from "./Modal.vue";
import MarkdownNoteEditor from "./MarkdownNoteEditor.vue";
import PriorityPicker from "./PriorityPicker.vue";
import TagInput from "./TagInput.vue";
import { MAX_PROJECT_CHARS, collectProjects, collectTags, normalizeProject, priorityOf } from "../tasks";
import { LEAD_OPTIONS, NO_REMINDER, leadOf, reminderForDue, type LeadChoice } from "../due";
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
// 截止时间与提前提醒（v2.1）：选择提前量时按截止时间算出提醒时间；手动改提醒时间时反推提前量。
const due = ref("");
const dueInput = ref<HTMLInputElement | null>(null);
const lead = ref<LeadChoice>(NO_REMINDER);
// 打开弹窗时载入已有的值，不触发“按提前量重算提醒时间”。
let loading = false;
const priority = ref(0);
const tags = ref<string[]>([]);
const tagSuggestions = computed(() => collectTags([...tasks.value, ...completedTasks.value]).map(item => item.tag));
// 项目（v2.2）：建议来自已有待办的项目。
const project = ref("");
const projectSuggestions = computed(() => collectProjects([...tasks.value, ...completedTasks.value]));

watch(
  () => taskEditor.open,
  open => {
    const task = taskEditor.task;
    if (!open || !task) {
      return;
    }
    description.value = task.description;
    stickyContent.value = task.stickyContent || "";
    loading = true;
    void nextTick(() => {
      loading = false;
    });
    reminder.value = toDatetimeLocal(task.reminderTime ?? null);
    due.value = toDatetimeLocal(task.dueAt ?? null);
    lead.value = due.value ? leadOf(due.value, reminder.value) : reminder.value ? "custom" : NO_REMINDER;
    priority.value = priorityOf(task);
    tags.value = [...(task.tags ?? [])];
    project.value = task.project ?? "";
  }
);

const close = () => {
  taskEditor.open = false;
};

// Linux WebKitGTK 的日期选择器选完后不会自动收起，其他平台主动失焦关闭。
const handlePicked = (input: HTMLInputElement | null) => {
  if (isLinuxPlatform) {
    return;
  }
  requestAnimationFrame(() => {
    input?.blur();
  });
};

watch([due, lead], ([dueValue, leadValue], [previousDue]) => {
  if (loading || !dueValue || leadValue === "custom") {
    return;
  }
  // 第一次设置截止时间且原本没有提醒时，默认准时提醒。
  if (!previousDue && leadValue === NO_REMINDER && !reminder.value) {
    lead.value = 0;
    return;
  }
  reminder.value = reminderForDue(dueValue, leadValue);
});

const syncLeadFromReminder = () => {
  lead.value = due.value ? leadOf(due.value, reminder.value) : "custom";
};

const clearReminder = () => {
  reminder.value = "";
  lead.value = NO_REMINDER;
};

const clearDue = () => {
  due.value = "";
  lead.value = reminder.value ? "custom" : NO_REMINDER;
};

const noteEditor = ref<InstanceType<typeof MarkdownNoteEditor> | null>(null);

const save = async () => {
  // 编辑器内容变更有 200ms 防抖：保存前先取当前内容，避免丢掉最后的输入。
  noteEditor.value?.flush();
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
    dueAt: fromDatetimeLocal(due.value),
    tags: tags.value,
    priority: priority.value,
    project: normalizeProject(project.value)
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
