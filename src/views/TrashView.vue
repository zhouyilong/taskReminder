<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">回收站</div>
      <span class="section-meta">共 {{ items.length }} 项</span>
    </div>
    <div class="form-row compact filter-bar">
      <div class="search-field trash-search">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="6.5" />
          <path d="M16 16l4 4" />
        </svg>
        <input class="input" v-model="keyword" placeholder="按标题搜索已删除的内容" />
        <button v-if="keyword" class="search-clear" type="button" title="清空" @click="keyword = ''">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M7 7l10 10M17 7L7 17" />
          </svg>
        </button>
      </div>
      <span class="field-hint">删除的内容保留 {{ retentionDays }} 天{{ syncHint }}，到期后自动永久删除</span>
      <button class="button danger" :disabled="!items.length || busy" @click="confirmEmptyTrash">清空回收站</button>
    </div>
    <div class="table-card">
      <div class="table-scroll table-scroll-no-x">
        <table class="table trash-table">
          <thead>
            <tr>
              <th class="col-type">类型</th>
              <th class="col-desc">标题</th>
              <th class="col-datetime">删除时间</th>
              <th class="col-remaining">剩余</th>
              <th class="col-trash-actions">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in trashPage" :key="item.key" class="table-row" @contextmenu.prevent.stop="openItemMenu($event, item)">
              <td class="col-type">
                <span class="chip" :class="item.kind === 'task' ? 'is-task' : 'is-recurring'">{{ item.typeLabel }}</span>
              </td>
              <td class="col-desc cell-title" :title="item.title">{{ item.title }}</td>
              <td class="col-datetime cell-time" :title="formatDateTime(item.deletedAt)">{{ formatDateTime(item.deletedAt) }}</td>
              <td class="col-remaining cell-muted">{{ item.remainingDays }} 天</td>
              <td class="col-trash-actions">
                <div class="row-actions">
                  <button class="button secondary" type="button" :disabled="busy" @click="restore(item)">恢复</button>
                  <button class="button ghost is-danger" type="button" :disabled="busy" @click="confirmPurge([item])">永久删除</button>
                </div>
              </td>
            </tr>
            <tr v-if="!trashPage.length" class="table-empty-row">
              <td colspan="5">
                <div class="table-empty">
                  <span class="table-empty-title">{{ keyword ? "没有匹配的内容" : "回收站是空的" }}</span>
                  <span class="table-empty-hint">{{ keyword ? "换个关键词试试" : "删除的待办、便签和循环提醒会在这里保留一段时间" }}</span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <Pagination :total="filtered.length" :total-pages="totalPages" v-model:page-index="pageIndex" v-model:page-size="pageSize" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Pagination from "../components/Pagination.vue";
import { api } from "../api";
import { errorMessage, formatDateTime } from "../format";
import type { RecurringTask, Task } from "../types";
import { useAppData } from "../composables/useAppData";
import { useContextMenu } from "../composables/useContextMenu";
import { useDialogs } from "../composables/useDialogs";
import { usePagination } from "../composables/usePagination";
import { useSettings } from "../composables/useSettings";

type TrashItem = {
  key: string;
  id: string;
  kind: "task" | "recurring";
  typeLabel: string;
  title: string;
  deletedAt: string;
  remainingDays: number;
};

const DAY_MS = 24 * 60 * 60 * 1000;

const { dataVersion, refreshAll } = useAppData();
const { showContextMenu } = useContextMenu();
const { confirmAction } = useDialogs();
const { settingsDraft } = useSettings();

const deletedTasks = ref<Task[]>([]);
const deletedRecurring = ref<RecurringTask[]>([]);
const retentionDays = ref(7);
const keyword = ref("");
const busy = ref(false);

const syncHint = computed(() => (settingsDraft.webdavEnabled ? "（已开启云同步）" : ""));

const remainingDays = (deletedAt: string) => {
  const deleted = new Date(deletedAt).getTime();
  if (Number.isNaN(deleted)) {
    return retentionDays.value;
  }
  const left = Math.ceil((deleted + retentionDays.value * DAY_MS - Date.now()) / DAY_MS);
  return Math.max(0, Math.min(retentionDays.value, left));
};

const items = computed<TrashItem[]>(() => {
  const list: TrashItem[] = [
    ...deletedTasks.value.map(task => ({
      key: `task-${task.id}`,
      id: task.id,
      kind: "task" as const,
      typeLabel: task.status === "COMPLETED" ? "已办" : "待办",
      title: task.description || "（无标题）",
      deletedAt: task.deletedAt ?? "",
      remainingDays: remainingDays(task.deletedAt ?? ""),
    })),
    ...deletedRecurring.value.map(task => ({
      key: `recurring-${task.id}`,
      id: task.id,
      kind: "recurring" as const,
      typeLabel: "循环",
      title: task.description || "（无标题）",
      deletedAt: task.deletedAt ?? "",
      remainingDays: remainingDays(task.deletedAt ?? ""),
    })),
  ];
  return list.sort((a, b) => b.deletedAt.localeCompare(a.deletedAt));
});

const filtered = computed(() => {
  const value = keyword.value.trim().toLowerCase();
  return value ? items.value.filter(item => item.title.toLowerCase().includes(value)) : items.value;
});

const { pageIndex, pageSize, totalPages, page: trashPage } = usePagination(filtered, keyword);

const loadTrash = async () => {
  try {
    const trash = await api.listTrash();
    deletedTasks.value = trash.tasks;
    deletedRecurring.value = trash.recurringTasks;
    retentionDays.value = trash.retentionDays;
  } catch (error) {
    console.error("[trash] 读取回收站失败", error);
  }
};

watch(dataVersion, loadTrash, { immediate: true });

const withBusy = async (action: () => Promise<void>) => {
  busy.value = true;
  try {
    await action();
  } catch (error) {
    alert(`操作失败：${errorMessage(error)}`);
  } finally {
    busy.value = false;
    // refreshAll 会递增 dataVersion，从而重新读取回收站。
    await refreshAll();
  }
};

const restore = (item: TrashItem) =>
  withBusy(async () => {
    if (item.kind === "task") {
      await api.restoreTask(item.id);
    } else {
      await api.restoreRecurringTask(item.id);
    }
  });

const purge = (targets: TrashItem[]) =>
  withBusy(async () => {
    await api.purgeTrash({
      taskIds: targets.filter(item => item.kind === "task").map(item => item.id),
      recurringIds: targets.filter(item => item.kind === "recurring").map(item => item.id),
    });
  });

const confirmPurge = (targets: TrashItem[]) => {
  confirmAction({
    title: "永久删除",
    message:
      targets.length === 1
        ? `确定要永久删除“${targets[0].title}”吗？此操作无法撤销。`
        : `确定要永久删除这 ${targets.length} 项吗？此操作无法撤销。`,
    action: () => purge(targets),
  });
};

const confirmEmptyTrash = () => {
  if (items.value.length) {
    confirmPurge([...items.value]);
  }
};

const openItemMenu = (event: MouseEvent, item: TrashItem) => {
  showContextMenu(event, [
    { label: "恢复", action: () => void restore(item) },
    { label: "永久删除", action: () => confirmPurge([item]), danger: true },
  ]);
};
</script>
