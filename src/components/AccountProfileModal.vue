<template>
  <Modal
    :open="profileModalOpen"
    title="账号资料与安全"
    hide-footer
    @close="profileModalOpen = false"
  >
    <div class="modal-section">
      <div class="cloud-field">
        <label>用户名（不可修改）</label>
        <input class="input" :value="cloudStatus?.username || ''" disabled />
      </div>
      <div class="cloud-field">
        <label>昵称</label>
        <div class="form-row compact">
          <input class="input" v-model="nicknameDraft" placeholder="输入展示昵称" style="flex: 1" />
          <button type="button" class="button" :disabled="savingProfile" @click="handleSaveProfile">
            {{ savingProfile ? "保存中..." : "保存昵称" }}
          </button>
        </div>
      </div>
    </div>

    <div class="modal-section">
      <div class="cell-title" style="margin-bottom: 8px;">修改登录密码</div>
      <div class="cloud-field">
        <label>当前密码</label>
        <input class="input" type="password" v-model="currentPassword" autocomplete="current-password" />
      </div>
      <div class="cloud-field">
        <label>新密码（至少 6 位，修改后其他设备需重新登录）</label>
        <input class="input" type="password" v-model="newPassword" autocomplete="new-password" />
      </div>
      <div class="cloud-field">
        <label>确认新密码</label>
        <input class="input" type="password" v-model="confirmPassword" autocomplete="new-password" />
      </div>
      <div v-if="msg" class="cloud-banner" :class="msgError ? 'is-danger' : 'is-success'">
        {{ msg }}
      </div>
      <div class="modal-footer-inline">
        <button type="button" class="button secondary" @click="profileModalOpen = false">关闭</button>
        <button type="button" class="button" :disabled="savingPassword" @click="handleChangePassword">
          {{ savingPassword ? "修改中..." : "更新密码" }}
        </button>
      </div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { errorMessage } from "../format";
import { useCloud } from "../composables/useCloud";

const { profileModalOpen, cloudStatus, refreshCloudStatus } = useCloud();

const nicknameDraft = ref("");
const currentPassword = ref("");
const newPassword = ref("");
const confirmPassword = ref("");
const savingProfile = ref(false);
const savingPassword = ref(false);
const msg = ref("");
const msgError = ref(false);

watch(profileModalOpen, open => {
  if (open) {
    nicknameDraft.value = cloudStatus.value?.nickname || "";
    currentPassword.value = "";
    newPassword.value = "";
    confirmPassword.value = "";
    msg.value = "";
    msgError.value = false;
  }
});

const handleSaveProfile = async () => {
  savingProfile.value = true;
  msg.value = "";
  msgError.value = false;
  try {
    await api.cloudUpdateProfile(nicknameDraft.value.trim());
    await refreshCloudStatus();
    msg.value = "昵称已更新";
  } catch (error) {
    msgError.value = true;
    msg.value = errorMessage(error);
  } finally {
    savingProfile.value = false;
  }
};

const handleChangePassword = async () => {
  if (newPassword.value.length < 6) {
    msgError.value = true;
    msg.value = "新密码长度不能少于 6 位";
    return;
  }
  if (newPassword.value !== confirmPassword.value) {
    msgError.value = true;
    msg.value = "两次输入的新密码不一致";
    return;
  }
  savingPassword.value = true;
  msg.value = "";
  msgError.value = false;
  try {
    await api.cloudChangePassword({
      currentPassword: currentPassword.value,
      newPassword: newPassword.value
    });
    currentPassword.value = "";
    newPassword.value = "";
    confirmPassword.value = "";
    await refreshCloudStatus();
    msg.value = "密码修改成功，其他设备已自动下线";
  } catch (error) {
    msgError.value = true;
    msg.value = errorMessage(error);
  } finally {
    savingPassword.value = false;
  }
};
</script>
