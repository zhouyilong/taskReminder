<template>
  <Modal :open="dataOpen" title="导入导出与备份" @close="dataOpen = false" @confirm="dataOpen = false">
    <div class="modal-section">
      <div class="data-section-title">导出</div>
      <div class="form-row compact">
        <button class="button secondary" type="button" :disabled="busy" @click="handleExport('json')">JSON 完整备份</button>
        <button class="button secondary" type="button" :disabled="busy" @click="handleExport('markdown')">Markdown 待办清单</button>
        <button class="button secondary" type="button" :disabled="busy" @click="handleExport('ics')">日历文件（ICS）</button>
      </div>
      <div class="field-hint">
        JSON 包含待办、循环提醒与提醒记录，可在其他设备导入；ICS 可导入 Outlook、系统日历等，区间间隔与 Cron 循环提醒不会导出。
      </div>
    </div>
    <div class="modal-section">
      <div class="data-section-title">导入</div>
      <div class="form-row compact">
        <button class="button secondary" type="button" :disabled="busy" @click="handleImport">从 JSON 备份导入…</button>
      </div>
      <div class="field-hint">按条目合并，较新的版本胜出：不会覆盖本地更新的修改。导入前会自动保存一份本地备份。</div>
    </div>
    <div class="modal-section">
      <div class="data-section-title">
        本地自动备份
        <span class="tag">每天一份，保留最近 {{ BACKUP_KEEP }} 份</span>
      </div>
      <div class="form-row compact">
        <button class="button secondary" type="button" :disabled="busy" @click="handleBackupNow">立即备份</button>
        <span class="field-hint data-backup-dir" :title="backupDir">{{ backupDir || "-" }}</span>
      </div>
      <div class="data-backup-list">
        <div v-for="item in backups" :key="item.name" class="data-backup-item">
          <span class="data-backup-time">{{ formatDateTime(item.createdAt) }}</span>
          <span class="data-backup-size">{{ formatBytes(item.size) }}</span>
          <button class="button secondary" type="button" :disabled="busy" @click="confirmRestore(item)">合并恢复</button>
        </div>
        <div v-if="!backups.length" class="field-hint">还没有备份，应用启动约 1 分钟后会自动创建当天的备份。</div>
      </div>
    </div>
    <div v-if="message" class="modal-section">
      <div class="field-hint" :class="{ 'is-error': messageIsError }">{{ message }}</div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { errorMessage, formatBytes, formatDateTime } from "../format";
import type { BackupInfo } from "../types";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";
import { useSettings } from "../composables/useSettings";

const BACKUP_KEEP = 7;

const { dataOpen } = useSettings();
const { refreshAll } = useAppData();
const { confirmAction } = useDialogs();

const busy = ref(false);
const message = ref("");
const messageIsError = ref(false);
const backupDir = ref("");
const backups = ref<BackupInfo[]>([]);

const showMessage = (text: string, isError = false) => {
  message.value = text;
  messageIsError.value = isError;
};

const loadBackups = async () => {
  try {
    const payload = await api.listBackups();
    backupDir.value = payload.dir;
    backups.value = payload.backups;
  } catch (error) {
    showMessage(`读取备份列表失败：${errorMessage(error)}`, true);
  }
};

watch(dataOpen, open => {
  if (open) {
    message.value = "";
    void loadBackups();
  }
});

const run = async (action: () => Promise<void>) => {
  if (busy.value) {
    return;
  }
  busy.value = true;
  try {
    await action();
  } catch (error) {
    showMessage(errorMessage(error), true);
  } finally {
    busy.value = false;
  }
};

const handleExport = (format: "json" | "markdown" | "ics") =>
  run(async () => {
    const result = await api.exportData(format);
    if (!result) {
      return;
    }
    const skipped = result.skipped ? `（${result.skipped} 个循环提醒无法用日历规则表达，未导出）` : "";
    showMessage(`已导出到 ${result.path}${skipped}`);
  });

const handleImport = () =>
  run(async () => {
    const summary = await api.importData();
    if (!summary) {
      return;
    }
    showMessage(`导入完成：新增 ${summary.inserted} 条，更新 ${summary.updated} 条，跳过 ${summary.skipped} 条`);
    await refreshAll();
    await loadBackups();
  });

const handleBackupNow = () =>
  run(async () => {
    await api.createBackupNow();
    await loadBackups();
    showMessage("已创建备份");
  });

const confirmRestore = (item: BackupInfo) => {
  confirmAction({
    title: "从备份恢复",
    message: `把 ${formatDateTime(item.createdAt)} 的备份合并回当前数据？只会找回缺失的条目与备份中更新的版本，之后的修改不受影响。`,
    action: () =>
      run(async () => {
        await api.restoreBackup(item.name);
        await refreshAll();
        showMessage("已从备份合并恢复");
      }),
  });
};
</script>

<style scoped>
.data-section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  color: var(--text-title);
  font-size: var(--font-body);
  font-weight: var(--weight-bold);
}

.data-backup-dir {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: text;
}

.data-backup-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 180px;
  overflow-y: auto;
}

.data-backup-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 4px 8px;
  border-radius: var(--radius-xs);
  background: var(--bg-muted);
  font-size: var(--font-meta);
}

.data-backup-time {
  flex: 1;
  color: var(--text-base);
  font-variant-numeric: tabular-nums;
}

.data-backup-size {
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.data-backup-item .button {
  height: 26px;
  padding: 0 10px;
}

.field-hint.is-error {
  color: var(--danger-text);
}
</style>
