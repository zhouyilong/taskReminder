<template>
  <div class="tab-panel">
    <div class="section-heading">
      <div class="section-title">已办事项</div>
      <span class="section-meta">筛选后 {{ filteredCompleted.length }} 条</span>
    </div>
    <div class="search-field">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="11" cy="11" r="6.5" />
        <path d="M16 16l4 4" />
      </svg>
      <input class="input" v-model="completedFilter" placeholder="按标题或描述搜索已办事项" />
      <button v-if="completedFilter" class="search-clear" type="button" title="清空" @click="completedFilter = ''">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 7l10 10M17 7L7 17" />
        </svg>
      </button>
    </div>
    <div class="table-card">
      <div class="table-scroll table-scroll-no-x">
        <table class="table completed-table">
          <thead>
            <tr>
              <th class="col-select">取消完成</th>
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
              @dblclick="openTaskDetail(task)"
              @contextmenu.prevent.stop="openCompletedMenu($event, task)"
            >
              <td class="col-select">
                <input type="checkbox" class="check-round" title="取消完成" checked @change="toggleTask(task)" />
              </td>
              <td class="col-desc cell-title is-done" :title="task.description">{{ task.description }}</td>
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
import { computed, ref } from "vue";
import Pagination from "../components/Pagination.vue";
import { formatDateTime, taskStickyPreview } from "../format";
import { markdownToPlainText } from "../markdown";
import { useAppData } from "../composables/useAppData";
import { useItemActions } from "../composables/useItemActions";
import { usePagination } from "../composables/usePagination";

const { completedTasks } = useAppData();
const { toggleTask, openTaskDetail, openCompletedMenu } = useItemActions();

const completedFilter = ref("");

const filteredCompleted = computed(() => {
  const keyword = completedFilter.value.trim().toLowerCase();
  if (!keyword) {
    return completedTasks.value;
  }
  return completedTasks.value.filter(task => {
    const descriptionMatched = task.description.toLowerCase().includes(keyword);
    const stickyContentMatched = markdownToPlainText(task.stickyContent).toLowerCase().includes(keyword);
    return descriptionMatched || stickyContentMatched;
  });
});

const {
  pageIndex: completedPageIndex,
  pageSize: completedPageSize,
  totalPages: completedTotalPages,
  page: completedPage
} = usePagination(filteredCompleted, completedFilter);
</script>
