// 应用设置草稿、同步状态与快速添加快捷键（模块级单例）。
import { computed, reactive, ref } from "vue";
import { api } from "../api";
import { errorMessage, formatDateTime } from "../format";
import { syncStateLabel, syncStateTone } from "../syncStatus";
import type { AppSettings, SyncStatus } from "../types";
import { useUiPrefs } from "./useUiPrefs";
import { useUpdater } from "./useUpdater";

const settingsDraft = reactive<AppSettings>({
  autoStartEnabled: false,
  soundEnabled: true,
  snoozeMinutes: 5,
  stickyNoteEnabled: false,
  stickyNoteContent: "",
  stickyNoteWidth: 360,
  stickyNoteHeight: 520,
  stickyNoteX: null,
  stickyNoteY: null,
  stickyNoteOpacity: 0.95,
  windowOpacity: 1.0,
  webdavEnabled: false,
  webdavUrl: "",
  webdavUsername: "",
  webdavPassword: "",
  webdavRootPath: "",
  webdavSyncIntervalMinutes: 60,
  webdavDeviceId: "",
  notificationTheme: "app",
  quickAddEnabled: true,
  quickAddShortcut: "CommandOrControl+Alt+N",
  syncEncryptionEnabled: false,
  syncPassphrase: ""
});
const syncStatus = ref<SyncStatus | null>(null);
const quickAddShortcutError = ref("");
const settingsOpen = ref(false);
const webdavOpen = ref(false);
const dataOpen = ref(false);

const syncStatusLabel = computed(() => syncStateLabel(syncStatus.value?.status));

const syncStatusTone = computed(() =>
  syncStateTone(syncStatus.value?.status, syncStatus.value?.error)
);

const syncStatusTitle = computed(() => {
  const parts = [`云同步：${syncStatusLabel.value}`];
  if (syncStatus.value?.time) {
    parts.push(`时间：${formatDateTime(syncStatus.value.time)}`);
  }
  if (syncStatus.value?.error) {
    parts.push(`错误：${syncStatus.value.error}`);
  }
  return parts.join("\n");
});

const loadSettings = async () => {
  const data = await api.getSettings();
  Object.assign(settingsDraft, data);
  useUiPrefs().windowOpacity.value = data.windowOpacity;
};

const refreshSyncStatus = async () => {
  syncStatus.value = await api.getSyncStatus();
};

// 按已保存的设置重新注册快速添加快捷键，并记录失败原因（如被其他程序占用）。
const applyQuickAddShortcut = async () => {
  try {
    await api.applyQuickAddShortcut();
    quickAddShortcutError.value = "";
    return true;
  } catch (error) {
    quickAddShortcutError.value = errorMessage(error);
    return false;
  }
};

const openSettings = async () => {
  await loadSettings();
  useUpdater().syncUpdatePreferencesDraft();
  settingsOpen.value = true;
};

const openWebdav = async () => {
  await loadSettings();
  webdavOpen.value = true;
};

const openData = () => {
  settingsOpen.value = false;
  dataOpen.value = true;
};

export const useSettings = () => ({
  settingsOpen,
  webdavOpen,
  dataOpen,
  openData,
  openSettings,
  openWebdav,
  settingsDraft,
  syncStatus,
  syncStatusLabel,
  syncStatusTone,
  syncStatusTitle,
  quickAddShortcutError,
  loadSettings,
  refreshSyncStatus,
  applyQuickAddShortcut,
});
