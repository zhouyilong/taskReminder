// 云端模式状态、登录拦截、模式切换引导与管理员弹窗（模块级单例）。
import { computed, ref } from "vue";
import { api } from "../api";
import { buildModeBadge, totalBusinessCount } from "../cloudMode";
import type {
  CloudStatus,
  SwitchToCloudPreview,
  SwitchToLocalPreview
} from "../types";
import { useAppData } from "./useAppData";
import { useSettings } from "./useSettings";

const cloudStatus = ref<CloudStatus | null>(null);
const cloudSyncing = ref(false);

// 登录弹窗 / 阻断式登录界面
const authModalOpen = ref(false);
const pendingSwitchAfterLogin = ref(false);

// 模式切换引导弹窗
const modeSwitchOpen = ref(false);
const modeSwitchTarget = ref<"cloud" | "local">("cloud");
const cloudPreview = ref<SwitchToCloudPreview | null>(null);
const localPreview = ref<SwitchToLocalPreview | null>(null);
const modeSwitching = ref(false);
const modeSwitchError = ref("");

// 管理员账号管理弹窗与个人资料/改密弹窗
const adminUsersOpen = ref(false);
const profileModalOpen = ref(false);

const isCloudMode = computed(() => cloudStatus.value?.mode === "cloud");
const isAdmin = computed(
  () => Boolean(cloudStatus.value?.loggedIn) && cloudStatus.value?.role === "admin"
);
const modeBadge = computed(() => buildModeBadge(cloudStatus.value));

// 处于云端模式且未登录、认证失效或首次登录需改密时，展示阻断式界面
const isAuthBlocking = computed(() => {
  const s = cloudStatus.value;
  if (!s || s.mode !== "cloud") return false;
  return !s.loggedIn || s.state === "auth_required" || s.mustChangePassword;
});

const refreshCloudStatus = async () => {
  try {
    cloudStatus.value = await api.getCloudStatus();
  } catch (error) {
    console.error("[cloud] 获取云端状态失败", error);
  }
};

const syncCloudNow = async () => {
  if (cloudSyncing.value || !isCloudMode.value) return;
  cloudSyncing.value = true;
  try {
    cloudStatus.value = await api.cloudSyncNow();
    await useAppData().refreshAll();
  } finally {
    cloudSyncing.value = false;
  }
};

const openLoginModal = (forSwitch = false) => {
  pendingSwitchAfterLogin.value = forSwitch;
  authModalOpen.value = true;
};

const closeLoginModal = () => {
  authModalOpen.value = false;
  pendingSwitchAfterLogin.value = false;
};

/** 发起「本地模式 → 云端模式」流程：先检测登录态，未登录先弹登录，再按本地数据条数决定直接切换或弹合并确认。 */
const startSwitchToCloud = async () => {
  modeSwitchError.value = "";
  const preview = await api.previewSwitchToCloud();
  cloudPreview.value = preview;
  if (!preview.loggedIn) {
    openLoginModal(true);
    return;
  }
  // 本地无业务数据时直接切换至云端模式
  if (totalBusinessCount(preview.localCounts) === 0) {
    modeSwitching.value = true;
    try {
      const result = await api.switchToCloud(false);
      cloudStatus.value = result.status;
      await useSettings().loadSettings();
      await useAppData().refreshAll();
    } finally {
      modeSwitching.value = false;
    }
    return;
  }
  modeSwitchTarget.value = "cloud";
  modeSwitchOpen.value = true;
};

/** 登录成功后若是由切换云端触发的，自动衔接后续合并确认流程。 */
const handleLoginCompleted = async (status: CloudStatus) => {
  cloudStatus.value = status;
  await useSettings().loadSettings();
  const shouldContinueSwitch = pendingSwitchAfterLogin.value;
  closeLoginModal();
  if (shouldContinueSwitch && status.mode === "local") {
    await startSwitchToCloud();
  } else if (status.mode === "cloud") {
    await useAppData().refreshAll();
  }
};

const confirmSwitchToCloud = async (mergeLocal: boolean) => {
  modeSwitching.value = true;
  modeSwitchError.value = "";
  try {
    const result = await api.switchToCloud(mergeLocal);
    cloudStatus.value = result.status;
    modeSwitchOpen.value = false;
    await useSettings().loadSettings();
    await useAppData().refreshAll();
    return result;
  } catch (error) {
    modeSwitchError.value = error instanceof Error ? error.message : String(error);
    throw error;
  } finally {
    modeSwitching.value = false;
  }
};

/** 发起「云端模式 → 本地模式」流程。 */
const startSwitchToLocal = async () => {
  modeSwitchError.value = "";
  localPreview.value = await api.previewSwitchToLocal();
  modeSwitchTarget.value = "local";
  modeSwitchOpen.value = true;
};

const confirmSwitchToLocal = async (mergeCloud: boolean) => {
  modeSwitching.value = true;
  modeSwitchError.value = "";
  try {
    const result = await api.switchToLocal(mergeCloud);
    cloudStatus.value = result.status;
    modeSwitchOpen.value = false;
    await useSettings().loadSettings();
    await useAppData().refreshAll();
    return result;
  } catch (error) {
    modeSwitchError.value = error instanceof Error ? error.message : String(error);
    throw error;
  } finally {
    modeSwitching.value = false;
  }
};

/** 逃生通道：在登录阻断页一键切回原本地数据（不合并）。 */
const escapeToLocalMode = async () => {
  modeSwitching.value = true;
  try {
    const result = await api.switchToLocal(false);
    cloudStatus.value = result.status;
    closeLoginModal();
    modeSwitchOpen.value = false;
    await useSettings().loadSettings();
    await useAppData().refreshAll();
  } finally {
    modeSwitching.value = false;
  }
};

const logoutCloud = async () => {
  const status = await api.cloudLogout();
  cloudStatus.value = status;
  await useSettings().loadSettings();
  await useAppData().refreshAll();
};

const openAdminUsers = () => {
  useSettings().settingsOpen.value = false;
  adminUsersOpen.value = true;
};

const openProfileModal = () => {
  profileModalOpen.value = true;
};

export const useCloud = () => ({
  cloudStatus,
  cloudSyncing,
  isCloudMode,
  isAdmin,
  modeBadge,
  isAuthBlocking,
  authModalOpen,
  modeSwitchOpen,
  modeSwitchTarget,
  cloudPreview,
  localPreview,
  modeSwitching,
  modeSwitchError,
  adminUsersOpen,
  profileModalOpen,
  refreshCloudStatus,
  syncCloudNow,
  openLoginModal,
  closeLoginModal,
  handleLoginCompleted,
  startSwitchToCloud,
  confirmSwitchToCloud,
  startSwitchToLocal,
  confirmSwitchToLocal,
  escapeToLocalMode,
  logoutCloud,
  openAdminUsers,
  openProfileModal
});
