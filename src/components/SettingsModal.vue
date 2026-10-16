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
        <button type="button" class="button secondary" title="播放提醒弹窗的提示音" @click="playChime">试听</button>
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
          <input type="checkbox" v-model="settingsDraft.holidayAutoUpdate" /> 自动更新节假日数据
        </label>
        <button class="button secondary" type="button" :disabled="holidayChecking" @click="handleCheckHolidays">
          {{ holidayChecking ? "检查中…" : "立即检查" }}
        </button>
        <span class="field-hint" :class="{ 'is-error': holidayCheckError }">{{ holidayHint }}</span>
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
      <div class="form-row compact shortcut-help-row">
        <label>主窗口快捷键</label>
        <dl class="shortcut-help">
          <template v-for="item in SHORTCUT_HELP" :key="item.keys">
            <dt><kbd>{{ item.keys }}</kbd></dt>
            <dd>{{ item.label }}</dd>
          </template>
        </dl>
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact">
        <label>数据</label>
        <button class="button secondary" type="button" @click="openData">导入导出与备份…</button>
        <span class="field-hint">JSON 备份、Markdown、日历（ICS），每天自动备份</span>
      </div>
      <div class="form-row compact">
        <label>已完成待办保留</label>
        <select class="select" v-model.number="settingsDraft.completedRetentionDays">
          <option v-for="item in COMPLETED_RETENTION_OPTIONS" :key="item.value" :value="item.value">{{ item.label }}</option>
        </select>
        <span class="field-hint">{{ completedRetentionHint(settingsDraft.completedRetentionDays, settingsDraft.webdavEnabled) }}</span>
      </div>
      <div class="form-row compact">
        <label>自动备份保留</label>
        <select class="select" v-model.number="settingsDraft.autoBackupKeep" aria-label="自动备份保留份数">
          <option v-for="item in AUTO_BACKUP_KEEP_OPTIONS" :key="item.value" :value="item.value">{{ item.label }}</option>
        </select>
        <span class="field-hint">每天一份；调小后在下一次备份时删除更早的备份</span>
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact">
        <label>全屏提醒效果</label>
        <select class="select" v-model="settingsDraft.attentionEffect" aria-label="全屏提醒效果">
          <option v-for="item in ATTENTION_EFFECT_OPTIONS" :key="item.value" :value="item.value">{{ item.label }}</option>
        </select>
        <label title="闪烁结束后保留边缘的柔光（横幅收起），处理完提醒才消失">
          <input
            type="checkbox"
            v-model="settingsDraft.attentionUntilHandled"
            :disabled="settingsDraft.attentionEffect === 'off'"
          />
          持续到处理
        </label>
        <button
          type="button"
          class="button secondary"
          title="按当前选择在每块屏幕上播放一次"
          :disabled="settingsDraft.attentionEffect === 'off' || attentionPreviewing"
          @click="handlePreviewAttention"
        >
          预览
        </button>
      </div>
      <div class="form-row compact">
        <span class="field-hint" :class="{ 'is-error': !!attentionPreviewError }">{{ attentionHint }}</span>
      </div>
      <div class="form-row compact" title="越强光晕越宽、越向屏幕中间延伸，可点“预览”查看">
        <label for="attention-intensity">光晕强度</label>
        <input
          id="attention-intensity"
          class="settings-range"
          type="range"
          :min="ATTENTION_INTENSITY_MIN"
          :max="ATTENTION_INTENSITY_MAX"
          step="1"
          v-model.number="settingsDraft.attentionIntensity"
          :disabled="settingsDraft.attentionEffect === 'off'"
          style="flex: 1"
        />
        <span class="tag">{{ attentionIntensityLabel(settingsDraft.attentionIntensity) }}</span>
      </div>
      <div class="form-row compact" title="错过的提醒仍用橙色标出">
        <label id="attention-color-label">光晕颜色</label>
        <div class="attention-swatches" role="radiogroup" aria-labelledby="attention-color-label">
          <button
            v-for="item in ATTENTION_COLOR_PRESETS"
            :key="item.value || 'default'"
            type="button"
            role="radio"
            class="attention-swatch"
            :class="{ active: attentionColor === item.value, 'is-default': !item.value }"
            :style="item.value ? { background: item.value } : undefined"
            :aria-checked="attentionColor === item.value"
            :aria-label="item.label"
            :title="item.label"
            :disabled="settingsDraft.attentionEffect === 'off'"
            @click="settingsDraft.attentionColor = item.value"
          ></button>
          <input
            type="color"
            class="attention-swatch is-custom"
            :class="{ active: attentionColorCustom }"
            :value="attentionColorPickerValue"
            aria-label="自选光晕颜色"
            title="自选颜色"
            :disabled="settingsDraft.attentionEffect === 'off'"
            @input="settingsDraft.attentionColor = normalizeAttentionColor(($event.target as HTMLInputElement).value)"
          />
        </div>
        <span class="tag">{{ attentionColorLabel(settingsDraft.attentionColor) }}</span>
      </div>
      <div class="form-row compact">
        <label>未处理时再次提醒</label>
        <select class="select" v-model.number="settingsDraft.attentionRepeatMinutes" aria-label="未处理时再次提醒">
          <option v-for="item in ATTENTION_REPEAT_OPTIONS" :key="item.value" :value="item.value">{{ item.label }}</option>
        </select>
        <span class="field-hint">{{ attentionRepeatHint(settingsDraft.attentionRepeatMinutes, settingsDraft.attentionEffect) }}</span>
      </div>
      <div class="form-row compact">
        <label>提醒弹窗位置</label>
        <select class="select" v-model="settingsDraft.notificationScreen" aria-label="提醒弹窗位置">
          <option v-for="item in NOTIFICATION_SCREEN_OPTIONS" :key="item.value" :value="item.value">{{ item.label }}</option>
        </select>
        <span class="field-hint">显示在该屏幕右下角，避开任务栏</span>
      </div>
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
import { computed, ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { errorMessage, formatDateTime } from "../format";
import { formatYearRange } from "../recurring";
import { formatQuietHoursHint } from "../quietHours";
import { acceleratorFromEvent, formatAccelerator } from "../shortcut";
import { formatVersionLabel, normalizeUpdateProxyUrl } from "../update";
import { useAppData } from "../composables/useAppData";
import { useSettings } from "../composables/useSettings";
import { useUiPrefs } from "../composables/useUiPrefs";
import { useUpdater } from "../composables/useUpdater";
import { playChime } from "../notificationSound";
import { SHORTCUT_HELP } from "../keyboard";
import { AUTO_BACKUP_KEEP_OPTIONS, COMPLETED_RETENTION_OPTIONS, completedRetentionHint } from "../retention";
import {
  ATTENTION_COLOR_PRESETS,
  ATTENTION_EFFECT_OPTIONS,
  ATTENTION_INTENSITY_MAX,
  ATTENTION_INTENSITY_MIN,
  ATTENTION_REPEAT_OPTIONS,
  NOTIFICATION_SCREEN_OPTIONS,
  attentionColorLabel,
  attentionEffectHint,
  attentionIntensityLabel,
  attentionRepeatHint,
  normalizeAttentionColor
} from "../attentionEffect";

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
// 节假日数据：显示已覆盖的年份与最近一次手动检查的结果。
const { holidayYears, loadHolidayYears } = useAppData();
const holidayChecking = ref(false);
const holidayCheckMessage = ref("");
const holidayCheckError = ref(false);
const holidayHint = computed(() => {
  if (holidayCheckMessage.value) {
    return holidayCheckMessage.value;
  }
  return holidayYears.value.length
    ? `已有 ${formatYearRange(holidayYears.value)}的法定节假日安排，每天从项目仓库检查一次`
    : "暂无节假日数据，每天从项目仓库检查一次";
});
const handleCheckHolidays = async () => {
  holidayChecking.value = true;
  holidayCheckError.value = false;
  try {
    const result = await api.checkHolidayUpdates();
    holidayYears.value = result.years;
    holidayCheckMessage.value = result.changed
      ? `已更新，现有 ${formatYearRange(result.years)}的节假日安排`
      : `已是最新（${formatYearRange(result.years)}）`;
  } catch (error) {
    holidayCheckError.value = true;
    holidayCheckMessage.value = `检查失败：${errorMessage(error)}`;
    try {
      await loadHolidayYears();
    } catch {
      // 忽略：只影响提示文案。
    }
  } finally {
    holidayChecking.value = false;
  }
};

// 吸附挂在 Windows 的系统移动循环上判断拖动结束，其他平台交给窗口管理器。
const isWindows = typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);
const stickySnapHint = isWindows
  ? "拖到屏幕边缘或其他便签旁时自动对齐，拖出屏幕时收回到边缘；松开时按住 Alt 不吸附"
  : "仅在 Windows 上生效";
// 全屏提醒效果：Linux 的透明窗口首帧后不重绘，目前只在 Windows 上播放。
const attentionPreviewing = ref(false);
const attentionPreviewError = ref("");
const attentionHint = computed(() => {
  if (attentionPreviewError.value) {
    return attentionPreviewError.value;
  }
  const hint = attentionEffectHint(settingsDraft.attentionEffect, settingsDraft.attentionUntilHandled);
  return isWindows || settingsDraft.attentionEffect === "off" ? hint : `${hint}（目前只支持 Windows）`;
});
const attentionColor = computed(() => normalizeAttentionColor(settingsDraft.attentionColor));
const attentionColorCustom = computed(
  () => !ATTENTION_COLOR_PRESETS.some(item => item.value === attentionColor.value)
);
// 取色器显示当前自选颜色；尚未自选时从默认蓝开始。
const attentionColorPickerValue = computed(() => (attentionColorCustom.value ? attentionColor.value : "#4f7cff"));
const handlePreviewAttention = async () => {
  attentionPreviewing.value = true;
  attentionPreviewError.value = "";
  try {
    await api.previewAttention({
      effect: settingsDraft.attentionEffect,
      untilHandled: settingsDraft.attentionUntilHandled,
      intensity: settingsDraft.attentionIntensity,
      color: settingsDraft.attentionColor,
      screen: settingsDraft.notificationScreen,
      theme: settingsDraft.notificationTheme
    });
  } catch (error) {
    attentionPreviewError.value = `预览失败：${errorMessage(error)}`;
  } finally {
    // 防止连点重复播放。
    window.setTimeout(() => (attentionPreviewing.value = false), 1500);
  }
};
watch(
  () => [settingsDraft.attentionEffect, settingsDraft.attentionUntilHandled],
  () => (attentionPreviewError.value = "")
);
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

<style scoped>
.attention-swatches {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.attention-swatch {
  width: 22px;
  height: 22px;
  padding: 0;
  border: 2px solid var(--bg-surface);
  border-radius: 50%;
  box-shadow: 0 0 0 1px var(--border-strong);
  cursor: pointer;
  transition: box-shadow 0.15s ease, transform 0.15s ease;
}

.attention-swatch.is-default {
  background: var(--attention-glow);
}

.attention-swatch:hover:not(:disabled) {
  transform: scale(1.1);
}

.attention-swatch.active {
  box-shadow: 0 0 0 2px var(--primary);
}

.attention-swatch:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

.attention-swatch:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

/* 自选颜色：原生取色器，去掉内边距与边框，显示成与预设一致的圆点；未自选时为虚线圈。 */
.attention-swatch.is-custom {
  appearance: none;
  -webkit-appearance: none;
  background: transparent;
  overflow: hidden;
}

.attention-swatch.is-custom:not(.active) {
  border-style: dashed;
  border-color: var(--text-faint);
  box-shadow: none;
}

.attention-swatch.is-custom::-webkit-color-swatch-wrapper {
  padding: 0;
}

.attention-swatch.is-custom::-webkit-color-swatch {
  border: none;
  border-radius: 50%;
}

.shortcut-help-row {
  align-items: flex-start;
}

.shortcut-help {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 6px 12px;
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
}

.shortcut-help dd {
  margin: 0;
}

.shortcut-help kbd {
  font-family: var(--font-sans);
  font-size: 11px;
  padding: 1px 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--text-base);
  background: var(--bg-surface);
}
</style>
