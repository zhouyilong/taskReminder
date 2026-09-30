<template>
  <aside class="sidebar" :class="{ collapsed: isSidebarCollapsed }">
    <div class="sidebar-header">
      <span class="sidebar-title" v-if="!isSidebarCollapsed">菜单</span>
      <button
        class="sidebar-toggle"
        type="button"
        :title="isSidebarCollapsed ? '展开菜单' : '收起菜单'"
        @click="toggleSidebar"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M14.5 6L8.5 12L14.5 18" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </button>
    </div>
    <template v-for="tab in tabs" :key="tab.key">
      <span v-if="tab.key === 'stats'" class="sidebar-spacer" aria-hidden="true"></span>
      <button
        class="tab-button"
        :class="{ active: activeTab === tab.key }"
        :title="isSidebarCollapsed ? tab.label : undefined"
        @click="activeTab = tab.key"
      >
        <span class="tab-icon" v-html="tab.icon"></span>
        <span class="tab-text">{{ tab.label }}</span>
        <span v-if="tab.badge" class="tab-badge">{{ tab.badge }}</span>
      </button>
    </template>
  </aside>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { TabKey } from "../navigation";
import { useAppData } from "../composables/useAppData";
import { useUiPrefs } from "../composables/useUiPrefs";

const activeTab = defineModel<TabKey>({ required: true });

const { tasks, activeRecurringCount } = useAppData();
const { isSidebarCollapsed, toggleSidebar } = useUiPrefs();

const icon = (body: string) =>
  `<svg viewBox="0 0 24 24" aria-hidden="true" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;

const ICONS: Record<TabKey, string> = {
  today: icon('<rect x="4" y="5" width="16" height="15" rx="2.5" /><path d="M4 9.5h16M8.5 3v4M15.5 3v4" /><circle cx="12" cy="14.5" r="2" />'),
  calendar: icon('<rect x="4" y="5" width="16" height="15" rx="2.5" /><path d="M4 9.5h16M8.5 3v4M15.5 3v4" /><path d="M8 13.5h.01M12 13.5h.01M16 13.5h.01M8 17h.01M12 17h.01" stroke-width="2.4" />'),
  tasks: icon('<path d="M6 7h12M6 12h12M6 17h8" />'),
  completed: icon('<path d="M6 12l4 4 8-8" />'),
  recurring: icon('<path d="M4 12a8 8 0 0 1 13.6-5.6" /><path d="M20 6v5h-5" /><path d="M20 12a8 8 0 0 1-13.6 5.6" /><path d="M4 18v-5h5" />'),
  stickies: icon('<path d="M5 4.5h14v9.5l-5 5H5z" /><path d="M14 19v-5h5" /><path d="M8.5 9h7M8.5 12.5h4" />'),
  records: icon('<rect x="5" y="4" width="14" height="16" rx="2" ry="2" /><path d="M8 9h8M8 13h8M8 17h6" />'),
  stats: icon('<path d="M4 20h16" /><path d="M7 16v-5M12 16V7M17 16v-8" />'),
  trash: icon('<path d="M4.5 7h15" /><path d="M9.5 7V5h5v2" /><path d="M6.5 7l1 12.5h9l1-12.5" /><path d="M10.5 11v5M13.5 11v5" />'),
};

const tabs = computed(() => [
  { key: "today" as const, label: "今天", badge: 0 },
  { key: "calendar" as const, label: "日历", badge: 0 },
  { key: "tasks" as const, label: "待办事项", badge: tasks.value.length },
  { key: "completed" as const, label: "已办事项", badge: 0 },
  { key: "recurring" as const, label: "循环提醒", badge: activeRecurringCount.value },
  { key: "stickies" as const, label: "便签", badge: 0 },
  { key: "records" as const, label: "提醒记录", badge: 0 },
  { key: "stats" as const, label: "统计", badge: 0 },
  { key: "trash" as const, label: "回收站", badge: 0 },
].map(tab => ({ ...tab, icon: ICONS[tab.key] })));
</script>
