<template>
  <div class="notification-shell" :class="{ 'light-theme': isLightTheme, 'linux-platform': isLinuxPlatform }">
    <div class="notification-window" v-if="visible">
      <div class="notification-header">
        <div class="notification-heading">
          <div class="notification-eyebrow">
            <span class="notification-pulse-dot" aria-hidden="true"></span>
            <span class="notification-status">{{ missedLabel ? "错过的提醒" : "提醒进行中" }}</span>
            <span v-if="queue.length > 1" class="notification-queue-count">{{ queuePositionLabel }}</span>
          </div>
          <div class="notification-title">
            {{ reminderTitle }}
            <span v-if="missedLabel" class="notification-missed">{{ missedLabel }}</span>
          </div>
        </div>
        <button class="notification-close" type="button" @click="handleDismiss">✕</button>
      </div>
      <div class="notification-body">{{ notificationDescription }}</div>
      <div class="notification-meta">
        <div class="notification-meta-item">
          <span class="notification-meta-label">已停留</span>
          <strong class="notification-meta-value">{{ elapsedLabel }}</strong>
        </div>
        <div class="notification-meta-item">
          <span class="notification-meta-label">剩余</span>
          <strong class="notification-meta-value">{{ remainingLabel }}</strong>
        </div>
      </div>
      <div class="notification-progress" aria-hidden="true">
        <div class="notification-progress-bar" :style="{ width: `${progressPercent}%` }"></div>
      </div>
      <div class="notification-actions">
        <button
          v-if="queue.length > 1"
          class="button secondary notification-dismiss-all"
          @click="handleDismissAll"
        >
          全部知道了
        </button>
        <button class="button secondary" @click="handleAcknowledge">知道了</button>
        <button class="button" @click="handleSnooze">稍后提醒</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, TauriEvent } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, type Window as TauriWindow } from "@tauri-apps/api/window";
import { api } from "./api";
import { markdownToPreviewText, stripLeadingListMarker } from "./markdown";
import { safeStorage } from "./safeStorage";
import type { NotificationPayload } from "./types";

type NotificationThemeMode = "system" | "app" | "light" | "dark";

const AUTO_CLOSE_MS = 15 * 60 * 1000;
// 弹出时间比原定时间晚超过该值，视为“错过的提醒”（如关机期间到点、启动后补发）。
const MISSED_THRESHOLD_MS = 2 * 60 * 1000;
// 待处理的提醒队列，由后端维护；弹窗始终展示队首一条。
const queue = ref<NotificationPayload[]>([]);
const payload = computed<NotificationPayload | null>(() => queue.value[0] ?? null);
const visible = ref(false);
const isLightTheme = ref(false);
const notificationTheme = ref<NotificationThemeMode>("app");
const shownAt = ref<number | null>(null);
const nowTick = ref(Date.now());
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
let elapsedTicker: number | null = null;
let unlistenTheme: (() => void) | null = null;
let unlistenThemeEvent: (() => void) | null = null;
let unlistenDataUpdated: (() => void) | null = null;
let unlistenQueue: (() => void) | null = null;
let mediaQuery: MediaQueryList | null = null;
let mediaHandler: ((event: MediaQueryListEvent) => void) | null = null;
let themePoll: number | null = null;
let storageHandler: ((event: StorageEvent) => void) | null = null;

const formatDuration = (totalMs: number) => {
  const totalSeconds = Math.max(0, Math.floor(totalMs / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
  }

  return `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
};

const elapsedMs = computed(() => {
  if (!shownAt.value) {
    return 0;
  }
  return Math.max(0, nowTick.value - shownAt.value);
});

const remainingMs = computed(() => Math.max(0, AUTO_CLOSE_MS - elapsedMs.value));
const elapsedLabel = computed(() => formatDuration(elapsedMs.value));
const remainingLabel = computed(() => formatDuration(remainingMs.value));
const notificationDescription = computed(() => {
  const text = stripLeadingListMarker(markdownToPreviewText(payload.value?.description, ""));
  return text || "-";
});
const queuePositionLabel = computed(() => `1 / ${queue.value.length}`);
const reminderTitle = computed(() =>
  payload.value?.reminderType === "RECURRING" ? "循环提醒" : "任务提醒"
);

const parseLocalDateTime = (value?: string | null): Date | null => {
  if (!value) {
    return null;
  }
  // 后端时间为不带时区的本地时间（YYYY-MM-DDTHH:mm[:ss]），按本地时间解析。
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
};

const formatScheduledTime = (date: Date) => {
  const now = new Date();
  const time = `${date.getHours().toString().padStart(2, "0")}:${date
    .getMinutes()
    .toString()
    .padStart(2, "0")}`;
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();
  return sameDay ? time : `${date.getMonth() + 1}月${date.getDate()}日 ${time}`;
};

const missedLabel = computed(() => {
  const scheduled = parseLocalDateTime(payload.value?.scheduledTime);
  if (!scheduled || !shownAt.value) {
    return "";
  }
  if (shownAt.value - scheduled.getTime() < MISSED_THRESHOLD_MS) {
    return "";
  }
  return `原定 ${formatScheduledTime(scheduled)}`;
});

const progressPercent = computed(() => {
  if (!shownAt.value) {
    return 0;
  }
  return Math.min(100, (elapsedMs.value / AUTO_CLOSE_MS) * 100);
});

const stopAutoClose = () => {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
};

const stopElapsedTicker = () => {
  if (elapsedTicker !== null) {
    clearInterval(elapsedTicker);
    elapsedTicker = null;
  }
};

const startElapsedTicker = () => {
  stopElapsedTicker();
  nowTick.value = Date.now();
  elapsedTicker = window.setInterval(() => {
    nowTick.value = Date.now();
  }, 1000);
};

const startAutoClose = () => {
  stopAutoClose();
  timer = window.setTimeout(() => {
    void handleDismiss();
  }, AUTO_CLOSE_MS);
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
      unlistenTheme = await appWindow.onThemeChanged(({ payload: theme }) => {
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

const show = async () => {
  shownAt.value = Date.now();
  nowTick.value = shownAt.value;
  visible.value = true;
  await loadNotificationTheme();
  await applyThemeByMode();
  if (appWindow) {
    await appWindow.show();
  }
  startElapsedTicker();
  startAutoClose();
};

const hide = async () => {
  visible.value = false;
  shownAt.value = null;
  stopAutoClose();
  stopElapsedTicker();
  stopThemePolling();
  if (appWindow) {
    await appWindow.hide();
  }
};

// 应用后端下发的最新队列：队列为空则隐藏；队首变化时重新计时并展示新的一条。
const applyQueue = async (items: NotificationPayload[] | null | undefined) => {
  const previousHead = payload.value?.recordId ?? null;
  queue.value = Array.isArray(items) ? items : [];
  if (!payload.value) {
    await hide();
    return;
  }
  if (!visible.value || payload.value.recordId !== previousHead) {
    await show();
  }
};

const runAction = async (action: () => Promise<NotificationPayload[] | void>) => {
  try {
    const remaining = await action();
    if (Array.isArray(remaining)) {
      await applyQueue(remaining);
      return;
    }
    await applyQueue(await api.getNotificationQueue());
  } catch (error) {
    console.error("[notification] 处理提醒失败", error);
  }
};

const handleDismiss = async () => {
  const current = payload.value;
  if (!current) {
    return;
  }
  await runAction(() =>
    api.acknowledgeNotification({
      recordId: current.recordId,
      action: "DISMISSED"
    })
  );
};

const handleAcknowledge = handleDismiss;

const handleDismissAll = async () => {
  await runAction(async () => {
    await api.acknowledgeAllNotifications();
    return [];
  });
};

const handleSnooze = async () => {
  const current = payload.value;
  if (!current) {
    return;
  }
  await runAction(() =>
    api.snoozeNotification({
      recordId: current.recordId,
      reminderId: current.reminderId,
      reminderType: current.reminderType,
      minutes: current.snoozeMinutes
    })
  );
};

onMounted(async () => {
  await setupThemeListeners();
  await loadNotificationTheme();
  await applyThemeByMode();
  try {
    await applyQueue(await api.getNotificationQueue());
  } catch (error) {
    console.error("[notification] 读取提醒队列失败", error);
  }
  try {
    unlistenQueue = await listen<NotificationPayload[]>("notification-queue", async event => {
      await applyQueue(event.payload);
    });
  } catch (error) {
    console.error("[notification] 监听 notification-queue 失败", error);
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
  if (unlistenQueue) {
    unlistenQueue();
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
  stopAutoClose();
  stopElapsedTicker();
  stopThemePolling();
});
</script>
