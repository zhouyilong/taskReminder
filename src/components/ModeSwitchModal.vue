<template>
  <Modal
    :open="modeSwitchOpen"
    :title="modalTitle"
    hide-footer
    @close="closeModal"
  >
    <!-- 本地 → 云端 -->
    <template v-if="modeSwitchTarget === 'cloud' && cloudPreview">
      <div class="modal-section">
        <p class="mode-switch-lead">
          即将切换到云端账号 <strong>{{ accountDisplay }}</strong>。检测到本机存在
          <strong>{{ formatBusinessCountsSummary(cloudPreview.localCounts) }}</strong>。
        </p>

        <div v-if="cloudPreview.crossAccountWarning" class="cloud-banner is-warning">
          <strong>防串号提示：</strong>本机数据曾与账号
          「{{ cloudPreview.lastMergedUsername || cloudPreview.lastMergedAccountId }}」合并过，
          合并到当前账号可能导致数据混入，请谨慎确认（已默认选择“直接切换”）。
        </div>

        <div class="mode-option-list" role="radiogroup" aria-label="本地数据处理方式">
          <label class="mode-option" :class="{ active: mergeChoice === true }">
            <input type="radio" :value="true" v-model="mergeChoice" />
            <div class="mode-option-body">
              <div class="mode-option-title">
                合并本地数据到云端{{ cloudPreview.crossAccountWarning ? "" : "（推荐）" }}
              </div>
              <div class="mode-option-desc">
                先自动备份本地库，再按较新版本胜出（LWW）规则将本地数据合并进云端；合并后原本地库数据保持不动。
              </div>
            </div>
          </label>
          <label class="mode-option" :class="{ active: mergeChoice === false }">
            <input type="radio" :value="false" v-model="mergeChoice" />
            <div class="mode-option-body">
              <div class="mode-option-title">
                直接切换（不合并）{{ cloudPreview.crossAccountWarning ? "（推荐）" : "" }}
              </div>
              <div class="mode-option-desc">
                保持本机数据原样不动，直接加载该云端账号下的数据。
              </div>
            </div>
          </label>
        </div>

        <div v-if="modeSwitchError" class="cloud-banner is-danger">{{ modeSwitchError }}</div>

        <div class="modal-footer-inline">
          <button type="button" class="button secondary" :disabled="modeSwitching" @click="closeModal">
            取消
          </button>
          <button type="button" class="button" :disabled="modeSwitching" @click="handleConfirmCloud">
            {{ modeSwitching ? "正在切换..." : "确认切换至云端模式" }}
          </button>
        </div>
      </div>
    </template>

    <!-- 云端 → 本地 -->
    <template v-else-if="modeSwitchTarget === 'local' && localPreview">
      <div class="modal-section">
        <div v-if="!localPreview.isOnline" class="cloud-banner is-warning">
          <strong>当前无法连接云端服务</strong>
          <span v-if="localPreview.pendingCount > 0">
            （尚有 {{ localPreview.pendingCount }} 条本地变更未推送）
          </span>
          。本机账号镜像库仍完整保存了断网前后的最新数据，你可以将其合并到本地继续使用，待恢复联网后再切回云端合并。
        </div>
        <p v-else class="mode-switch-lead">
          当前云端数据共 <strong>{{ formatBusinessCountsSummary(localPreview.cloudCounts) }}</strong>，
          原本地库现有 <strong>{{ formatBusinessCountsSummary(localPreview.localCounts) }}</strong>。
          切回本地模式默认保留当前登录态，方便随时切回。
        </p>

        <div class="mode-option-list" role="radiogroup" aria-label="云端数据处理方式">
          <label class="mode-option" :class="{ active: mergeChoice === true }">
            <input type="radio" :value="true" v-model="mergeChoice" />
            <div class="mode-option-body">
              <div class="mode-option-title">
                {{ localPreview.isOnline ? "合并云端数据到本地（推荐）" : "合并当前云端缓存到本地（推荐）" }}
              </div>
              <div class="mode-option-desc">
                先自动备份原本地库，再将当前云端镜像库中的最新数据按较新胜出（LWW）规则合入本地库后切换。
              </div>
            </div>
          </label>
          <label class="mode-option" :class="{ active: mergeChoice === false }">
            <input type="radio" :value="false" v-model="mergeChoice" />
            <div class="mode-option-body">
              <div class="mode-option-title">
                {{ localPreview.isOnline ? "仅切换至本地（不合并）" : "直接切换至原本地数据" }}
              </div>
              <div class="mode-option-desc">
                不把云端账号数据写入本地库，直接使用原本地数据（适合公用设备保护隐私）。
              </div>
            </div>
          </label>
        </div>

        <div v-if="modeSwitchError" class="cloud-banner is-danger">{{ modeSwitchError }}</div>

        <div class="modal-footer-inline">
          <button type="button" class="button secondary" :disabled="modeSwitching" @click="closeModal">
            取消
          </button>
          <button type="button" class="button" :disabled="modeSwitching" @click="handleConfirmLocal">
            {{ modeSwitching ? "正在切换..." : "确认切换至本地模式" }}
          </button>
        </div>
      </div>
    </template>
  </Modal>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Modal from "./Modal.vue";
import { formatBusinessCountsSummary } from "../cloudMode";
import { useCloud } from "../composables/useCloud";

const {
  modeSwitchOpen,
  modeSwitchTarget,
  cloudPreview,
  localPreview,
  modeSwitching,
  modeSwitchError,
  confirmSwitchToCloud,
  confirmSwitchToLocal
} = useCloud();

const mergeChoice = ref(true);

const modalTitle = computed(() =>
  modeSwitchTarget.value === "cloud" ? "切换至云端模式" : "切换至本地模式"
);

const accountDisplay = computed(() => {
  const p = cloudPreview.value;
  if (!p) return "";
  return p.nickname ? `${p.nickname} (${p.username})` : p.username;
});

watch(
  [modeSwitchOpen, modeSwitchTarget, cloudPreview],
  ([open, target, preview]) => {
    if (!open) return;
    if (target === "cloud" && preview?.crossAccountWarning) {
      mergeChoice.value = false;
    } else {
      mergeChoice.value = true;
    }
  },
  { immediate: true }
);

const closeModal = () => {
  if (modeSwitching.value) return;
  modeSwitchOpen.value = false;
};

const handleConfirmCloud = async () => {
  try {
    await confirmSwitchToCloud(mergeChoice.value);
  } catch {
    // 错误已展示在 modeSwitchError
  }
};

const handleConfirmLocal = async () => {
  try {
    await confirmSwitchToLocal(mergeChoice.value);
  } catch {
    // 错误已展示在 modeSwitchError
  }
};
</script>
