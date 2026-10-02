<template>
  <div class="batch-bar" role="toolbar" aria-label="批量操作">
    <span class="batch-count">已选 {{ ids.length }} 项</span>
    <template v-if="mode === 'tags'">
      <input
        ref="tagInputEl"
        v-model="tagText"
        class="input batch-input"
        placeholder="输入标签，空格分隔，如 #工作 周报"
        @keydown.enter.prevent="applyTags"
        @keydown.esc.stop.prevent="mode = null"
      />
      <button class="button" :disabled="!parsedTags.length || busy" @click="applyTags">添加</button>
      <button class="button secondary" @click="mode = null">返回</button>
    </template>
    <template v-else-if="mode === 'reminder'">
      <input
        ref="reminderInputEl"
        v-model="reminderText"
        class="input batch-input"
        placeholder="如“明天下午3点”“周五 9:00”，留空清除提醒"
        @keydown.enter.prevent="applyReminder"
        @keydown.esc.stop.prevent="mode = null"
      />
      <span class="batch-hint" :class="{ 'is-error': reminder.error }">{{ reminderHint }}</span>
      <button class="button" :disabled="Boolean(reminder.error) || busy" @click="applyReminder">
        {{ reminderText.trim() ? "改期" : "清除提醒" }}
      </button>
      <button class="button secondary" @click="mode = null">返回</button>
    </template>
    <template v-else>
      <button class="button" :disabled="!ids.length || busy" @click="run({ action: 'complete' })">完成</button>
      <button class="button secondary" :disabled="!ids.length" @click="openMode('tags')">添加标签</button>
      <select class="select batch-select" :disabled="!ids.length || busy" title="设置优先级" @change="applyPriority">
        <option value="" selected disabled>优先级…</option>
        <option v-for="option in priorityOptions" :key="option.value" :value="option.value">
          {{ option.value ? `${option.label}优先级` : "无优先级" }}
        </option>
      </select>
      <button class="button secondary" :disabled="!ids.length" @click="openMode('reminder')">改提醒时间</button>
      <button class="button danger" :disabled="!ids.length || busy" @click="confirmDelete">删除</button>
    </template>
    <span class="batch-spacer" />
    <button class="button ghost" title="退出多选（Esc）" @click="emit('cancel')">取消</button>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { api } from "../api";
import { formatDateTime, toLocalDateTimeString } from "../format";
import { parseRescheduleTime } from "../nlp";
import { PRIORITY_OPTIONS, normalizeTags } from "../tasks";
import type { TaskBatchPayload } from "../types";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";

const props = defineProps<{ ids: string[] }>();
const emit = defineEmits<{ (event: "done"): void; (event: "cancel"): void }>();

const { refreshAll } = useAppData();
const { confirmAction } = useDialogs();

const priorityOptions = [...PRIORITY_OPTIONS].reverse();
const mode = ref<null | "tags" | "reminder">(null);
const busy = ref(false);
const tagText = ref("");
const reminderText = ref("");
const tagInputEl = ref<HTMLInputElement | null>(null);
const reminderInputEl = ref<HTMLInputElement | null>(null);

const parsedTags = computed(() => normalizeTags(tagText.value.split(/[\s,，]+/)));
const reminder = computed(() => parseRescheduleTime(reminderText.value));
const reminderHint = computed(() => {
  if (reminder.value.error) return reminder.value.error;
  if (reminder.value.time) return `改到 ${formatDateTime(toLocalDateTimeString(reminder.value.time))}`;
  return "";
});

const openMode = async (next: "tags" | "reminder") => {
  mode.value = next;
  tagText.value = "";
  reminderText.value = "";
  await nextTick();
  (next === "tags" ? tagInputEl.value : reminderInputEl.value)?.focus();
};

const run = async (payload: TaskBatchPayload) => {
  if (!props.ids.length || busy.value) return;
  busy.value = true;
  try {
    await api.batchUpdateTasks([...props.ids], payload);
    mode.value = null;
    emit("done");
  } catch (error) {
    console.error("[tasks] 批量操作失败", error);
  } finally {
    busy.value = false;
    await refreshAll();
  }
};

const applyTags = () => {
  if (parsedTags.value.length) void run({ action: "addTags", tags: parsedTags.value });
};

const applyPriority = (event: Event) => {
  const select = event.target as HTMLSelectElement;
  const priority = Number(select.value);
  select.value = "";
  void run({ action: "setPriority", priority });
};

const applyReminder = () => {
  if (reminder.value.error) return;
  const time = reminder.value.time;
  void run({ action: "setReminder", reminderTime: time ? toLocalDateTimeString(time) : null });
};

const confirmDelete = () => {
  confirmAction({
    message: `确定要删除选中的 ${props.ids.length} 项待办吗？删除后可在回收站中恢复。`,
    action: () => run({ action: "delete" })
  });
};
</script>

<style scoped>
.batch-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  margin-bottom: 10px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--primary-soft);
}

.batch-count {
  font-weight: 600;
  color: var(--primary-text);
  margin-right: 4px;
}

.batch-input {
  flex: 1 1 220px;
  min-width: 180px;
  height: 34px;
}

.batch-select {
  height: 34px;
  min-width: 108px;
}

.batch-hint {
  font-size: 12px;
  color: var(--text-muted);
}

.batch-hint.is-error {
  color: var(--danger-text);
}

.batch-spacer {
  flex: 1 1 auto;
}
</style>
