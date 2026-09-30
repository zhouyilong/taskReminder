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
        <span class="field-hint">{{ secretStorageHint }}</span>
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
        <div v-if="passphraseMismatch" class="field-hint is-error passphrase-mismatch">
          云端数据无法用本机的同步密码解密，自动同步已暂停。如果在其他设备上更换过同步密码，请在上方填写新密码并保存。
        </div>
        <div v-if="canChangePassphrase" class="form-row compact">
          <button
            class="button secondary"
            type="button"
            :disabled="changeBusy"
            @click="changeFormOpen = !changeFormOpen"
          >
            {{ changeFormOpen ? "收起" : "更换同步密码…" }}
          </button>
          <span v-if="changeResult" class="field-hint" :class="{ 'is-error': !changeResult.ok }">
            {{ changeResult.message }}
          </span>
        </div>
        <div v-if="canChangePassphrase && changeFormOpen" class="passphrase-change">
          <input
            class="input"
            :type="changeVisible ? 'text' : 'password'"
            v-model="changeCurrent"
            placeholder="当前同步密码"
            autocomplete="current-password"
          />
          <input
            class="input"
            :type="changeVisible ? 'text' : 'password'"
            v-model="changeNext"
            placeholder="新同步密码（至少 8 个字符）"
            autocomplete="new-password"
          />
          <input
            class="input"
            :type="changeVisible ? 'text' : 'password'"
            v-model="changeConfirm"
            placeholder="再次输入新密码"
            autocomplete="new-password"
            @keydown.enter.prevent="submitPassphraseChange"
          />
          <div class="form-row compact">
            <button
              class="button"
              type="button"
              :disabled="changeBusy || changeValidation.incomplete || !!changeValidation.message || hasUnsavedSyncChanges"
              @click="submitPassphraseChange"
            >
              {{ changeBusy ? "正在更换…" : "确认更换" }}
            </button>
            <button class="button secondary" type="button" @click="changeVisible = !changeVisible">
              {{ changeVisible ? "隐藏" : "显示" }}
            </button>
          </div>
          <div class="field-hint" :class="{ 'is-error': changeValidation.message || hasUnsavedSyncChanges }">
            {{
              changeValidation.message ||
              (hasUnsavedSyncChanges
                ? "云同步设置有未保存的修改，请先保存再更换密码"
                : "会先用当前密码合并云端数据，再用新密码重新加密上传。其他设备下次同步时会提示密码不匹配，填入新密码即可恢复。")
            }}
          </div>
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
import { errorMessage, formatDateTime } from "../format";
import { parseSyncState, syncStateLabel } from "../syncStatus";
import { MIN_PASSPHRASE_CHARS, passphraseChangeError, passphraseLength } from "../syncPassphrase";
import type { AppSettings } from "../types";
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
const secretStorageHint = computed(() =>
  settingsDraft.secretStorage === "keyring"
    ? "WebDAV 密码与同步密码保存在 Windows 凭据管理器中，不写入数据库与备份"
    : "WebDAV 密码与同步密码保存在本机数据库中（不会上传到云端）"
);

const handlePresetChange = () => {
  Object.assign(settingsDraft, applyWebdavPreset(presetId.value, settingsDraft));
};

const passphraseError = computed(() => {
  if (!settingsDraft.syncEncryptionEnabled) {
    return "";
  }
  const length = passphraseLength(settingsDraft.syncPassphrase);
  return length > 0 && length < MIN_PASSPHRASE_CHARS ? `同步密码至少需要 ${MIN_PASSPHRASE_CHARS} 个字符` : "";
});

const passphraseMismatch = computed(
  () => parseSyncState(settingsDraft.webdavLastSyncStatus) === "passphrase_mismatch"
);

// 更换同步密码用的是已保存的设置：记下打开弹窗时（或保存后）的同步相关字段，草稿改动过就先要求保存。
const SYNC_FIELDS = [
  "webdavEnabled",
  "webdavUrl",
  "webdavUsername",
  "webdavPassword",
  "webdavRootPath",
  "syncEncryptionEnabled",
  "syncPassphrase"
] as const satisfies ReadonlyArray<keyof AppSettings>;
const savedSyncFields = ref<Partial<AppSettings>>({});
const rememberSavedSyncFields = () => {
  savedSyncFields.value = Object.fromEntries(SYNC_FIELDS.map(key => [key, settingsDraft[key]]));
};
const hasUnsavedSyncChanges = computed(() =>
  SYNC_FIELDS.some(key => savedSyncFields.value[key] !== settingsDraft[key])
);
const canChangePassphrase = computed(
  () =>
    !passphraseMismatch.value &&
    savedSyncFields.value.webdavEnabled === true &&
    savedSyncFields.value.syncEncryptionEnabled === true &&
    passphraseLength(String(savedSyncFields.value.syncPassphrase ?? "")) >= MIN_PASSPHRASE_CHARS
);

const changeFormOpen = ref(false);
const changeVisible = ref(false);
const changeCurrent = ref("");
const changeNext = ref("");
const changeConfirm = ref("");
const changeBusy = ref(false);
const changeResult = ref<{ ok: boolean; message: string } | null>(null);
const changeValidation = computed(() =>
  passphraseChangeError(changeCurrent.value, changeNext.value, changeConfirm.value)
);

const resetChangeForm = () => {
  changeFormOpen.value = false;
  changeVisible.value = false;
  changeCurrent.value = "";
  changeNext.value = "";
  changeConfirm.value = "";
  changeResult.value = null;
};

// 只刷新同步相关字段，保留弹窗中其他未保存的修改。
const refreshSyncFieldsFromBackend = async () => {
  const latest = await api.getSettings();
  settingsDraft.syncPassphrase = latest.syncPassphrase;
  settingsDraft.webdavLastSyncTime = latest.webdavLastSyncTime;
  settingsDraft.webdavLastLocalChangeTime = latest.webdavLastLocalChangeTime;
  settingsDraft.webdavLastSyncStatus = latest.webdavLastSyncStatus;
  settingsDraft.webdavLastSyncError = latest.webdavLastSyncError;
  savedSyncFields.value = { ...savedSyncFields.value, syncPassphrase: latest.syncPassphrase };
  await refreshSyncStatus();
};

const submitPassphraseChange = async () => {
  if (changeBusy.value || changeValidation.value.incomplete || changeValidation.value.message || hasUnsavedSyncChanges.value) {
    return;
  }
  changeBusy.value = true;
  changeResult.value = null;
  try {
    await api.changeSyncPassphrase(changeCurrent.value, changeNext.value);
    changeFormOpen.value = false;
    changeCurrent.value = "";
    changeNext.value = "";
    changeConfirm.value = "";
    changeResult.value = { ok: true, message: "已更换同步密码。请在其他设备的云同步设置中填写新密码。" };
  } catch (error) {
    changeResult.value = { ok: false, message: `更换失败：${errorMessage(error)}` };
  } finally {
    changeBusy.value = false;
    try {
      // 无论成功与否都以后端为准：成功时草稿换成新密码，避免之后“保存”把旧密码写回。
      await refreshSyncFieldsFromBackend();
    } catch (error) {
      console.error("[webdav] 刷新同步设置失败", error);
    }
  }
};

watch(webdavOpen, open => {
  if (open) {
    webdavPasswordVisible.value = false;
    passphraseVisible.value = false;
    presetId.value = detectWebdavPreset(settingsDraft.webdavUrl);
    rememberSavedSyncFields();
    resetChangeForm();
  }
});

const saveWebdavSettings = async () => {
  if (changeBusy.value) {
    alert("正在更换同步密码，请稍候");
    return;
  }
  if (settingsDraft.syncEncryptionEnabled && passphraseLength(settingsDraft.syncPassphrase) < MIN_PASSPHRASE_CHARS) {
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

<style scoped>
.passphrase-change {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-subtle);
}

.passphrase-change .form-row {
  margin-bottom: 0;
}

.passphrase-mismatch {
  margin-top: 6px;
}
</style>
