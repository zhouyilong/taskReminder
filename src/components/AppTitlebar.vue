<template>
  <div class="titlebar" @dblclick="toggleMaximize">
    <div class="titlebar-left" data-tauri-drag-region>
      <span class="app-logo" aria-hidden="true">
        <svg viewBox="0 0 24 24">
          <path d="M12 2.27C7.91 2.27 5.06 5.24 5.06 9.46v3.72c0 1.11-.62 1.98-1.49 2.73-.75.62-.37 1.98.74 1.98h15.38c1.11 0 1.49-1.36.74-1.98-.87-.75-1.49-1.62-1.49-2.73V9.46c0-4.22-2.85-7.19-6.94-7.19z" fill="currentColor" />
          <path d="M9.52 19.13a2.48 2.48 0 0 0 4.96 0z" fill="currentColor" />
          <path class="app-logo-check" d="M8.53 10.33l2.48 2.48 4.59-4.59" fill="none" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </span>
      <span class="app-title">任务提醒<span v-if="isDevMode" class="dev-tag"> [开发]</span></span>
      <span v-if="appVersion" class="app-version">v{{ appVersion }}</span>
      <span class="tag sync-tag" :class="`is-${syncStatusTone}`" :title="syncStatusTitle">
        <span class="sync-dot" aria-hidden="true"></span>{{ syncStatusLabel }}
      </span>
      <button
        v-if="updateTagLabel"
        class="tag tag-button update-tag"
        type="button"
        :title="updateTagLabel"
        @click="openSettings"
      >
        {{ updateTagLabel }}
      </button>
    </div>
    <div class="titlebar-actions">
      <button class="icon-button theme-toggle" type="button" title="切换主题" @click="toggleTheme">
        <transition name="theme" mode="out-in">
          <svg v-if="isLightTheme" key="sun" viewBox="0 0 24 24" aria-hidden="true" class="theme-icon theme-icon-sun">
            <circle cx="12" cy="12" r="4.2" />
            <path d="M12 2.75V5.1" />
            <path d="M12 18.9V21.25" />
            <path d="M2.75 12H5.1" />
            <path d="M18.9 12H21.25" />
            <path d="M5.45 5.45L7.1 7.1" />
            <path d="M16.9 16.9L18.55 18.55" />
            <path d="M16.9 7.1L18.55 5.45" />
            <path d="M5.45 18.55L7.1 16.9" />
          </svg>
          <svg v-else key="moon" viewBox="0 0 24 24" aria-hidden="true" class="theme-icon">
            <path
              d="M20 15.2a8.2 8.2 0 0 1-10.2-10 9 9 0 1 0 10.2 10z"
            />
          </svg>
        </transition>
      </button>
      <button
        class="icon-button"
        type="button"
        :title="creatingQuickStickyNote ? '新建便签处理中...' : '新建便签'"
        :disabled="creatingQuickStickyNote"
        @click="handleCreateStickyNote"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true" class="stroke-icon">
          <path d="M15.5 20H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v9.5L15.5 20z" />
          <path d="M15 20v-3.5a1 1 0 0 1 1-1h4" />
          <path d="M8.5 9h7M8.5 12.5h4" />
        </svg>
      </button>
      <button class="icon-button" type="button" title="设置" @click="openSettings">
        <svg viewBox="0 0 24 24" aria-hidden="true" class="stroke-icon">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1.08-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1.08 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      </button>
      <button class="icon-button" type="button" title="云同步" @click="openWebdav">
        <svg viewBox="0 0 24 24" aria-hidden="true" class="stroke-icon">
          <path d="M7.5 18.5a4.5 4.5 0 0 1-.4-8.98 5.5 5.5 0 0 1 10.66 1.2A3.9 3.9 0 0 1 17 18.5H7.5z" />
          <path d="M12 10.5v5M9.8 13.4l2.2 2.2 2.2-2.2" />
        </svg>
      </button>
    </div>
    <span class="titlebar-divider" aria-hidden="true"></span>
    <div class="titlebar-controls">
      <button class="titlebar-button" type="button" title="最小化" @click="handleMinimize">
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <rect x="1" y="5" width="8" height="1.5" />
        </svg>
      </button>
      <button class="titlebar-button" type="button" :title="isWindowMaximized ? '还原' : '最大化'" @click="handleMaximize">
        <svg v-if="!isWindowMaximized" viewBox="0 0 10 10" aria-hidden="true">
          <rect x="2" y="2" width="6" height="6" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
        <svg v-else viewBox="0 0 10 10" aria-hidden="true">
          <rect x="1.5" y="3" width="5.5" height="5.5" fill="none" stroke="currentColor" stroke-width="1" />
          <rect x="3" y="1.5" width="5.5" height="5.5" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button class="titlebar-button close" type="button" title="关闭" @click="handleClose">
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2 2 L8 8 M8 2 L2 8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
        </svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getCurrentWindow, type Window as TauriWindow } from "@tauri-apps/api/window";
import { useSettings } from "../composables/useSettings";
import { useUiPrefs } from "../composables/useUiPrefs";
import { useUpdater } from "../composables/useUpdater";

defineProps<{
  appVersion: string;
  isDevMode: boolean;
  creatingQuickStickyNote: boolean;
}>();

const emit = defineEmits<{ (e: "create-sticky-note"): void }>();

const { syncStatusLabel, syncStatusTone, syncStatusTitle, openSettings, openWebdav } = useSettings();
const { isLightTheme, toggleTheme } = useUiPrefs();
const { updateTagLabel } = useUpdater();

const isWindowMaximized = ref(false);

const getAppWindow = (): TauriWindow | null => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
};

const handleCreateStickyNote = () => emit("create-sticky-note");

const handleMinimize = async () => {
  await getAppWindow()?.minimize();
};

const handleMaximize = async () => {
  const appWindow = getAppWindow();
  if (!appWindow) {
    return;
  }
  const isMax = await appWindow.isMaximized();
  isWindowMaximized.value = !isMax;
  if (isMax) {
    await appWindow.unmaximize();
  } else {
    await appWindow.maximize();
  }
};

const toggleMaximize = async () => {
  await handleMaximize();
};

const handleClose = async () => {
  // 默认“关闭”改为最小化到托盘：隐藏主窗口，保留后台运行（托盘可重新打开/退出）。
  await getAppWindow()?.hide();
};

onMounted(async () => {
  const appWindow = getAppWindow();
  if (!appWindow) {
    return;
  }
  try {
    isWindowMaximized.value = await appWindow.isMaximized();
    await appWindow.onResized(async () => {
      isWindowMaximized.value = await appWindow.isMaximized();
    });
  } catch (error) {
    console.error("[main] 初始化窗口状态失败", error);
  }
});
</script>
