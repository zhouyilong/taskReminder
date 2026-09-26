<template>
  <div class="quick-add" @keydown.esc.prevent="closeWindow">
    <div class="quick-add-header" data-tauri-drag-region>
      <span class="quick-add-title" data-tauri-drag-region>快速添加待办</span>
      <span class="quick-add-hint" data-tauri-drag-region>Enter 保存 · Esc 关闭</span>
    </div>
    <input
      ref="inputRef"
      v-model="description"
      class="input quick-add-input"
      placeholder="要做什么？"
      maxlength="500"
      @keydown.enter.prevent="submit"
    />
    <div class="quick-add-reminders" role="radiogroup" aria-label="提醒时间">
      <button
        v-for="option in reminderOptions"
        :key="option.key"
        type="button"
        class="quick-add-chip"
        :class="{ 'is-active': selectedKey === option.key }"
        :aria-checked="selectedKey === option.key"
        role="radio"
        @click="selectOption(option.key)"
      >
        {{ option.label }}
      </button>
      <input
        v-if="selectedKey === 'custom'"
        v-model="customTime"
        class="input quick-add-custom"
        type="datetime-local"
        @keydown.enter.prevent="submit"
      />
    </div>
    <div class="quick-add-footer">
      <span v-if="errorMessage" class="quick-add-message is-error">{{ errorMessage }}</span>
      <span v-else-if="savedMessage" class="quick-add-message is-success">{{ savedMessage }}</span>
      <span v-else class="quick-add-message">{{ reminderSummary }}</span>
      <button class="button" type="button" :disabled="saving || !description.trim()" @click="submit">
        {{ saving ? "保存中..." : "添加" }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "./api";
import { safeStorage } from "./safeStorage";

type ReminderKey = "none" | "30m" | "1h" | "tonight" | "tomorrow" | "custom";

const description = ref("");
const selectedKey = ref<ReminderKey>("none");
const customTime = ref("");
const saving = ref(false);
const errorMessage = ref("");
const savedMessage = ref("");
const inputRef = ref<HTMLInputElement | null>(null);
let unlistenReset: UnlistenFn | null = null;
let storageHandler: ((event: StorageEvent) => void) | null = null;
let hideTimer: number | null = null;

const pad = (value: number) => value.toString().padStart(2, "0");
const formatLocal = (date: Date) =>
  `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(
    date.getMinutes()
  )}`;
const formatDisplay = (date: Date) => {
  const now = new Date();
  const sameDay = date.toDateString() === now.toDateString();
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  return sameDay ? `今天 ${time}` : `${date.getMonth() + 1}月${date.getDate()}日 ${time}`;
};

const atTime = (hours: number, dayOffset = 0) => {
  const date = new Date();
  date.setDate(date.getDate() + dayOffset);
  date.setHours(hours, 0, 0, 0);
  return date;
};

const reminderOptions = computed(() => {
  const now = new Date();
  const options: { key: ReminderKey; label: string }[] = [
    { key: "none", label: "不提醒" },
    { key: "30m", label: "30 分钟后" },
    { key: "1h", label: "1 小时后" }
  ];
  if (now.getHours() < 20) {
    options.push({ key: "tonight", label: "今晚 20:00" });
  }
  options.push({ key: "tomorrow", label: "明早 9:00" });
  options.push({ key: "custom", label: "自定义" });
  return options;
});

const resolveReminder = (): Date | null => {
  const now = Date.now();
  switch (selectedKey.value) {
    case "30m":
      return new Date(now + 30 * 60 * 1000);
    case "1h":
      return new Date(now + 60 * 60 * 1000);
    case "tonight":
      return atTime(20);
    case "tomorrow":
      return atTime(9, new Date().getHours() < 6 ? 0 : 1);
    case "custom": {
      if (!customTime.value) {
        return null;
      }
      const parsed = new Date(customTime.value);
      return Number.isNaN(parsed.getTime()) ? null : parsed;
    }
    default:
      return null;
  }
};

const reminderSummary = computed(() => {
  if (selectedKey.value === "none") {
    return "不设置提醒";
  }
  const target = resolveReminder();
  return target ? `将在 ${formatDisplay(target)} 提醒` : "请选择提醒时间";
});

const selectOption = (key: ReminderKey) => {
  selectedKey.value = key;
  errorMessage.value = "";
  if (key === "custom" && !customTime.value) {
    customTime.value = formatLocal(new Date(Date.now() + 60 * 60 * 1000));
  }
  if (key !== "custom") {
    void nextTick(() => inputRef.value?.focus());
  }
};

const reset = () => {
  description.value = "";
  selectedKey.value = "none";
  customTime.value = "";
  errorMessage.value = "";
  savedMessage.value = "";
  saving.value = false;
  if (hideTimer !== null) {
    clearTimeout(hideTimer);
    hideTimer = null;
  }
  void nextTick(() => inputRef.value?.focus());
};

const closeWindow = async () => {
  try {
    await getCurrentWindow().hide();
  } catch {
    // 浏览器预览时没有 Tauri 窗口，忽略。
  }
  reset();
};

const submit = async () => {
  const text = description.value.trim();
  if (!text || saving.value) {
    return;
  }
  const target = resolveReminder();
  if (selectedKey.value !== "none" && !target) {
    errorMessage.value = "请选择有效的提醒时间";
    return;
  }
  if (target && target.getTime() <= Date.now()) {
    errorMessage.value = "提醒时间需晚于当前时间";
    return;
  }
  saving.value = true;
  errorMessage.value = "";
  try {
    await api.quickAddTask({
      description: text,
      reminderTime: target ? `${formatLocal(target)}:00` : null
    });
    savedMessage.value = target ? `已添加，${formatDisplay(target)} 提醒` : "已添加";
    description.value = "";
    hideTimer = window.setTimeout(() => {
      void closeWindow();
    }, 700);
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    saving.value = false;
  }
};

const applyTheme = () => {
  const useLight = safeStorage.getItem("appTheme") === "light";
  document.documentElement.classList.toggle("light-theme", useLight);
  document.body.classList.toggle("light-theme", useLight);
};

onMounted(async () => {
  applyTheme();
  storageHandler = event => {
    if (event.key === "appTheme") {
      applyTheme();
    }
  };
  window.addEventListener("storage", storageHandler);
  try {
    unlistenReset = await listen("quick-add-reset", () => {
      applyTheme();
      reset();
    });
  } catch (error) {
    console.error("[quick-add] 监听 quick-add-reset 失败", error);
  }
  reset();
});

onBeforeUnmount(() => {
  unlistenReset?.();
  if (storageHandler) {
    window.removeEventListener("storage", storageHandler);
  }
  if (hideTimer !== null) {
    clearTimeout(hideTimer);
  }
});
</script>

<style scoped>
.quick-add {
  height: 100vh;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 16px 14px;
  border: 1px solid var(--border-strong);
  background: var(--bg-surface);
  font-family: var(--font-sans);
  color: var(--text-base);
}

.quick-add > * {
  flex-shrink: 0;
}

.quick-add-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  cursor: default;
  user-select: none;
}

.quick-add-title {
  color: var(--text-title);
  font-size: var(--font-title);
  font-weight: var(--weight-bold);
}

.quick-add-hint {
  color: var(--text-faint);
  font-size: var(--font-meta);
}

.quick-add-input {
  width: 100%;
  box-sizing: border-box;
  height: 38px;
  font-size: 14px;
}

.quick-add-reminders {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.quick-add-chip {
  height: 26px;
  padding: 0 10px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg-muted);
  color: var(--text-muted);
  font: inherit;
  font-size: var(--font-meta);
  cursor: pointer;
  transition: border-color 0.15s ease, background 0.15s ease, color 0.15s ease;
}

.quick-add-chip:hover {
  border-color: var(--primary);
  color: var(--text-base);
}

.quick-add-chip.is-active {
  border-color: var(--primary);
  background: var(--primary-soft);
  color: var(--primary-text);
}

.quick-add-custom {
  height: 28px;
  flex: 1;
  min-width: 170px;
}

.quick-add-footer {
  margin-top: auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.quick-add-message {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-muted);
  font-size: var(--font-meta);
}

.quick-add-message.is-error {
  color: var(--danger-text);
}

.quick-add-message.is-success {
  color: var(--success-text);
}
</style>
