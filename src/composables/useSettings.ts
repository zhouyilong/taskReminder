// 应用设置草稿与快速添加快捷键（模块级单例）。
import { reactive, ref } from "vue";
import { api } from "../api";
import { errorMessage } from "../format";
import type { AppSettings } from "../types";
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
  deviceId: "",
  notificationTheme: "app",
  quickAddEnabled: true,
  quickAddShortcut: "CommandOrControl+Alt+N",
  quietHoursEnabled: false,
  quietHoursStart: "22:00",
  quietHoursEnd: "08:00",
  nativeNotificationEnabled: false,
  stickyToggleShortcut: "",
  stickySnapEnabled: true,
  holidayAutoUpdate: true,
  completedRetentionDays: 30,
  autoBackupKeep: 7,
  attentionEffect: "banner",
  attentionUntilHandled: false,
  attentionIntensity: 2,
  attentionColor: "",
  attentionRepeatMinutes: 0,
  notificationScreen: "cursor",
  appMode: "local",
  cloudServerUrl: "",
  cloudUserId: "",
  cloudUsername: "",
  cloudNickname: "",
  cloudUserRole: "",
  cloudMustChangePassword: false,
  lastMergedAccountId: "",
  lastMergedUsername: ""
});
const quickAddShortcutError = ref("");
const settingsOpen = ref(false);
const dataOpen = ref(false);

const loadSettings = async () => {
  const data = await api.getSettings();
  Object.assign(settingsDraft, data);
  useUiPrefs().windowOpacity.value = data.windowOpacity;
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

const openData = () => {
  settingsOpen.value = false;
  dataOpen.value = true;
};

export const useSettings = () => ({
  settingsOpen,
  dataOpen,
  openData,
  openSettings,
  settingsDraft,
  quickAddShortcutError,
  loadSettings,
  applyQuickAddShortcut
});
