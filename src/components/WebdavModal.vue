<template>
  <Modal :open="webdavOpen" title="云同步设置" @close="webdavOpen = false" @confirm="saveWebdavSettings">
    <div class="modal-section">
      <div class="form-row compact">
        <label>
          <input type="checkbox" v-model="settingsDraft.webdavEnabled" /> 启用 WebDAV
        </label>
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact">
        <label>服务</label>
        <select class="select" v-model="presetId" @change="handlePresetChange">
          <option v-for="preset in WEBDAV_PRESETS" :key="preset.id" :value="preset.id">{{ preset.label }}</option>
        </select>
      </div>
      <div class="form-row compact">
        <span class="field-hint">{{ presetHint }}</span>
      </div>
      <div class="form-row compact">
        <input class="input" v-model="settingsDraft.webdavUrl" placeholder="WebDAV 地址" style="flex: 1" />
      </div>
      <div v-if="urlWarning" class="form-row compact">
        <span class="field-hint is-warning">{{ urlWarning }}</span>
      </div>
      <div class="form-row compact">
        <input class="input" v-model="settingsDraft.webdavUsername" placeholder="用户名" style="flex: 1" />
        <input
          class="input"
          :type="webdavPasswordVisible ? 'text' : 'password'"
          v-model="settingsDraft.webdavPassword"
          placeholder="密码"
          style="flex: 1"
        />
        <button class="button secondary" type="button" @click="webdavPasswordVisible = !webdavPasswordVisible">
          {{ webdavPasswordVisible ? "隐藏" : "显示" }}
        </button>
      </div>
      <div class="form-row compact">
        <input class="input" v-model="settingsDraft.webdavRootPath" placeholder="远端路径" style="flex: 1" />
        <input class="input" type="number" min="1" v-model.number="settingsDraft.webdavSyncIntervalMinutes" placeholder="同步频率(分钟)" style="width: 160px" />
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact">
        <label>
          <input type="checkbox" v-model="settingsDraft.syncEncryptionEnabled" /> 端到端加密
        </label>
        <span class="field-hint">上传前用同步密码加密，服务器只能看到密文</span>
      </div>
      <template v-if="settingsDraft.syncEncryptionEnabled">
        <div class="form-row compact">
          <input
            class="input"
            :type="passphraseVisible ? 'text' : 'password'"
            v-model="settingsDraft.syncPassphrase"
            placeholder="同步密码（至少 8 个字符）"
            autocomplete="new-password"
            style="flex: 1"
          />
          <button class="button secondary" type="button" @click="passphraseVisible = !passphraseVisible">
            {{ passphraseVisible ? "隐藏" : "显示" }}
          </button>
        </div>
        <div class="field-hint" :class="{ 'is-error': passphraseError }">
          {{
            passphraseError ||
            "所有设备需开启加密并填写相同的密码。密码只保存在本机，忘记后无法解密云端数据（本机数据不受影响）。旧版本无法再同步此目录，请一起升级。"
          }}
        </div>
      </template>
    </div>
    <div class="modal-section">
      <div class="form-row compact" style="gap: 8px;">
        <button class="button secondary" type="button" @click="handleTestWebdav">测试连接</button>
        <button class="button secondary" type="button" @click="handleSyncNow">立即同步</button>
      </div>
    </div>
    <div class="modal-section">
      <div class="form-row compact sync-status-panel">
        <div class="sync-status-row">
          <span class="sync-status-label">上次同步:</span>
          <span class="sync-status-value">{{ formatDateTime(settingsDraft.webdavLastSyncTime) }}</span>
        </div>
        <div class="sync-status-row">
          <span class="sync-status-label">上次本地变更:</span>
          <span class="sync-status-value">{{ formatDateTime(settingsDraft.webdavLastLocalChangeTime) }}</span>
        </div>
        <div class="sync-status-row">
          <span class="sync-status-label">同步状态:</span>
          <span class="sync-status-value">{{ syncStateLabel(settingsDraft.webdavLastSyncStatus) }}</span>
        </div>
        <div class="sync-status-row">
          <span class="sync-status-label">最近错误:</span>
          <span class="sync-status-value">{{ settingsDraft.webdavLastSyncError || "无" }}</span>
        </div>
      </div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { formatDateTime } from "../format";
import { syncStateLabel } from "../syncStatus";
import {
  WEBDAV_PRESETS,
  applyWebdavPreset,
  detectWebdavPreset,
  findWebdavPreset,
  webdavUrlPlaceholderWarning,
  type WebdavPresetId
} from "../webdavPresets";
import { useSettings } from "../composables/useSettings";

const { webdavOpen, settingsDraft, loadSettings, refreshSyncStatus } = useSettings();
const webdavPasswordVisible = ref(false);
const passphraseVisible = ref(false);
const presetId = ref<WebdavPresetId>("custom");
const presetHint = computed(() => findWebdavPreset(presetId.value).hint);
const urlWarning = computed(() => webdavUrlPlaceholderWarning(settingsDraft.webdavUrl));

const handlePresetChange = () => {
  Object.assign(settingsDraft, applyWebdavPreset(presetId.value, settingsDraft));
};

const MIN_PASSPHRASE_CHARS = 8;
const passphraseError = computed(() => {
  if (!settingsDraft.syncEncryptionEnabled) {
    return "";
  }
  const length = [...settingsDraft.syncPassphrase].length;
  return length > 0 && length < MIN_PASSPHRASE_CHARS ? `同步密码至少需要 ${MIN_PASSPHRASE_CHARS} 个字符` : "";
});

watch(webdavOpen, open => {
  if (open) {
    webdavPasswordVisible.value = false;
    passphraseVisible.value = false;
    presetId.value = detectWebdavPreset(settingsDraft.webdavUrl);
  }
});

const saveWebdavSettings = async () => {
  if (settingsDraft.syncEncryptionEnabled && [...settingsDraft.syncPassphrase].length < MIN_PASSPHRASE_CHARS) {
    alert(`开启端到端加密需要设置至少 ${MIN_PASSPHRASE_CHARS} 个字符的同步密码`);
    return;
  }
  await api.saveSettings({ ...settingsDraft });
  await api.setAutoStart(settingsDraft.autoStartEnabled);
  webdavPasswordVisible.value = false;
  passphraseVisible.value = false;
  webdavOpen.value = false;
  await refreshSyncStatus();
};

const handleTestWebdav = async () => {
  const result = await api.testWebDav({ ...settingsDraft });
  alert(result.message);
  await loadSettings();
};

const handleSyncNow = async () => {
  await api.syncNow("manual");
};
</script>
