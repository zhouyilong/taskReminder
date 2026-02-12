<template>
  <div class="notification-shell" :class="{ 'light-theme': isLightTheme, 'linux-platform': isLinuxPlatform }">
    <div class="notification-window" v-if="visible">
      <div class="notification-header">
        <div class="notification-title">任务提醒</div>
        <button class="notification-close" type="button" @click="handleDismiss">✕</button>
      </div>
      <div class="notification-body">{{ payload?.description }}</div>
      <div class="notification-actions">
        <button class="button secondary" @click="handleComplete">不再提醒</button>
        <button class="button" @click="handleSnooze">稍后提醒</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, TauriEvent } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, type Window as TauriWindow } from "@tauri-apps/api/window";
import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
import { api } from "./api";
import { safeStorage } from "./safeStorage";
import type { NotificationPayload } from "./types";

type NotificationThemeMode = "system" | "app" | "light" | "dark";

const payload = ref<NotificationPayload | null>(null);
const visible = ref(false);
const isLightTheme = ref(false);
const notificationTheme = ref<NotificationThemeMode>("app");
const isLinuxPlatform =
  typeof navigator !== "undefined" && /linux/i.test(navigator.userAgent);
const resolveCurrentWindow = (): TauriWindow | null => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
};
const appWindow = resolveCurrentWindow();
let timer: number | null = null;
let unlistenTheme: (() => void) | null = null;
let unlistenThemeEvent: (() => void) | null = null;
let unlistenDataUpdated: (() => void) | null = null;
let mediaQuery: MediaQueryList | null = null;
let mediaHandler: ((event: MediaQueryListEvent) => void) | null = null;
let themePoll: number | null = null;
let storageHandler: ((event: StorageEvent) => void) | null = null;

const adjustLinuxWindowSize = async () => {
  if (!isLinuxPlatform || !appWindow) {
    return;
  }
  await nextTick();
  const panel = document.querySelector(".notification-window");
  if (!(panel instanceof HTMLElement)) {
    return;
  }
  const width = Math.max(320, Math.ceil(panel.offsetWidth));
  const height = Math.max(128, Math.ceil(panel.offsetHeight));
  try {
    await appWindow.setSize(new LogicalSize(width, height));
    const monitor = await appWindow.currentMonitor();
    const windowSize = await appWindow.outerSize();
    if (monitor) {
      const x = Math.max(0, monitor.size.width - windowSize.width - 20);
      const y = Math.max(0, monitor.size.height - windowSize.height - 40);
      await appWindow.setPosition(new PhysicalPosition(x, y));
    }
  } catch (error) {
    console.error("[notification] 调整 Linux 窗口尺寸失败", error);
  }
};

const startAutoClose = () => {
  if (timer) {
    clearTimeout(timer);
  }
  timer = window.setTimeout(() => {
    handleDismiss();
  }, 180000);
};

const setThemeClass = (useLight: boolean) => {
  isLightTheme.value = useLight;
  document.documentElement.classList.toggle("light-theme", useLight);
  document.body.classList.toggle("light-theme", useLight);
};

const applyTheme = (theme?: string | null) => {
  if (!theme) {
    return;
  }
  setThemeClass(theme === "light");
};

const readSystemTheme = async () => {
  if (!appWindow) {
    if (mediaQuery) {
      setThemeClass(mediaQuery.matches);
    }
    return;
  }
  try {
    const theme = await appWindow.theme();
    if (theme) {
      applyTheme(theme);
      return;
    }
  } catch {
    // 忽略主题读取失败
  }
  if (mediaQuery) {
    setThemeClass(mediaQuery.matches);
  }
};

const syncThemeFromBackend = async () => {
  try {
    const theme = await invoke<string>("get_current_theme");
    applyTheme(theme);
  } catch {
    // 忽略主题同步失败
  }
};

const loadNotificationTheme = async () => {
  try {
    const settings = await api.getSettings();
    notificationTheme.value = settings.notificationTheme ?? "app";
  } catch {
    // 忽略设置读取失败
  }
};

const applyAppTheme = () => {
  const stored = safeStorage.getItem("appTheme");
  if (stored === "light") {
    setThemeClass(true);
    return true;
  }
  if (stored === "dark") {
    setThemeClass(false);
    return true;
  }
  return false;
};

const applyThemeByMode = async () => {
  if (notificationTheme.value === "light") {
    stopThemePolling();
    setThemeClass(true);
    return;
  }
  if (notificationTheme.value === "dark") {
    stopThemePolling();
    setThemeClass(false);
    return;
  }
  if (notificationTheme.value === "app") {
    stopThemePolling();
    if (!applyAppTheme()) {
      await syncThemeFromBackend();
      await readSystemTheme();
    }
    return;
  }
  await syncThemeFromBackend();
  await readSystemTheme();
  if (visible.value) {
    startThemePolling();
  }
};

const setupThemeListeners = async () => {
  mediaQuery = window.matchMedia?.("(prefers-color-scheme: light)") ?? null;
  if (mediaQuery) {
    mediaHandler = event => {
      if (notificationTheme.value === "system") {
        setThemeClass(event.matches);
      }
    };
    if (typeof mediaQuery.addEventListener === "function") {
      mediaQuery.addEventListener("change", mediaHandler);
    } else {
      mediaQuery.addListener?.(mediaHandler);
    }
  }

  if (appWindow) {
    try {
      unlistenTheme = await appWindow.onThemeChanged(theme => {
        if (notificationTheme.value === "system") {
          applyTheme(theme);
        }
      });
    } catch {
      // 忽略主题监听失败
    }
  }

  try {
    unlistenThemeEvent = await listen<string>(TauriEvent.WINDOW_THEME_CHANGED, event => {
      if (notificationTheme.value === "system") {
        applyTheme(event.payload);
      }
    });
  } catch {
    // 忽略事件监听失败
  }
};

const startThemePolling = () => {
  if (themePoll !== null || notificationTheme.value !== "system") {
    return;
  }
  themePoll = window.setInterval(() => {
    void syncThemeFromBackend();
  }, 2000);
};

const stopThemePolling = () => {
  if (themePoll !== null) {
    clearInterval(themePoll);
    themePoll = null;
  }
};

const show = async (data: NotificationPayload) => {
  payload.value = data;
  visible.value = true;
  await loadNotificationTheme();
  await applyThemeByMode();
  if (appWindow) {
    await appWindow.show();
  }
  await adjustLinuxWindowSize();
  startAutoClose();
};

const hide = async () => {
  visible.value = false;
  stopThemePolling();
  if (appWindow) {
    await appWindow.hide();
  }
};

const handleDismiss = async () => {
  if (!payload.value) {
    return;
  }
  await api.acknowledgeNotification({
    recordId: payload.value.recordId,
    action: "DISMISSED"
  });
  await hide();
};

const handleComplete = async () => {
  if (!payload.value) {
    return;
  }
  await api.acknowledgeNotification({
    recordId: payload.value.recordId,
    action: "COMPLETED"
  });
  await hide();
};

const handleSnooze = async () => {
  if (!payload.value) {
    return;
  }
  await api.snoozeNotification({
    recordId: payload.value.recordId,
    reminderId: payload.value.reminderId,
    reminderType: payload.value.reminderType,
    minutes: payload.value.snoozeMinutes
  });
  await hide();
};

onMounted(async () => {
  await setupThemeListeners();
  await loadNotificationTheme();
  await applyThemeByMode();
  try {
    const snapshot = await api.getNotificationSnapshot();
    if (snapshot) {
      await show(snapshot);
    }
  } catch (error) {
    console.error("[notification] 读取通知快照失败", error);
  }
  try {
    await listen<NotificationPayload>("notification", async event => {
      await show(event.payload);
    });
  } catch (error) {
    console.error("[notification] 监听 notification 失败", error);
  }
  try {
    unlistenDataUpdated = await listen("data-updated", async () => {
      await loadNotificationTheme();
      await applyThemeByMode();
    });
  } catch (error) {
    console.error("[notification] 监听 data-updated 失败", error);
  }
  storageHandler = event => {
    if (event.key === "appTheme" && notificationTheme.value === "app") {
      void applyThemeByMode();
    }
  };
  window.addEventListener("storage", storageHandler);
});

onBeforeUnmount(() => {
  if (unlistenTheme) {
    unlistenTheme();
  }
  if (unlistenThemeEvent) {
    unlistenThemeEvent();
  }
  if (unlistenDataUpdated) {
    unlistenDataUpdated();
  }
  if (mediaQuery && mediaHandler) {
    if (typeof mediaQuery.removeEventListener === "function") {
      mediaQuery.removeEventListener("change", mediaHandler);
    } else {
      mediaQuery.removeListener?.(mediaHandler);
    }
  }
  if (storageHandler) {
    window.removeEventListener("storage", storageHandler);
  }
  stopThemePolling();
});
</script>
