<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">待办事项</div>
      <span class="section-meta">共 {{ tasks.length }} 条任务</span>
    </div>
    <div class="composer-card">
      <div class="composer-title-row">
        <span class="composer-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </span>
        <input
          class="composer-input"
          v-model="newTaskDescription"
          placeholder="添加新任务，输入标题后按回车"
          @keydown.enter.exact.prevent="handleAddTask"
        />
        <button class="button" :disabled="!newTaskDescription.trim()" @click="handleAddTask">添加任务</button>
      </div>
      <MarkdownNoteEditor
        v-model="newTaskStickyContent"
        class="composer-editor"
        variant="ghost"
        :theme="isLightTheme ? 'light' : 'dark'"
        placeholder="补充任务描述（可选），支持 Markdown 所见即所得"
      />
    </div>
    <div class="table-card">
      <div class="table-scroll table-scroll-no-x">
        <table class="table tasks-table">
          <thead>
            <tr>
              <th class="col-select">完成</th>
              <th class="col-desc">标题</th>
              <th class="col-note">描述</th>
              <th class="col-datetime">提醒时间</th>
              <th class="col-datetime">创建时间</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="task in tasksPage"
              :key="task.id"
              class="table-row"
              @dblclick="openTaskEditor(task)"
              @contextmenu.prevent.stop="openTaskMenu($event, task)"
            >
              <td class="col-select">
                <input
                  type="checkbox"
                  class="check-round"
                  title="标记完成"
                  :checked="task.status === 'COMPLETED'"
                  @change="toggleTask(task)"
                />
              </td>
              <td class="col-desc cell-title" :title="task.description">{{ task.description }}</td>
              <td class="col-note cell-muted" :class="{ 'cell-empty': !task.stickyContent?.trim() }" :title="taskStickyPreview(task.stickyContent)">{{ taskStickyPreview(task.stickyContent) }}</td>
              <td class="col-datetime" :title="formatDateTime(task.reminderTime)">
                <span v-if="task.reminderTime" class="time-chip" :class="reminderTone(task.reminderTime)">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <circle cx="12" cy="12" r="8" />
                    <path d="M12 8v4l2.5 1.5" />
                  </svg>
                  {{ formatDateTime(task.reminderTime) }}
                </span>
                <span v-else class="cell-empty">未设置</span>
              </td>
              <td class="col-datetime cell-time" :title="formatDateTime(task.createdAt)">{{ formatDateTime(task.createdAt) }}</td>
            </tr>
            <tr v-if="!tasksPage.length" class="table-empty-row">
              <td colspan="5">
                <div class="table-empty">
                  <span class="table-empty-title">暂无待办任务</span>
                  <span class="table-empty-hint">在上方输入标题即可快速添加</span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <Pagination :total="tasks.length" :total-pages="tasksTotalPages" v-model:page-index="tasksPageIndex" v-model:page-size="tasksPageSize" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import MarkdownNoteEditor from "../components/MarkdownNoteEditor.vue";
import Pagination from "../components/Pagination.vue";
import { api } from "../api";
import { formatDateTime, reminderTone, taskStickyPreview } from "../format";
import { useAppData } from "../composables/useAppData";
import { useItemActions } from "../composables/useItemActions";
import { usePagination } from "../composables/usePagination";
import { useUiPrefs } from "../composables/useUiPrefs";

const { tasks, refreshAll } = useAppData();
const { toggleTask, openTaskMenu, openTaskEditor } = useItemActions();
const { isLightTheme } = useUiPrefs();
const {
  pageIndex: tasksPageIndex,
  pageSize: tasksPageSize,
  totalPages: tasksTotalPages,
  page: tasksPage
} = usePagination(tasks);

const newTaskDescription = ref("");
const newTaskStickyContent = ref("");

const handleAddTask = async () => {
  const description = newTaskDescription.value.trim();
  if (!description) {
    return;
  }
  await api.createTask({
    description,
    stickyContent: newTaskStickyContent.value.trim() ? newTaskStickyContent.value : null,
  });
  newTaskDescription.value = "";
  newTaskStickyContent.value = "";
  await refreshAll();
};
</script>
