<template>
  <div class="app" :class="{ 'light-theme': isLightTheme, 'is-linux': isLinuxPlatform }">
    <AppTitlebar
      :app-version="appVersion"
      :is-dev-mode="isDevMode"
      :creating-quick-sticky-note="creatingQuickStickyNote"
      @create-sticky-note="handleCreateStickyNote"
    />

    <div class="main" :style="{ zoom: uiScale, opacity: windowOpacity }">
      <AppSidebar v-model="activeTab" />

      <section class="content">
        <Transition name="fade" mode="out-in">
          <component :is="activeView" :key="activeTab" @navigate="activeTab = $event" />
        </Transition>
      </section>
    </div>

    <SettingsModal :app-version="appVersion" />
    <WebdavModal />
    <DataModal />
    <SearchPalette @navigate="activeTab = $event" />
    <!-- 放在最后：确认框需要叠在设置类弹窗之上。 -->
    <SharedDialogs />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { listen } from "@tauri-apps/api/event";
import AppTitlebar from "./components/AppTitlebar.vue";
import AppSidebar from "./components/AppSidebar.vue";
import SharedDialogs from "./components/SharedDialogs.vue";
import SettingsModal from "./components/SettingsModal.vue";
import WebdavModal from "./components/WebdavModal.vue";
import DataModal from "./components/DataModal.vue";
import SearchPalette from "./components/SearchPalette.vue";
import TodayView from "./views/TodayView.vue";
import CalendarView from "./views/CalendarView.vue";
import TasksView from "./views/TasksView.vue";
import CompletedView from "./views/CompletedView.vue";
import RecurringView from "./views/RecurringView.vue";
import StickiesView from "./views/StickiesView.vue";
import RecordsView from "./views/RecordsView.vue";
import StatsView from "./views/StatsView.vue";
import TrashView from "./views/TrashView.vue";
import { api } from "./api";
import { errorMessage, isLinuxPlatform } from "./format";
import { resolveShortcut, VIEW_SHORTCUT_EVENT, type ShortcutAction } from "./keyboard";
import { closeTopModal, hasOpenModal } from "./modalStack";
import { isTabKey, type TabKey } from "./navigation";
import { safeStorage } from "./safeStorage";
import { useAppData } from "./composables/useAppData";
import { useSettings } from "./composables/useSettings";
import { useUiPrefs } from "./composables/useUiPrefs";
import { useUpdater } from "./composables/useUpdater";
import { useSearchPalette } from "./composables/useSearchPalette";
import type { SyncStatus } from "./types";

const VIEWS: Record<TabKey, Component> = {
  today: TodayView,
  calendar: CalendarView,
  tasks: TasksView,
  completed: CompletedView,
  recurring: RecurringView,
  stickies: StickiesView,
  records: RecordsView,
  stats: StatsView,
  trash: TrashView,
};

const storedTab = safeStorage.getItem("activeTab");
const activeTab = ref<TabKey>(isTabKey(storedTab) ? storedTab : "today");
const activeView = computed(() => VIEWS[activeTab.value]);
watch(activeTab, tab => {
  safeStorage.setItem("activeTab", tab);
});

const appVersion = ref("");
const isDevMode = ref(false);
const creatingQuickStickyNote = ref(false);

const { refreshAll, loadHolidayYears } = useAppData();
const { isLightTheme, uiScale, windowOpacity, emitCurrentUiState } = useUiPrefs();
const { syncStatus, loadSettings, refreshSyncStatus, applyQuickAddShortcut, openSettings, openWebdav } = useSettings();
const { openSearch } = useSearchPalette();
const { handleCheckForUpdates, maybeAutoCheckForUpdates, releaseAvailableUpdateHandle, syncUpdatePreferencesDraft } = useUpdater();

const handleCreateStickyNote = async () => {
  if (creatingQuickStickyNote.value) {
    return;
  }
  creatingQuickStickyNote.value = true;
  try {
    await api.createStickyNote({});
    await refreshAll();
  } catch (error) {
    console.error("[main] 快速新建便签失败", error);
    alert(`新建便签失败：${errorMessage(error)}`);
  } finally {
    creatingQuickStickyNote.value = false;
  }
};

const unlisteners: Array<() => void> = [];

// 各监听互不影响：单个失败只记录日志。
const listenSafely = async <T>(event: string, handler: (payload: T) => void | Promise<void>) => {
  try {
    unlisteners.push(await listen<T>(event, item => handler(item.payload)));
  } catch (error) {
    console.error(`[main] 监听 ${event} 失败`, error);
  }
};

// 主窗口快捷键（见 keyboard.ts）：Esc 先关闭最上层弹窗；其余在有弹窗时不生效。
const focusShortcutTarget = async (kind: "new" | "search") => {
  const selector = `.content [data-shortcut="${kind}"]`;
  let target = document.querySelector<HTMLInputElement>(selector);
  if (!target && activeTab.value !== "tasks") {
    activeTab.value = "tasks";
    // 等视图切换（含过渡动画）完成后再聚焦。
    for (let attempt = 0; attempt < 20 && !target; attempt += 1) {
      await nextTick();
      await new Promise(resolve => setTimeout(resolve, 25));
      target = document.querySelector<HTMLInputElement>(selector);
    }
  }
  target?.focus();
  target?.select?.();
};

const handleShortcut = (event: KeyboardEvent) => {
  const action = resolveShortcut(event);
  if (!action) return;
  if (action.type === "escape") {
    if (closeTopModal()) {
      event.preventDefault();
      return;
    }
  } else if (hasOpenModal()) {
    return;
  }
  switch (action.type) {
    case "tab":
      activeTab.value = action.tab;
      break;
    case "new":
    case "search":
      void focusShortcutTarget(action.type);
      break;
    case "palette":
      openSearch();
      break;
    case "escape":
    case "delete":
      // 交给当前视图（如待办列表退出多选、删除选中项）。
      window.dispatchEvent(new CustomEvent<ShortcutAction>(VIEW_SHORTCUT_EVENT, { detail: action }));
      if (action.type === "escape") return;
      break;
  }
  event.preventDefault();
};

window.addEventListener("keydown", handleShortcut);

onMounted(async () => {
  syncUpdatePreferencesDraft();
  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    appVersion.value = await getVersion();
    isDevMode.value = await api.isDevMode();
  } catch {
    // 浏览器直接访问 http://127.0.0.1:5173/ 时没有 Tauri API，忽略即可。
  }
  try {
    await refreshAll();
    await loadSettings();
    await refreshSyncStatus();
  } catch (error) {
    console.error("[main] 初始化数据失败", error);
  }
  try {
    await emitCurrentUiState();
  } catch (error) {
    console.error("[main] 初始化广播 UI 状态失败", error);
  }
  try {
    await loadHolidayYears();
  } catch (error) {
    console.error("[main] 读取节假日数据范围失败", error);
  }
  // 重新应用一次快捷键，以便在设置中展示启动时注册失败的原因。
  await applyQuickAddShortcut();
  try {
    await maybeAutoCheckForUpdates();
  } catch (error) {
    console.error("[main] 自动检查更新失败", error);
  }
  await listenSafely<SyncStatus>("sync-status", payload => {
    syncStatus.value = payload;
  });
  await listenSafely("data-updated", async () => {
    await refreshAll();
    await loadSettings();
    await refreshSyncStatus();
  });
  await listenSafely("open-sync-settings", () => openWebdav());
  // 节假日数据在线更新后，刷新“节假日待更新”等覆盖范围提示。
  await listenSafely("holidays-updated", () => loadHolidayYears());
  await listenSafely("tray-create-sticky-note", () => handleCreateStickyNote());
  await listenSafely("tray-check-update", async () => {
    await openSettings();
    await handleCheckForUpdates(true);
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleShortcut);
  unlisteners.forEach(unlisten => unlisten());
  void releaseAvailableUpdateHandle();
});
</script>
