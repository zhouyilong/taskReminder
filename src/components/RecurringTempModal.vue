<template>
  <Modal :open="tempReminderEditor.open" title="临时提醒" @close="close" @confirm="save">
    <div v-if="task" class="temp-reminder-form">
      <div class="modal-text temp-reminder-target" :title="task.description">{{ task.description }}</div>
      <div class="field-hint">
        规则不变，只在下面的时间点提醒一次，提醒后自动清除{{ task.isPaused ? "（暂停中也会提醒）" : "" }}。
      </div>
      <div class="form-row compact">
        <label class="field-label" for="temp-reminder-at">提醒时间</label>
        <input id="temp-reminder-at" v-model="at" class="input" type="datetime-local" style="width: 220px" />
      </div>
      <div class="form-row compact">
        <label :title="replaceTitle" :class="{ 'is-disabled': !target }">
          <input v-model="replaceNext" type="checkbox" :disabled="!target" /> 替代最近的一次常规提醒
        </label>
      </div>
      <div class="field-hint" :class="{ 'is-warning': !target && !loadingTarget }">{{ replaceHint }}</div>
      <div v-if="current" class="form-row compact temp-reminder-current">
        <span class="field-hint">当前：{{ current.text }}</span>
        <button type="button" class="button secondary" @click="clear">取消临时提醒</button>
      </div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { errorMessage, fromDatetimeLocal } from "../format";
import { defaultTempReminderTime, describeTempReminder, formatTempTime } from "../recurring";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";

const { tempReminderEditor } = useDialogs();
const { recurringTasks, refreshAll } = useAppData();

// 以列表中的最新数据为准（打开后可能已触发或被其他设备修改）。
const task = computed(() => {
  const id = tempReminderEditor.task?.id;
  return recurringTasks.value.find(item => item.id === id) ?? tempReminderEditor.task;
});
const current = computed(() => (task.value ? describeTempReminder(task.value) : null));

const at = ref("");
const replaceNext = ref(true);
const target = ref<string | null>(null);
const loadingTarget = ref(false);
let previewSeq = 0;

watch(
  () => tempReminderEditor.open,
  open => {
    if (!open || !task.value) return;
    at.value = defaultTempReminderTime(task.value);
    // 已有临时提醒时沿用它是否替代；新设置的默认替代（如晨会改时间）。
    replaceNext.value = task.value.tempTrigger ? Boolean(task.value.tempReplaces) : true;
  }
);

// 时间变化后向后端询问会替代哪一次（取离它最近的一次常规提醒）。
watch([() => tempReminderEditor.open, at], async ([open, value]) => {
  const id = task.value?.id;
  const iso = fromDatetimeLocal(value);
  if (!open || !id || !iso) {
    target.value = null;
    return;
  }
  const seq = ++previewSeq;
  loadingTarget.value = true;
  try {
    const result = await api.previewRecurringTempTarget(id, iso);
    if (seq === previewSeq) target.value = result;
  } catch {
    if (seq === previewSeq) target.value = null;
  } finally {
    if (seq === previewSeq) loadingTarget.value = false;
  }
});

const replaceHint = computed(() => {
  if (loadingTarget.value) return "正在计算替代哪一次…";
  if (!target.value) return "没有可替代的常规提醒（已暂停、已结束或之后不再提醒），只会多提醒一次。";
  const text = formatTempTime(target.value);
  return replaceNext.value ? `勾选后 ${text} 的那一次不再提醒。` : `${text} 的那一次照常提醒。`;
});
const replaceTitle = computed(() => (target.value ? `替代 ${formatTempTime(target.value)} 的提醒` : "没有可替代的常规提醒"));

const close = () => {
  tempReminderEditor.open = false;
};

const save = async () => {
  const id = task.value?.id;
  const iso = fromDatetimeLocal(at.value);
  if (!id) {
    close();
    return;
  }
  if (!iso) {
    alert("请选择临时提醒的时间");
    return;
  }
  if (new Date(iso).getTime() <= Date.now()) {
    alert("临时提醒时间需晚于现在");
    return;
  }
  try {
    await api.setRecurringTempReminder(id, iso, replaceNext.value && Boolean(target.value));
  } catch (error) {
    alert(errorMessage(error));
    return;
  }
  close();
  await refreshAll();
};

const clear = async () => {
  const id = task.value?.id;
  if (!id) return;
  try {
    await api.setRecurringTempReminder(id, null, false);
  } catch (error) {
    alert(errorMessage(error));
    return;
  }
  close();
  await refreshAll();
};
</script>

<style scoped>
.temp-reminder-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 340px;
}

.temp-reminder-target {
  color: var(--text-title);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.temp-reminder-current {
  justify-content: space-between;
  padding-top: 6px;
  border-top: 1px solid var(--border);
}

label.is-disabled {
  color: var(--text-faint);
}
</style>
