<template>
  <Modal :open="settingsOpen" title="应用设置" @close="settingsOpen = false" @confirm="saveSettings">
    <div class="modal-section">
      <div class="form-row compact">
        <label>
          <input type="checkbox" v-model="settingsDraft.autoStartEnabled" /> 开机自启
        </label>
        <label>
          <input type="checkbox" v-model="settingsDraft.soundEnabled" /> 提示音
        </label>
        <label title="全屏程序或游戏中也能看到提醒；勿扰时段内不发送">
          <input type="checkbox" v-model="settingsDraft.nativeNotificationEnabled" /> 同时发送系统通知
        </label>
      </div>
      <div class="form-row compact">
        <label>稍后提醒分钟数</label>
        <input class="input" type="number" min="1" v-model.number="settingsDraft.snoozeMinutes" />
      </div>
      <div class="form-row compact">
        <label>
          <input type="checkbox" v-model="settingsDraft.quietHoursEnabled" /> 勿扰时段
        </label>
        <input
          class="input"
          type="time"
          v-model="settingsDraft.quietHoursStart"
          :disabled="!settingsDraft.quietHoursEnabled"
          style="width: 130px"
        />
        <span class="field-hint">至</span>
        <input
          class="input"
          type="time"
          v-model="settingsDraft.quietHoursEnd"
          :disabled="!settingsDraft.quietHoursEnabled"
          style="width: 130px"
        />
      </div>
      <div v-if="settingsDraft.quietHoursEnabled" class="form-row compact">
        <span class="field-hint">{{ quietHoursHint }}</span>
      </div>
      <div class="form-row compact">
        <label>
          <input type="checkbox" v-model="settingsDraft.quickAddEnabled" /> 快速添加快捷键
        </label>
        <input
          class="input shortcut-input"
          :class="{ 'is-recording': shortcutRecording === 'quickAdd' }"
          readonly
          :disabled="!settingsDraft.quickAddEnabled"
          :value="shortcutRecording === 'quickAdd' ? '请按下组合键…' : formatAccelerator(settingsDraft.quickAddShortcut)"
          title="点击后按下新的组合键，Esc 取消"
          @focus="shortcutRecording = 'quickAdd'"
          @blur="shortcutRecording = null"
          @keydown.prevent="handleShortcutKeydown($event, 'quickAdd')"
        />
      </div>
      <div class="form-row compact">
        <label>显示/隐藏全部便签</label>
        <input
          class="input shortcut-input"
          :class="{ 'is-recording': shortcutRecording === 'stickyToggle' }"
          readonly
          :value="
            shortcutRecording === 'stickyToggle'
              ? '请按下组合键…'
              : settingsDraft.stickyToggleShortcut
                ? formatAccelerator(settingsDraft.stickyToggleShortcut)
                : '未设置'
          "
          title="点击后按下新的组合键，Esc 取消"
          @focus="shortcutRecording = 'stickyToggle'"
          @blur="shortcutRecording = null"
          @keydown.prevent="handleShortcutKeydown($event, 'stickyToggle')"
        />
        <button
          v-if="settingsDraft.stickyToggleShortcut"
          class="button secondary"
          type="button"
          @click="settingsDraft.stickyToggleShortcut = ''"
        >
          清除
        </button>
        <span class="field-hint">托盘菜单也可以显示或隐藏全部便签</span>
      </div>
      <div class="form-row compact">
        <label>
          <input type="checkbox" v-model="settingsDraft.stickySnapEnabled" /> 便签贴边吸附
        </label>
        <span class="field-hint">{{ stickySnapHint }}</span>
      </div>
      <div v-if="quickAddShortcutError" class="form-row compact">
        <span class="field-hint is-error">{{ quickAddShortcutError }}</span>
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact">
        <label>界面缩放</label>
        <input class="settings-range" type="range" min="0.8" max="1.2" step="0.05" v-model.number="uiScale" style="flex: 1" />
        <span class="tag">{{ uiScalePercent }}%</span>
      </div>
      <div class="form-row compact">
        <label>整体透明度</label>
        <input class="settings-range" type="range" min="0.3" max="1" step="0.05" v-model.number="windowOpacity" style="flex: 1" />
        <span class="tag">{{ windowOpacityPercent }}%</span>
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact">
        <label>数据</label>
        <button class="button secondary" type="button" @click="openData">导入导出与备份…</button>
        <span class="field-hint">JSON 备份、Markdown、日历（ICS），每天自动备份</span>
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact">
        <label>提醒弹窗主题</label>
        <select class="select" v-model="settingsDraft.notificationTheme">
          <option value="system">跟随系统</option>
          <option value="app">跟随应用</option>
          <option value="light">浅色</option>
          <option value="dark">深色</option>
        </select>
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact" style="gap: 8px;">
        <label>
          <input type="checkbox" v-model="updatePreferencesDraft.autoCheckEnabled" /> 启动时自动检查更新
        </label>
        <button
          class="button secondary"
          type="button"
          :disabled="updateChecking || updateInstalling"
          @click="handleCheckForUpdates(true)"
        >
          {{ updateChecking ? "检查中..." : "检查更新" }}
        </button>
      </div>
      <div class="form-row compact">
        <label>更新代理（仅更新）</label>
        <input
          class="input"
          v-model="updatePreferencesDraft.proxyUrl"
          placeholder="http://127.0.0.1:7890，仅用于检查和下载更新"
          style="flex: 1"
        />
      </div>
      <div class="form-row compact sync-status-panel update-panel">
        <div class="sync-status-row">
          <span class="sync-status-label">当前版本:</span>
          <span class="sync-status-value">{{ currentVersionLabel }}</span>
        </div>
        <div class="sync-status-row">
          <span class="sync-status-label">最近检查:</span>
          <span class="sync-status-value">{{ formatDateTime(updatePreferences.lastCheckAt) }}</span>
        </div>
        <div class="sync-status-row">
          <span class="sync-status-label">更新状态:</span>
          <span class="sync-status-value" :class="{ 'is-error': !!updateCheckError }">{{ updateStatusText }}</span>
        </div>
      </div>
      <div v-if="availableUpdate" class="update-card">
        <div class="update-card-header">
          <span class="update-version">发现新版本 {{ formatVersionLabel(availableUpdate.version) }}</span>
          <span v-if="isUpdateIgnored" class="tag">已忽略</span>
        </div>
        <div class="update-meta">
          <span>当前版本 {{ formatVersionLabel(availableUpdate.currentVersion) }}</span>
          <span>发布时间 {{ formatDateTime(availableUpdate.date) }}</span>
        </div>
        <div class="update-notes">{{ updateNotesText }}</div>
        <div class="update-actions">
          <button
            class="button"
            type="button"
            :disabled="updateChecking || updateInstalling"
            @click="handleInstallUpdate"
          >
            {{ updatePrimaryActionLabel }}
          </button>
          <button
            v-if="!isUpdateIgnored"
            class="button secondary"
            type="button"
            :disabled="updateInstalling"
            @click="ignoreAvailableUpdate"
          >
            忽略此版本
          </button>
          <button
            v-else
            class="button secondary"
            type="button"
            :disabled="updateInstalling"
            @click="restoreIgnoredUpdate"
          >
            恢复提醒
          </button>
        </div>
        <div v-if="updateInstalling" class="update-progress">
          <div class="update-progress-track">
            <span class="update-progress-fill" :style="{ width: `${updateProgressPercent ?? 8}%` }"></span>
          </div>
          <span class="update-progress-text">{{ updateProgressText }}</span>
        </div>
      </div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { formatDateTime } from "../format";
import { formatQuietHoursHint } from "../quietHours";
import { acceleratorFromEvent, formatAccelerator } from "../shortcut";
import { formatVersionLabel, normalizeUpdateProxyUrl } from "../update";
import { useSettings } from "../composables/useSettings";
import { useUiPrefs } from "../composables/useUiPrefs";
import { useUpdater } from "../composables/useUpdater";

const props = defineProps<{ appVersion: string }>();

const {
  settingsOpen,
  settingsDraft,
  quickAddShortcutError,
  applyQuickAddShortcut,
  refreshSyncStatus,
  openData
} = useSettings();
const { uiScale, windowOpacity } = useUiPrefs();
const {
  updatePreferences,
  updatePreferencesDraft,
  availableUpdate,
  updateChecking,
  updateInstalling,
  updateCheckError,
  isUpdateIgnored,
  updateNotesText,
  updateProgressPercent,
  updateStatusText,
  updatePrimaryActionLabel,
  updateProgressText,
  persistUpdatePreferencesState,
  handleCheckForUpdates,
  ignoreAvailableUpdate,
  restoreIgnoredUpdate,
  handleInstallUpdate
} = useUpdater();

type ShortcutField = "quickAdd" | "stickyToggle";
const shortcutRecording = ref<ShortcutField | null>(null);
const uiScalePercent = computed(() => Math.round(uiScale.value * 100));
const windowOpacityPercent = computed(() => Math.round(windowOpacity.value * 100));
// 吸附靠 Windows 的鼠标按键状态判断拖动结束，其他平台交给窗口管理器。
const isWindows = typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);
const stickySnapHint = isWindows
  ? "拖到屏幕边缘或其他便签旁时自动对齐，松开时按住 Alt 不吸附"
  : "仅在 Windows 上生效";
const quietHoursHint = computed(() =>
  formatQuietHoursHint(settingsDraft.quietHoursStart, settingsDraft.quietHoursEnd)
);
const currentVersionLabel = computed(() => (props.appVersion ? formatVersionLabel(props.appVersion) : "-"));

const handleShortcutKeydown = (event: KeyboardEvent, field: ShortcutField) => {
  const target = event.target as HTMLInputElement | null;
  if (event.key === "Escape") {
    target?.blur();
    return;
  }
  const accelerator = acceleratorFromEvent(event);
  if (!accelerator) {
    return;
  }
  if (field === "quickAdd") {
    settingsDraft.quickAddShortcut = accelerator;
  } else {
    settingsDraft.stickyToggleShortcut = accelerator;
  }
  target?.blur();
};

const saveSettings = async () => {
  settingsDraft.windowOpacity = windowOpacity.value;
  await api.saveSettings({ ...settingsDraft });
  if (!(await applyQuickAddShortcut())) {
    alert(`设置已保存，但${quickAddShortcutError.value}`);
  }
  await api.setAutoStart(settingsDraft.autoStartEnabled);
  updatePreferences.autoCheckEnabled = updatePreferencesDraft.autoCheckEnabled;
  updatePreferences.proxyUrl = normalizeUpdateProxyUrl(updatePreferencesDraft.proxyUrl);
  persistUpdatePreferencesState();
  settingsOpen.value = false;
  await refreshSyncStatus();
};
</script>
