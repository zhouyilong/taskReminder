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
        <input class="input" v-model="settingsDraft.webdavUrl" placeholder="WebDAV 地址" style="flex: 1" />
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
          <span class="sync-status-value">{{ settingsDraft.webdavLastSyncStatus || "未同步" }}</span>
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
import { ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { formatDateTime } from "../format";
import { useSettings } from "../composables/useSettings";

const { webdavOpen, settingsDraft, loadSettings, refreshSyncStatus } = useSettings();
const webdavPasswordVisible = ref(false);

watch(webdavOpen, open => {
  if (open) {
    webdavPasswordVisible.value = false;
  }
});

const saveWebdavSettings = async () => {
  await api.saveSettings({ ...settingsDraft });
  await api.setAutoStart(settingsDraft.autoStartEnabled);
  webdavPasswordVisible.value = false;
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
