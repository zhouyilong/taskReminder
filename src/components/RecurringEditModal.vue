<template>
  <Modal :open="recurringEditor.open" title="编辑循环提醒" :showDelete="true" @close="close" @confirm="save" @delete="remove">
    <div class="form-row">
      <input class="input" v-model="draft.description" placeholder="提醒描述" style="flex: 1" />
    </div>
    <div class="form-row compact">
      <label class="field-label">模式</label>
      <select class="select" v-model="draft.mode" style="width: 180px">
        <option v-for="mode in recurringModeOptions" :key="mode.value" :value="mode.value">{{ mode.label }}</option>
      </select>
    </div>
    <div class="form-row compact">
      <RecurringFields :draft="draft" />
    </div>
    <div class="form-row compact">
      <span class="form-row-label">标签</span>
      <TagInput v-model="draft.tags" :suggestions="tagSuggestions" />
    </div>
    <div class="form-row compact">
      <span class="form-row-label">结束条件</span>
      <RecurringEndFields :draft="draft" editing />
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import Modal from "./Modal.vue";
import RecurringEndFields from "./RecurringEndFields.vue";
import RecurringFields from "./RecurringFields.vue";
import TagInput from "./TagInput.vue";
import { collectTags } from "../tasks";
import { api } from "../api";
import { errorMessage } from "../format";
import {
  buildRecurringPayload,
  createRecurringDraft,
  draftFromRecurringTask,
  recurringModeOptions,
  validateRecurringDraft
} from "../recurring";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";
import { useItemActions } from "../composables/useItemActions";

const { recurringEditor } = useDialogs();
const { tasks, completedTasks, recurringTasks, refreshAll } = useAppData();
const tagSuggestions = computed(() =>
  collectTags([...recurringTasks.value, ...tasks.value, ...completedTasks.value]).map(item => item.tag)
);
const { confirmDeleteRecurring } = useItemActions();

const draft = reactive(createRecurringDraft());

watch(
  () => recurringEditor.open,
  open => {
    if (open && recurringEditor.task) {
      Object.assign(draft, draftFromRecurringTask(recurringEditor.task));
    }
  }
);

const close = () => {
  recurringEditor.open = false;
};

const save = async () => {
  const id = recurringEditor.task?.id;
  // 以最新数据为基准，避免覆盖打开弹窗后发生的变化（如暂停状态）。
  const target = recurringTasks.value.find(item => item.id === id) ?? recurringEditor.task;
  if (!target) {
    close();
    return;
  }
  const error = validateRecurringDraft(draft);
  if (error) {
    alert(error);
    return;
  }
  try {
    await api.updateRecurringTask({ ...target, ...buildRecurringPayload(draft) });
  } catch (error) {
    alert(errorMessage(error));
    return;
  }
  close();
  await refreshAll();
};

const remove = () => {
  const task = recurringEditor.task;
  if (!task) {
    close();
    return;
  }
  confirmDeleteRecurring(task, close);
};
</script>
