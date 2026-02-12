<template>
  <div class="sticky-note-shell">
    <div class="sticky-note-card">
      <div class="sticky-note-header" data-tauri-drag-region>
        <div class="sticky-note-title">便签</div>
        <button class="sticky-note-clear" type="button" @click="clearContent">清空</button>
      </div>
      <textarea
        v-model="content"
        class="sticky-note-editor"
        placeholder="记录今天最重要的事情..."
      />
      <div class="sticky-note-footer">{{ saveHint }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import type { AppSettings } from "./types";

const content = ref("");
const saveHint = ref("自动保存");

let saveTimer: number | null = null;
let unlistenSettings: (() => void) | null = null;
let applyingExternalState = false;
let skipNextSave = false;

const updateFromSettings = (settings: AppSettings) => {
  const next = settings.stickyNoteContent ?? "";
  if (next === content.value) {
    return;
  }
  applyingExternalState = true;
  skipNextSave = true;
  content.value = next;
  applyingExternalState = false;
  saveHint.value = "已同步";
};

const flushSave = async () => {
  try {
    await api.saveStickyNoteContent(content.value);
    saveHint.value = `已保存 ${new Date().toLocaleTimeString("zh-CN", { hour12: false })}`;
  } catch (error) {
    console.error("[sticky-note] 保存失败", error);
    saveHint.value = "保存失败";
  }
};

const scheduleSave = () => {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
  }
  saveHint.value = "正在保存...";
  saveTimer = window.setTimeout(() => {
    void flushSave();
  }, 600);
};

const clearContent = () => {
  content.value = "";
};

watch(content, () => {
  if (skipNextSave) {
    skipNextSave = false;
    return;
  }
  if (applyingExternalState) {
    return;
  }
  scheduleSave();
});

onMounted(async () => {
  try {
    const settings = await api.getSettings();
    updateFromSettings(settings);
  } catch (error) {
    console.error("[sticky-note] 读取设置失败", error);
  }
  try {
    unlistenSettings = await listen<AppSettings>("sticky-note-settings-updated", event => {
      updateFromSettings(event.payload);
    });
  } catch (error) {
    console.error("[sticky-note] 监听 sticky-note-settings-updated 失败", error);
  }
});

onBeforeUnmount(() => {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
  }
  if (unlistenSettings) {
    unlistenSettings();
  }
});
</script>
