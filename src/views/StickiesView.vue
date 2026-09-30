<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">便签</div>
      <span class="section-meta">显示中 {{ counts.showing }} · 共 {{ notes.length }} 张</span>
      <div class="segmented" role="radiogroup" aria-label="便签筛选">
        <button
          v-for="option in STICKY_FILTER_OPTIONS"
          :key="option.value"
          class="segmented-item"
          :class="{ active: filter === option.value }"
          type="button"
          role="radio"
          :aria-checked="filter === option.value"
          @click="filter = option.value"
        >
          {{ option.label }}
        </button>
      </div>
    </div>
    <div class="form-row compact filter-bar">
      <div class="search-field sticky-search">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="6.5" />
          <path d="M16 16l4 4" />
        </svg>
        <input class="input" v-model="keyword" placeholder="搜索便签标题或内容" />
        <button v-if="keyword" class="search-clear" type="button" title="清空" @click="keyword = ''">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M7 7l10 10M17 7L7 17" />
          </svg>
        </button>
      </div>
      <button class="button" type="button" :disabled="busy" @click="createNote">新建便签</button>
    </div>
    <div class="table-card">
      <div class="table-scroll table-scroll-no-x">
        <table class="table sticky-table">
          <thead>
            <tr>
              <th class="col-sticky-state">状态</th>
              <th class="col-desc">标题</th>
              <th class="col-note">内容</th>
              <th class="col-sticky-reminder">提醒</th>
              <th class="col-datetime">修改时间</th>
              <th class="col-sticky-actions">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="item in stickyPage"
              :key="item.note.taskId"
              class="table-row"
              @contextmenu.prevent.stop="openItemMenu($event, item)"
            >
              <td class="col-sticky-state">
                <span class="status-pill" :class="`sticky-${item.state}`">{{ STICKY_STATE_LABELS[item.state] }}</span>
              </td>
              <td class="col-desc cell-title" :title="item.note.title">
                <button class="sticky-title-link" type="button" :disabled="busy" @click="bringToFront(item)">
                  {{ item.note.title || "（无标题）" }}
                </button>
              </td>
              <td class="col-note cell-muted" :class="{ 'cell-empty': !item.preview }" :title="item.preview">
                {{ item.preview || "-" }}
              </td>
              <td class="col-sticky-reminder">
                <span v-if="item.note.reminderTime" class="time-chip" :class="reminderTone(item.note.reminderTime)">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <circle cx="12" cy="12" r="8" />
                    <path d="M12 8v4l2.5 1.5" />
                  </svg>
                  {{ formatDateTime(item.note.reminderTime).slice(5, 16) }}
                </span>
                <span v-else class="cell-empty">-</span>
              </td>
              <td class="col-datetime cell-time" :title="formatDateTime(item.note.updatedAt)">
                {{ formatDateTime(item.note.updatedAt).slice(5, 16) }}
              </td>
              <td class="col-sticky-actions">
                <div class="row-actions">
                  <button class="button secondary" type="button" :disabled="busy" @click="bringToFront(item)">
                    {{ primaryActionLabel(item) }}
                  </button>
                  <button
                    v-if="item.state !== 'closed'"
                    class="button ghost"
                    type="button"
                    :disabled="busy"
                    @click="closeNote(item)"
                  >
                    关闭
                  </button>
                </div>
              </td>
            </tr>
            <tr v-if="!stickyPage.length" class="table-empty-row">
              <td colspan="6">
                <div class="table-empty">
                  <span class="table-empty-title">{{ emptyTitle }}</span>
                  <span class="table-empty-hint">{{ emptyHint }}</span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <Pagination :total="list.length" :total-pages="totalPages" v-model:page-index="pageIndex" v-model:page-size="pageSize" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import Pagination from "../components/Pagination.vue";
import { api } from "../api";
import { errorMessage, formatDateTime, reminderTone } from "../format";
import {
  STICKY_FILTER_OPTIONS,
  STICKY_STATE_LABELS,
  buildStickyList,
  countStickyStates,
  type StickyFilter,
  type StickyListItem,
} from "../stickies";
import type { StickyNoteSummary } from "../types";
import { useAppData } from "../composables/useAppData";
import { useContextMenu } from "../composables/useContextMenu";
import { useDialogs } from "../composables/useDialogs";
import { usePagination } from "../composables/usePagination";

const { tasks, dataVersion, refreshAll } = useAppData();
const { showContextMenu } = useContextMenu();
const { confirmAction, openTaskEditor } = useDialogs();

const notes = ref<StickyNoteSummary[]>([]);
const keyword = ref("");
const filter = ref<StickyFilter>("all");
const busy = ref(false);

const counts = computed(() => countStickyStates(notes.value));
const list = computed(() => buildStickyList(notes.value, keyword.value, filter.value));
const listKey = computed(() => `${keyword.value}|${filter.value}`);
const { pageIndex, pageSize, totalPages, page: stickyPage } = usePagination(list, listKey);

const emptyTitle = computed(() => {
  if (keyword.value.trim()) {
    return "没有匹配的便签";
  }
  return notes.value.length ? "没有符合筛选的便签" : "还没有便签";
});
const emptyHint = computed(() =>
  keyword.value.trim() || notes.value.length
    ? "换个关键词或筛选试试"
    : "点击“新建便签”，或在待办上右键“打开便签卡片”",
);

const primaryActionLabel = (item: StickyListItem) =>
  item.state === "showing" ? "置前" : item.state === "hidden" ? "显示" : "打开";

const loadNotes = async () => {
  try {
    notes.value = await api.listStickyNoteSummaries();
  } catch (error) {
    console.error("[stickies] 读取便签列表失败", error);
  }
};

watch(dataVersion, loadNotes, { immediate: true });

const withBusy = async (action: () => Promise<void>) => {
  busy.value = true;
  try {
    await action();
  } catch (error) {
    alert(`操作失败：${errorMessage(error)}`);
  } finally {
    busy.value = false;
    await loadNotes();
  }
};

/** 已打开的便签只显示并置前（不写库）；已关闭的重新打开。 */
const bringToFront = (item: StickyListItem) =>
  withBusy(async () => {
    if (item.state === "closed") {
      await api.openStickyNote({ taskId: item.note.taskId });
    } else {
      await api.showStickyNote(item.note.taskId);
    }
  });

const closeNote = (item: StickyListItem) => withBusy(() => api.closeStickyNote(item.note.taskId));

const createNote = () =>
  withBusy(async () => {
    await api.createStickyNote({});
    await refreshAll();
  });

// 便签窗口里的编辑可能还没保存，只允许清空已关闭的便签；重新打开时窗口会载入清空后的内容。
const confirmClearContent = (item: StickyListItem) => {
  confirmAction({
    title: "清空便签内容",
    message: `确定要清空“${item.note.title || "（无标题）"}”的便签内容吗？待办本身会保留。`,
    action: () =>
      withBusy(async () => {
        await api.saveStickyNoteContent({ taskId: item.note.taskId, content: "" });
        await refreshAll();
      }),
  });
};

const confirmDelete = (item: StickyListItem) => {
  confirmAction({
    message: `确定要删除“${item.note.title || "（无标题）"}”吗？便签与所属待办会一起进入回收站。`,
    action: () =>
      withBusy(async () => {
        if (item.state !== "closed") {
          await api.closeStickyNote(item.note.taskId);
        }
        await api.deleteTask(item.note.taskId);
        await refreshAll();
      }),
  });
};

const openItemMenu = (event: MouseEvent, item: StickyListItem) => {
  const task = tasks.value.find(candidate => candidate.id === item.note.taskId);
  showContextMenu(event, [
    { label: primaryActionLabel(item), action: () => void bringToFront(item) },
    ...(item.state !== "closed" ? [{ label: "关闭", action: () => void closeNote(item) }] : []),
    ...(task ? [{ label: "编辑待办", action: () => openTaskEditor(task) }] : []),
    ...(item.state === "closed" && item.note.content.trim()
      ? [{ label: "清空内容", action: () => confirmClearContent(item) }]
      : []),
    { label: "删除", action: () => confirmDelete(item), danger: true },
  ]);
};

// 便签窗口里关闭、托盘“隐藏/显示全部便签”都会发出该事件；切回主窗口时也刷新一次。
let unlistenStickyChanged: (() => void) | null = null;
const handleWindowFocus = () => void loadNotes();

onMounted(async () => {
  window.addEventListener("focus", handleWindowFocus);
  try {
    unlistenStickyChanged = await listen("sticky-note-changed", () => void loadNotes());
  } catch (error) {
    console.error("[stickies] 监听便签变更失败", error);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("focus", handleWindowFocus);
  unlistenStickyChanged?.();
});
</script>
