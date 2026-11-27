<template>
  <Modal
    :open="adminUsersOpen"
    title="账号管理（管理员）"
    hide-footer
    @close="adminUsersOpen = false"
  >
    <div class="modal-section">
      <div class="form-row compact" style="justify-content: space-between;">
        <span class="field-hint">
          隐私隔离：管理员仅可维护账号元数据，无法查看或操作其他用户的待办与便签内容。
        </span>
        <button type="button" class="button" @click="toggleCreateForm">
          {{ showCreateForm ? "收起新建" : "+ 新建账号" }}
        </button>
      </div>

      <!-- 新建账号表单 -->
      <form v-if="showCreateForm" class="admin-subcard" @submit.prevent="handleCreateUser">
        <div class="admin-form-grid">
          <div class="cloud-field">
            <label>用户名</label>
            <input class="input" v-model="createDraft.username" placeholder="2-64 位字母/数字/下划线" required />
          </div>
          <div class="cloud-field">
            <label>昵称</label>
            <input class="input" v-model="createDraft.nickname" placeholder="可选，用于展示" />
          </div>
          <div class="cloud-field">
            <label>角色</label>
            <select class="select" v-model="createDraft.role">
              <option value="user">普通用户 (user)</option>
              <option value="admin">管理员 (admin)</option>
            </select>
          </div>
          <div class="cloud-field">
            <label>初始密码</label>
            <div class="cloud-password-row">
              <input class="input" v-model="createDraft.password" placeholder="至少 6 位" style="flex: 1" required />
              <button type="button" class="button secondary" @click="generateCreatePassword">随机</button>
              <button type="button" class="button secondary" :disabled="!createDraft.password" @click="copyText(createDraft.password)">复制</button>
            </div>
          </div>
        </div>
        <div class="form-row compact" style="justify-content: space-between; margin-top: 8px;">
          <label>
            <input type="checkbox" v-model="createDraft.mustChangePassword" /> 首次登录要求修改密码
          </label>
          <button type="submit" class="button" :disabled="saving">
            {{ saving ? "创建中..." : "创建账号" }}
          </button>
        </div>
      </form>

      <div v-if="feedbackMsg" class="cloud-banner" :class="feedbackError ? 'is-danger' : 'is-success'">
        {{ feedbackMsg }}
      </div>

      <!-- 账号列表 -->
      <div class="admin-table-wrap">
        <table class="table admin-table">
          <thead>
            <tr>
              <th>账号</th>
              <th>角色</th>
              <th>状态</th>
              <th>数据概览</th>
              <th>最近登录</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="user in users" :key="user.id">
              <td>
                <div class="cell-title">{{ user.username }}</div>
                <div v-if="user.nickname" class="cell-muted">{{ user.nickname }}</div>
              </td>
              <td>
                <span class="chip" :class="user.role === 'admin' ? 'is-recurring' : 'is-task'">
                  {{ user.role === "admin" ? "管理员" : "普通用户" }}
                </span>
              </td>
              <td>
                <span class="status-pill" :class="user.enabled ? 'is-running' : 'is-paused'">
                  {{ user.enabled ? "正常" : "已禁用" }}
                </span>
              </td>
              <td class="cell-muted">
                待办 {{ user.taskCount }} / 循环 {{ user.recurringCount }} / 记录 {{ user.recordCount }}
              </td>
              <td class="cell-time">{{ user.lastLoginAt ? formatDateTime(user.lastLoginAt) : "从未" }}</td>
              <td>
                <div class="admin-row-actions">
                  <button
                    type="button"
                    class="button secondary compact-btn"
                    :disabled="user.id === currentUserId || saving"
                    @click="handleToggleStatus(user)"
                  >
                    {{ user.enabled ? "禁用" : "启用" }}
                  </button>
                  <button
                    type="button"
                    class="button secondary compact-btn"
                    :disabled="saving"
                    @click="openResetPassword(user)"
                  >
                    重置密码
                  </button>
                  <button
                    type="button"
                    class="button danger compact-btn"
                    :disabled="user.id === currentUserId || saving"
                    @click="openDeleteConfirm(user)"
                  >
                    删除
                  </button>
                </div>
              </td>
            </tr>
            <tr v-if="!loading && users.length === 0" class="table-empty-row">
              <td colspan="6">
                <div class="table-empty">
                  <div class="table-empty-title">暂无账号数据</div>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 重置密码面板 -->
      <div v-if="resetTarget" class="admin-subcard">
        <div class="form-row compact" style="justify-content: space-between;">
          <strong>重置账号「{{ resetTarget.username }}」密码（提交后立即下线其全部设备）</strong>
          <button type="button" class="button secondary compact-btn" @click="resetTarget = null">取消</button>
        </div>
        <div class="form-row compact" style="margin-top: 8px;">
          <input class="input" v-model="resetPasswordValue" placeholder="输入或随机生成临时密码" style="flex: 1" />
          <button type="button" class="button secondary" @click="resetPasswordValue = generateStrongPassword(16)">随机</button>
          <button type="button" class="button secondary" :disabled="!resetPasswordValue" @click="copyText(resetPasswordValue)">复制</button>
          <button type="button" class="button" :disabled="saving || resetPasswordValue.length < 6" @click="handleResetPassword">
            确认重置
          </button>
        </div>
      </div>

      <!-- 删除账号二次确认面板 -->
      <div v-if="deleteTarget" class="admin-subcard is-danger-card">
        <div class="form-row compact" style="justify-content: space-between;">
          <strong>删除账号「{{ deleteTarget.username }}」将永久级联清除其全部待办、便签与提醒记录</strong>
          <button type="button" class="button secondary compact-btn" @click="deleteTarget = null">取消</button>
        </div>
        <div class="form-row compact" style="margin-top: 8px;">
          <input
            class="input"
            v-model="deleteConfirmInput"
            :placeholder="`请输入 ${deleteTarget.username} 确认删除`"
            style="flex: 1"
          />
          <button
            type="button"
            class="button danger"
            :disabled="saving || deleteConfirmInput.trim() !== deleteTarget.username"
            @click="handleDeleteUser"
          >
            永久删除账号
          </button>
        </div>
      </div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { copyText as copyToClipboard } from "../clipboard";
import { generateStrongPassword } from "../cloudMode";
import { errorMessage, formatDateTime } from "../format";
import { useCloud } from "../composables/useCloud";
import type { AdminUserSummary, UserRole } from "../types";

const { adminUsersOpen, cloudStatus } = useCloud();
const currentUserId = computed(() => cloudStatus.value?.userId || "");

const users = ref<AdminUserSummary[]>([]);
const loading = ref(false);
const saving = ref(false);
const feedbackMsg = ref("");
const feedbackError = ref(false);

const showCreateForm = ref(false);
const createDraft = reactive<{
  username: string;
  nickname: string;
  role: UserRole;
  password: string;
  mustChangePassword: boolean;
}>({
  username: "",
  nickname: "",
  role: "user",
  password: "",
  mustChangePassword: true
});

const resetTarget = ref<AdminUserSummary | null>(null);
const resetPasswordValue = ref("");

const deleteTarget = ref<AdminUserSummary | null>(null);
const deleteConfirmInput = ref("");

const setFeedback = (msg: string, isError = false) => {
  feedbackMsg.value = msg;
  feedbackError.value = isError;
};

const loadUsers = async () => {
  loading.value = true;
  try {
    users.value = await api.adminListUsers();
  } catch (error) {
    setFeedback(`加载账号列表失败：${errorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
};

watch(adminUsersOpen, open => {
  if (open) {
    feedbackMsg.value = "";
    resetTarget.value = null;
    deleteTarget.value = null;
    void loadUsers();
  }
});

const toggleCreateForm = () => {
  showCreateForm.value = !showCreateForm.value;
  if (showCreateForm.value && !createDraft.password) {
    createDraft.password = generateStrongPassword(16);
  }
};

const generateCreatePassword = () => {
  createDraft.password = generateStrongPassword(16);
};

const copyText = async (text: string) => {
  const ok = await copyToClipboard(text);
  setFeedback(ok ? "已复制密码到剪贴板" : "复制失败，请手动复制", !ok);
};

const handleCreateUser = async () => {
  saving.value = true;
  feedbackMsg.value = "";
  try {
    const created = await api.adminCreateUser({
      username: createDraft.username.trim(),
      nickname: createDraft.nickname.trim(),
      role: createDraft.role,
      password: createDraft.password,
      mustChangePassword: createDraft.mustChangePassword
    });
    setFeedback(`已创建账号「${created.username}」，请将初始密码安全告知对方`);
    createDraft.username = "";
    createDraft.nickname = "";
    createDraft.role = "user";
    createDraft.password = generateStrongPassword(16);
    showCreateForm.value = false;
    await loadUsers();
  } catch (error) {
    setFeedback(errorMessage(error), true);
  } finally {
    saving.value = false;
  }
};

const handleToggleStatus = async (user: AdminUserSummary) => {
  saving.value = true;
  feedbackMsg.value = "";
  try {
    await api.adminUpdateUser({ userId: user.id, enabled: !user.enabled });
    setFeedback(`已${user.enabled ? "禁用" : "启用"}账号「${user.username}」`);
    await loadUsers();
  } catch (error) {
    setFeedback(errorMessage(error), true);
  } finally {
    saving.value = false;
  }
};

const openResetPassword = (user: AdminUserSummary) => {
  deleteTarget.value = null;
  resetTarget.value = user;
  resetPasswordValue.value = generateStrongPassword(16);
};

const handleResetPassword = async () => {
  if (!resetTarget.value) return;
  saving.value = true;
  feedbackMsg.value = "";
  try {
    await api.adminResetPassword({
      userId: resetTarget.value.id,
      newPassword: resetPasswordValue.value,
      mustChangePassword: true
    });
    setFeedback(`已重置「${resetTarget.value.username}」的密码并强制下线全部设备`);
    resetTarget.value = null;
    await loadUsers();
  } catch (error) {
    setFeedback(errorMessage(error), true);
  } finally {
    saving.value = false;
  }
};

const openDeleteConfirm = (user: AdminUserSummary) => {
  resetTarget.value = null;
  deleteTarget.value = user;
  deleteConfirmInput.value = "";
};

const handleDeleteUser = async () => {
  if (!deleteTarget.value) return;
  saving.value = true;
  feedbackMsg.value = "";
  try {
    const uname = deleteTarget.value.username;
    await api.adminDeleteUser(deleteTarget.value.id);
    setFeedback(`已永久删除账号「${uname}」及其全部云端数据`);
    deleteTarget.value = null;
    await loadUsers();
  } catch (error) {
    setFeedback(errorMessage(error), true);
  } finally {
    saving.value = false;
  }
};
</script>
