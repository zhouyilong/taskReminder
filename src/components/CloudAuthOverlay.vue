<template>
  <Transition name="modal">
    <div v-if="visible" class="cloud-auth-overlay" :class="{ 'is-blocking': isAuthBlocking }">
      <div class="cloud-auth-card" role="dialog" aria-modal="true">
        <div class="cloud-auth-header">
          <div>
            <h2 class="cloud-auth-title">
              {{ mustChangePasswordMode ? "首次登录请修改密码" : "登录云端账号" }}
            </h2>
            <p class="cloud-auth-subtitle">
              {{
                mustChangePasswordMode
                  ? "管理员要求此账号在继续使用前设置新密码，改密后将自动进入云端模式。"
                  : "登录由管理员分配的账号，启用多设备实时同步。"
              }}
            </p>
          </div>
          <button
            v-if="!isAuthBlocking"
            type="button"
            class="icon-button"
            title="关闭"
            @click="closeLoginModal"
          >
            ✕
          </button>
        </div>

        <!-- 强制改密表单 -->
        <form v-if="mustChangePasswordMode" class="cloud-auth-form" @submit.prevent="handleForcePasswordChange">
          <div class="cloud-field">
            <label>当前密码</label>
            <input
              class="input"
              type="password"
              v-model="currentPassword"
              autocomplete="current-password"
              placeholder="输入当前登录密码"
              required
            />
          </div>
          <div class="cloud-field">
            <label>新密码（至少 6 位）</label>
            <input
              class="input"
              type="password"
              v-model="newPassword"
              autocomplete="new-password"
              placeholder="设置新密码"
              required
            />
          </div>
          <div class="cloud-field">
            <label>确认新密码</label>
            <input
              class="input"
              type="password"
              v-model="confirmPassword"
              autocomplete="new-password"
              placeholder="再次输入新密码"
              required
            />
          </div>
          <div v-if="errorMsg" class="cloud-banner is-danger">{{ errorMsg }}</div>
          <div class="cloud-auth-actions">
            <button class="button" type="submit" :disabled="submitting">
              {{ submitting ? "提交中..." : "确认修改并进入" }}
            </button>
            <button
              class="button secondary"
              type="button"
              :disabled="submitting"
              @click="handleEscapeToLocal"
            >
              切换至本地模式
            </button>
          </div>
        </form>

        <!-- 登录表单 -->
        <form v-else class="cloud-auth-form" @submit.prevent="handleLogin">
          <div v-if="cloudStatus?.error && isAuthBlocking" class="cloud-banner is-warning">
            {{ cloudStatus.error }}
          </div>

          <div class="cloud-field">
            <label>用户名</label>
            <input
              class="input"
              type="text"
              v-model="username"
              autocomplete="username"
              placeholder="请输入用户名"
              required
            />
          </div>
          <div class="cloud-field">
            <label>密码</label>
            <div class="cloud-password-row">
              <input
                class="input"
                :type="showPassword ? 'text' : 'password'"
                v-model="password"
                autocomplete="current-password"
                placeholder="请输入密码"
                style="flex: 1"
                required
              />
              <button
                type="button"
                class="button secondary"
                @click="showPassword = !showPassword"
              >
                {{ showPassword ? "隐藏" : "显示" }}
              </button>
            </div>
          </div>

          <!-- 服务器设置折叠区 -->
          <details class="cloud-server-details" :open="serverExpanded">
            <summary @click.prevent="serverExpanded = !serverExpanded">
              服务器设置（当前：{{ effectiveUrlDisplay }}）
            </summary>
            <div class="cloud-server-body">
              <div class="cloud-field">
                <label>服务端地址（留空使用官方默认地址）</label>
                <input
                  class="input"
                  type="url"
                  v-model="serverUrlDraft"
                  :placeholder="defaultServerUrl"
                />
              </div>
              <div v-if="insecureHttpWarning" class="cloud-banner is-warning">
                当前填写的是非加密 HTTP 公网地址，账号与业务数据将以明文传输，建议配置 HTTPS。
              </div>
              <div class="form-row compact">
                <button
                  type="button"
                  class="button secondary"
                  :disabled="testing"
                  @click="handleTestServer"
                >
                  {{ testing ? "测试中..." : "测试连接" }}
                </button>
                <button
                  type="button"
                  class="button secondary"
                  @click="serverUrlDraft = ''"
                >
                  恢复默认地址
                </button>
                <span v-if="testHint" class="field-hint" :class="{ 'is-error': testError }">
                  {{ testHint }}
                </span>
              </div>
            </div>
          </details>

          <div v-if="errorMsg" class="cloud-banner is-danger">{{ errorMsg }}</div>

          <div class="cloud-auth-actions">
            <button class="button" type="submit" :disabled="submitting">
              {{ submitting ? "登录中..." : "登录" }}
            </button>
            <button
              v-if="isAuthBlocking"
              class="button secondary"
              type="button"
              :disabled="submitting"
              @click="handleEscapeToLocal"
            >
              切换至本地模式
            </button>
            <button
              v-else
              class="button secondary"
              type="button"
              :disabled="submitting"
              @click="closeLoginModal"
            >
              取消
            </button>
          </div>
          <p class="cloud-auth-note">不开放自助注册，如需账号请联系管理员在设置中创建。</p>
        </form>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { api } from "../api";
import { isInsecureRemoteUrl } from "../cloudMode";
import { errorMessage } from "../format";
import { useCloud } from "../composables/useCloud";
import { useSettings } from "../composables/useSettings";

const {
  cloudStatus,
  isAuthBlocking,
  authModalOpen,
  closeLoginModal,
  handleLoginCompleted,
  escapeToLocalMode,
  refreshCloudStatus
} = useCloud();
const { settingsDraft } = useSettings();

const visible = computed(() => isAuthBlocking.value || authModalOpen.value);
const mustChangePasswordMode = computed(
  () => Boolean(cloudStatus.value?.loggedIn && cloudStatus.value?.mustChangePassword)
);

const username = ref("");
const password = ref("");
const showPassword = ref(false);
const serverUrlDraft = ref("");
const serverExpanded = ref(false);
const submitting = ref(false);
const errorMsg = ref("");
const testing = ref(false);
const testHint = ref("");
const testError = ref(false);

const currentPassword = ref("");
const newPassword = ref("");
const confirmPassword = ref("");

const defaultServerUrl = computed(
  () => cloudStatus.value?.defaultServerUrl || "http://104.168.101.151:8080"
);
const effectiveUrlDisplay = computed(
  () => serverUrlDraft.value.trim() || defaultServerUrl.value
);
const insecureHttpWarning = computed(() => isInsecureRemoteUrl(effectiveUrlDisplay.value));

watch(visible, open => {
  if (open) {
    errorMsg.value = "";
    testHint.value = "";
    testError.value = false;
    password.value = "";
    currentPassword.value = "";
    newPassword.value = "";
    confirmPassword.value = "";
    username.value = settingsDraft.cloudUsername || cloudStatus.value?.username || "";
    serverUrlDraft.value = settingsDraft.cloudServerUrl || "";
  }
});

const handleTestServer = async () => {
  testing.value = true;
  testHint.value = "";
  testError.value = false;
  try {
    const res = await api.cloudTestConnection(serverUrlDraft.value.trim());
    testHint.value = `连接成功（服务端 v${res.version}）`;
  } catch (error) {
    testError.value = true;
    testHint.value = errorMessage(error);
  } finally {
    testing.value = false;
  }
};

const handleLogin = async () => {
  if (!username.value.trim() || !password.value) return;
  submitting.value = true;
  errorMsg.value = "";
  try {
    const status = await api.cloudLogin({
      serverUrl: serverUrlDraft.value.trim(),
      username: username.value.trim(),
      password: password.value
    });
    password.value = "";
    await handleLoginCompleted(status);
  } catch (error) {
    errorMsg.value = errorMessage(error);
  } finally {
    submitting.value = false;
  }
};

const handleForcePasswordChange = async () => {
  if (newPassword.value.length < 6) {
    errorMsg.value = "新密码长度不能少于 6 位";
    return;
  }
  if (newPassword.value !== confirmPassword.value) {
    errorMsg.value = "两次输入的新密码不一致";
    return;
  }
  submitting.value = true;
  errorMsg.value = "";
  try {
    await api.cloudChangePassword({
      currentPassword: currentPassword.value,
      newPassword: newPassword.value
    });
    await refreshCloudStatus();
  } catch (error) {
    errorMsg.value = errorMessage(error);
  } finally {
    submitting.value = false;
  }
};

const handleEscapeToLocal = async () => {
  try {
    await escapeToLocalMode();
  } catch (error) {
    errorMsg.value = errorMessage(error);
  }
};
</script>
