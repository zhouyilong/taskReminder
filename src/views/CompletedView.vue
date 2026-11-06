<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">已办事项</div>
      <span class="section-meta">筛选后 {{ filteredCompleted.length }} 条</span>
    </div>
    <div class="task-toolbar">
      <div class="search-field">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="6.5" />
          <path d="M16 16l4 4" />
        </svg>
        <input data-shortcut="search" class="input" v-model="completedFilter" placeholder="按标题、描述、#标签或 @项目搜索已办事项" />
        <button v-if="completedFilter" class="search-clear" type="button" title="清空" @click="completedFilter = ''">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M7 7l10 10M17 7L7 17" />
          </svg>
        </button>
      </div>
      <button
        class="button secondary"
        :class="{ 'is-active': selectionMode }"
        title="多选后批量删除（也可按住 Ctrl 点击行）"
        @click="selectionMode ? exitSelection() : (selectionMode = true)"
      >
        {{ selectionMode ? "退出多选" : "多选" }}
      </button>
    </div>
    <ListBatchBar
      v-if="selectionMode"
      :count="selectedCount"
      :total="filteredCompleted.length"
      unit="项"
      cancel-label="取消"
      @select-all="selectAllVisible"
      @cancel="exitSelection"
    >
      <button class="button danger" type="button" :disabled="!selectedCount" @click="deleteSelected">删除</button>
    </ListBatchBar>
    <div class="table-card">
      <div class="table-scroll table-scroll-no-x">
        <table class="table completed-table" :class="{ 'is-selecting': selectionMode }">
          <thead>
            <tr>
              <th class="col-select">
                <input
                  v-if="selectionMode"
                  type="checkbox"
                  title="全选本页"
                  aria-label="全选本页"
                  :checked="pageAllSelected"
                  :indeterminate.prop="pageSomeSelected && !pageAllSelected"
                  :disabled="!completedPage.length"
                  @change="togglePage"
                />
                <template v-else>取消完成</template>
              </th>
              <th class="col-desc">标题</th>
              <th class="col-note">描述</th>
              <th class="col-datetime">创建时间</th>
              <th class="col-datetime">完成时间</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="task in completedPage"
              :key="task.id"
              class="table-row"
              :class="{ 'is-selected': isSelected(task.id) }"
              @click="handleRowClick($event, task.id)"
              @dblclick="openTaskDetail(task)"
              @contextmenu.prevent.stop="openCompletedMenu($event, task)"
            >
              <td class="col-select">
                <input
                  v-if="selectionMode"
                  type="checkbox"
                  title="选择（Shift 连选）"
                  :checked="isSelected(task.id)"
                  @click.stop="toggleSelection($event, task.id)"
                />
                <input v-else type="checkbox" class="check-round" title="取消完成" checked @change="toggleTask(task)" />
              </td>
              <td class="col-desc cell-title is-done" :title="task.description">
                <div class="task-title-cell">
                  <span class="task-title-text">{{ task.description }}</span>
                  <TaskBadges :task="task" />
                </div>
              </td>
              <td class="col-note cell-muted" :class="{ 'cell-empty': !task.stickyContent?.trim() }" :title="taskStickyPreview(task.stickyContent)">{{ taskStickyPreview(task.stickyContent) }}</td>
              <td class="col-datetime cell-time" :title="formatDateTime(task.createdAt)">{{ formatDateTime(task.createdAt) }}</td>
              <td class="col-datetime cell-time" :title="formatDateTime(task.completedAt)">{{ formatDateTime(task.completedAt) }}</td>
            </tr>
            <tr v-if="!completedPage.length" class="table-empty-row">
              <td colspan="5">
                <div class="table-empty">
                  <span class="table-empty-title">{{ completedFilter ? "没有匹配的已办事项" : "暂无已办事项" }}</span>
                  <span class="table-empty-hint">{{ completedFilter ? "换个关键词试试" : "完成的任务会出现在这里" }}</span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <Pagination :total="filteredCompleted.length" :total-pages="completedTotalPages" v-model:page-index="completedPageIndex" v-model:page-size="completedPageSize" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { api } from "../api";
import ListBatchBar from "../components/ListBatchBar.vue";
import Pagination from "../components/Pagination.vue";
import TaskBadges from "../components/TaskBadges.vue";
import { formatDateTime, taskStickyPreview } from "../format";
import { markdownToPlainText } from "../markdown";
import { VIEW_SHORTCUT_EVENT, type ShortcutAction } from "../keyboard";
import { isEmptyQuery, matchesKeyword, parseQuery } from "../search";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";
import { useItemActions } from "../composables/useItemActions";
import { useListSelection } from "../composables/useListSelection";
import { usePagination } from "../composables/usePagination";

const { completedTasks, refreshAll } = useAppData();
const { confirmAction } = useDialogs();
const { toggleTask, openTaskDetail, openCompletedMenu } = useItemActions();

const completedFilter = ref("");

const filteredCompleted = computed(() => {
  const query = parseQuery(completedFilter.value);
  if (isEmptyQuery(query)) {
    return completedTasks.value;
  }
  return completedTasks.value.filter(task =>
    matchesKeyword(
      { text: [task.description, markdownToPlainText(task.stickyContent)], tags: task.tags, project: task.project },
      query
    )
  );
});

const {
  pageIndex: completedPageIndex,
  pageSize: completedPageSize,
  totalPages: completedTotalPages,
  page: completedPage
} = usePagination(filteredCompleted, completedFilter);

// 多选（v2.2.8）：进入多选后点击行切换选择、Shift 连选，表头全选本页，批量栏中全选筛选结果；
// Ctrl / ⌘ 点击行可直接进入多选。
const selectionMode = ref(false);
const {
  selectedIds,
  count: selectedCount,
  pageAllSelected,
  pageSomeSelected,
  isSelected,
  toggle: toggleSelection,
  togglePage,
  selectAllVisible,
  clear: clearSelection
} = useListSelection(
  computed(() => filteredCompleted.value.map(task => task.id)),
  computed(() => completedPage.value.map(task => task.id))
);

const exitSelection = () => {
  selectionMode.value = false;
  clearSelection();
};

const handleRowClick = (event: MouseEvent, id: string) => {
  if (!selectionMode.value) {
    if (!(event.ctrlKey || event.metaKey)) return;
    selectionMode.value = true;
  } else if ((event.target as HTMLElement | null)?.closest("a, button, input, .task-tag")) {
    return;
  }
  toggleSelection(event, id);
};

const deleteSelected = () => {
  const ids = [...selectedIds.value];
  if (!ids.length) return;
  confirmAction({
    message: `确定要删除选中的 ${ids.length} 项已办事项吗？删除后可在回收站中恢复。`,
    action: async () => {
      await api.batchUpdateTasks(ids, { action: "delete" });
      exitSelection();
      await refreshAll();
    }
  });
};

// 快捷键：Esc 退出多选；Delete 删除选中的已办（需确认）。
const handleViewShortcut = (event: Event) => {
  const action = (event as CustomEvent<ShortcutAction>).detail;
  if (action.type === "escape" && selectionMode.value) {
    exitSelection();
  } else if (action.type === "delete" && selectedCount.value) {
    deleteSelected();
  }
};
onMounted(() => window.addEventListener(VIEW_SHORTCUT_EVENT, handleViewShortcut));
onBeforeUnmount(() => window.removeEventListener(VIEW_SHORTCUT_EVENT, handleViewShortcut));
</script>
