// 自动更新状态与操作（模块级单例）。Update 句柄含 JS 私有字段，必须用 shallowRef 存储。
import { computed, reactive, ref, shallowRef } from "vue";
import type { DownloadEvent, Update } from "@tauri-apps/plugin-updater";
import { errorMessage, formatBytes, formatDateTime } from "../format";
import {
  checkForUpdates,
  formatVersionLabel,
  installUpdate,
  loadUpdatePreferences,
  resolveUpdateNetworkOptions,
  saveUpdatePreferences,
  summarizeUpdate,
  type UpdatePreferences,
  type UpdateSummary
} from "../update";

const initialUpdatePreferences = loadUpdatePreferences();
const updatePreferences = reactive<UpdatePreferences>({ ...initialUpdatePreferences });
const updatePreferencesDraft = reactive<UpdatePreferences>({ ...initialUpdatePreferences });
const availableUpdate = ref<UpdateSummary | null>(null);
const availableUpdateHandle = shallowRef<Update | null>(null);
const updateChecking = ref(false);
const updateInstalling = ref(false);
const updateCheckError = ref("");
const updateDownloadedBytes = ref(0);
const updateContentLength = ref<number | null>(null);

const isUpdateIgnored = computed(() => {
  return !!availableUpdate.value && updatePreferences.ignoredVersion === availableUpdate.value.version;
});
const updateTagLabel = computed(() => {
  if (!availableUpdate.value || isUpdateIgnored.value) {
    return "";
  }
  return `新版本 ${formatVersionLabel(availableUpdate.value.version)}`;
});
const updateNotesText = computed(() => {
  const body = availableUpdate.value?.body?.trim();
  return body || "本次版本未提供更新说明。";
});
const updateProgressPercent = computed(() => {
  if (!updateContentLength.value || updateContentLength.value <= 0) {
    return null;
  }
  return Math.min(100, Math.round((updateDownloadedBytes.value / updateContentLength.value) * 100));
});
const updateStatusText = computed(() => {
  if (updateInstalling.value) {
    return updateProgressPercent.value !== null
      ? `正在下载更新 ${updateProgressPercent.value}%`
      : "正在准备安装更新";
  }
  if (updateChecking.value) {
    return "正在检查更新";
  }
  if (availableUpdate.value && !isUpdateIgnored.value) {
    return `发现新版本 ${formatVersionLabel(availableUpdate.value.version)}`;
  }
  if (availableUpdate.value && isUpdateIgnored.value) {
    return `已忽略 ${formatVersionLabel(availableUpdate.value.version)}`;
  }
  if (updateCheckError.value) {
    return updateCheckError.value;
  }
  if (updatePreferences.lastCheckAt) {
    return `上次检查 ${formatDateTime(updatePreferences.lastCheckAt)}`;
  }
  return "尚未检查更新";
});
const updatePrimaryActionLabel = computed(() => {
  if (updateInstalling.value) {
    return updateProgressPercent.value !== null ? `下载中 ${updateProgressPercent.value}%` : "准备安装...";
  }
  return "立即更新";
});
const updateProgressText = computed(() => {
  if (!updateInstalling.value) {
    return "";
  }
  if (updateContentLength.value && updateContentLength.value > 0) {
    return `${formatBytes(updateDownloadedBytes.value)} / ${formatBytes(updateContentLength.value)}`;
  }
  if (updateDownloadedBytes.value > 0) {
    return `已下载 ${formatBytes(updateDownloadedBytes.value)}`;
  }
  return "正在准备安装包...";
});

const persistUpdatePreferencesState = () => {
  saveUpdatePreferences({
    autoCheckEnabled: updatePreferences.autoCheckEnabled,
    ignoredVersion: updatePreferences.ignoredVersion,
    lastCheckAt: updatePreferences.lastCheckAt,
    proxyUrl: updatePreferences.proxyUrl,
  });
};

const syncUpdatePreferencesDraft = () => {
  Object.assign(updatePreferencesDraft, updatePreferences);
};

const resetUpdateProgress = () => {
  updateDownloadedBytes.value = 0;
  updateContentLength.value = null;
};

const releaseAvailableUpdateHandle = async () => {
  if (!availableUpdateHandle.value) {
    return;
  }
  try {
    await availableUpdateHandle.value.close();
  } catch (error) {
    console.warn("[update] 释放更新句柄失败", error);
  } finally {
    availableUpdateHandle.value = null;
  }
};

const handleUpdateDownloadEvent = (event: DownloadEvent) => {
  switch (event.event) {
    case "Started":
      updateContentLength.value = event.data.contentLength ?? null;
      updateDownloadedBytes.value = 0;
      break;
    case "Progress":
      updateDownloadedBytes.value += event.data.chunkLength;
      break;
    case "Finished":
      if (updateContentLength.value) {
        updateDownloadedBytes.value = updateContentLength.value;
      }
      break;
  }
};

const handleCheckForUpdates = async (manual = false) => {
  if (updateChecking.value || updateInstalling.value) {
    return;
  }
  updateChecking.value = true;
  updateCheckError.value = "";
  resetUpdateProgress();
  try {
    await releaseAvailableUpdateHandle();
    const update = await checkForUpdates(resolveUpdateNetworkOptions(updatePreferences.proxyUrl));
    updatePreferences.lastCheckAt = new Date().toISOString();
    persistUpdatePreferencesState();
    if (!update) {
      availableUpdate.value = null;
      if (manual) {
        alert("当前已经是最新版本。");
      }
      return;
    }

    if (updatePreferences.ignoredVersion && updatePreferences.ignoredVersion !== update.version) {
      updatePreferences.ignoredVersion = null;
      persistUpdatePreferencesState();
    }

    availableUpdateHandle.value = update;
    availableUpdate.value = summarizeUpdate(update);
  } catch (error) {
    updateCheckError.value = `检查更新失败：${errorMessage(error)}`;
    if (manual) {
      alert(updateCheckError.value);
    }
  } finally {
    updateChecking.value = false;
  }
};

const ignoreAvailableUpdate = async () => {
  if (!availableUpdate.value) {
    return;
  }
  updatePreferences.ignoredVersion = availableUpdate.value.version;
  persistUpdatePreferencesState();
  await releaseAvailableUpdateHandle();
};

const restoreIgnoredUpdate = () => {
  updatePreferences.ignoredVersion = null;
  persistUpdatePreferencesState();
};

const handleInstallUpdate = async () => {
  if (updateChecking.value || updateInstalling.value) {
    return;
  }
  updateCheckError.value = "";
  if (!availableUpdateHandle.value) {
    await handleCheckForUpdates(true);
    if (!availableUpdateHandle.value) {
      return;
    }
  }

  updateInstalling.value = true;
  resetUpdateProgress();
  try {
    // 代理已在检查更新时写入 Update 句柄，下载安装会沿用同一代理。
    await installUpdate(availableUpdateHandle.value, handleUpdateDownloadEvent);
  } catch (error) {
    updateCheckError.value = `安装更新失败：${errorMessage(error)}`;
    alert(updateCheckError.value);
  } finally {
    updateInstalling.value = false;
    await releaseAvailableUpdateHandle();
  }
};

const maybeAutoCheckForUpdates = async () => {
  if (!updatePreferences.autoCheckEnabled) {
    return;
  }
  await handleCheckForUpdates(false);
};

export const useUpdater = () => ({
  updatePreferences,
  updatePreferencesDraft,
  availableUpdate,
  updateChecking,
  updateInstalling,
  updateCheckError,
  isUpdateIgnored,
  updateTagLabel,
  updateNotesText,
  updateProgressPercent,
  updateStatusText,
  updatePrimaryActionLabel,
  updateProgressText,
  persistUpdatePreferencesState,
  syncUpdatePreferencesDraft,
  releaseAvailableUpdateHandle,
  handleCheckForUpdates,
  ignoreAvailableUpdate,
  restoreIgnoredUpdate,
  handleInstallUpdate,
  maybeAutoCheckForUpdates,
});
