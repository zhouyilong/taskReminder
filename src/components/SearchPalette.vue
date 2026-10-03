<template>
  <Transition name="modal">
    <div v-if="searchOpen" class="modal-mask search-palette-mask" @click.self="closeSearch">
      <div class="search-palette" role="dialog" aria-modal="true" aria-label="全局搜索">
        <div class="search-palette-field">
          <svg class="search-icon" viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="11" cy="11" r="7" />
            <path d="M20 20l-3.5-3.5" />
          </svg>
          <input
            ref="inputRef"
            v-model="query"
            class="search-palette-input"
            type="text"
            placeholder="搜索待办、循环提醒、已办与便签内容"
            role="combobox"
            aria-autocomplete="list"
            :aria-expanded="hits.length > 0"
            aria-controls="search-palette-results"
            :aria-activedescendant="activeId"
            @keydown="handleKeydown"
          />
          <kbd class="search-palette-kbd">Esc</kbd>
        </div>

        <div id="search-palette-results" ref="resultsRef" class="search-palette-results" role="listbox" aria-label="搜索结果">
          <div v-if="!query.trim()" class="search-palette-empty">
            <div>输入关键词，多个词用空格分开</div>
            <div class="search-palette-empty-hint">以 # 开头只匹配标签，如“#工作 周报”</div>
          </div>
          <div v-else-if="!hits.length" class="search-palette-empty">没有找到“{{ query.trim() }}”</div>
          <template v-else>
            <section v-for="group in groups" :key="group.kind" class="search-palette-group" role="group" :aria-label="group.label">
              <header class="search-palette-group-title">
                {{ group.label }}
                <span class="search-palette-count">{{ group.total > group.hits.length ? `前 ${group.hits.length} / 共 ${group.total} 条` : `${group.total} 条` }}</span>
              </header>
              <div
                v-for="hit in group.hits"
                :id="optionId(hit)"
                :key="`${hit.kind}-${hit.id}`"
                class="search-palette-item"
                :class="{ 'is-active': indexOf(hit) === activeIndex }"
                role="option"
                :aria-selected="indexOf(hit) === activeIndex"
                :data-search-hit="hit.id"
                @mousemove="activeIndex = indexOf(hit)"
                @click="openHit(hit)"
              >
                <div class="search-palette-main">
                  <div class="search-palette-title">
                    <template v-for="(segment, index) in hit.title" :key="index">
                      <mark v-if="segment.hit">{{ segment.text }}</mark><template v-else>{{ segment.text }}</template>
                    </template>
                  </div>
                  <div v-if="hit.snippet" class="search-palette-snippet">
                    <template v-for="(segment, index) in hit.snippet" :key="index">
                      <mark v-if="segment.hit">{{ segment.text }}</mark><template v-else>{{ segment.text }}</template>
                    </template>
                  </div>
                </div>
                <div class="search-palette-side">
                  <span v-for="tag in hit.tags.slice(0, 3)" :key="tag" class="search-palette-tag">#{{ tag }}</span>
                  <span v-if="hitMeta(hit)" class="search-palette-meta">{{ hitMeta(hit) }}</span>
                  <button
                    v-if="hit.kind === 'task' && hit.hasSticky"
                    class="button ghost search-palette-sticky"
                    type="button"
                    title="打开便签"
                    @click.stop="openSticky(hit)"
                  >
                    便签
                  </button>
                </div>
              </div>
            </section>
          </template>
        </div>

        <footer class="search-palette-footer">
          <span><kbd>↑</kbd><kbd>↓</kbd> 选择</span>
          <span><kbd>Enter</kbd> 打开</span>
          <span><kbd>Esc</kbd> 关闭</span>
        </footer>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
// 全局搜索面板：在已加载的待办、循环提醒、已办中查找，回车跳到对应页面并打开编辑或详情。
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { flattenHits, hitMeta, searchAll, type SearchHit } from "../search";
import { pushModal } from "../modalStack";
import type { TabKey } from "../navigation";
import type { Task } from "../types";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";
import { useItemActions } from "../composables/useItemActions";
import { useSearchPalette } from "../composables/useSearchPalette";

const emit = defineEmits<{ (e: "navigate", tab: TabKey): void }>();

const { searchOpen, closeSearch } = useSearchPalette();
const { tasks, recurringTasks, completedTasks } = useAppData();
const { openTaskEditor, openRecurringEditor } = useDialogs();
const { openTaskDetail, openTaskStickyNote } = useItemActions();

const query = ref("");
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const resultsRef = ref<HTMLElement | null>(null);

const groups = computed(() =>
  searchAll({ tasks: tasks.value, recurringTasks: recurringTasks.value, completedTasks: completedTasks.value }, query.value)
);
const hits = computed(() => flattenHits(groups.value));
const indexOf = (hit: SearchHit) => hits.value.indexOf(hit);
const optionId = (hit: SearchHit) => `search-hit-${hit.kind}-${hit.id}`;
const activeId = computed(() => {
  const hit = hits.value[activeIndex.value];
  return hit ? optionId(hit) : undefined;
});

watch(query, () => {
  activeIndex.value = 0;
});

// 打开时登记到弹窗栈（Esc 关闭），聚焦并选中上次的查询。
let release: (() => void) | null = null;
watch(
  searchOpen,
  async open => {
    release?.();
    release = open ? pushModal(closeSearch) : null;
    if (open) {
      activeIndex.value = 0;
      await nextTick();
      inputRef.value?.focus();
      inputRef.value?.select();
    }
  },
  { immediate: true }
);
onBeforeUnmount(() => release?.());

const scrollActiveIntoView = async () => {
  await nextTick();
  const id = activeId.value;
  if (id) {
    document.getElementById(id)?.scrollIntoView({ block: "nearest" });
  }
};

const handleKeydown = (event: KeyboardEvent) => {
  if (event.isComposing) return;
  const count = hits.value.length;
  if (event.key === "ArrowDown" && count) {
    event.preventDefault();
    activeIndex.value = (activeIndex.value + 1) % count;
    void scrollActiveIntoView();
  } else if (event.key === "ArrowUp" && count) {
    event.preventDefault();
    activeIndex.value = (activeIndex.value - 1 + count) % count;
    void scrollActiveIntoView();
  } else if (event.key === "Enter") {
    event.preventDefault();
    const hit = hits.value[activeIndex.value];
    if (hit) openHit(hit);
  }
};

const openHit = (hit: SearchHit) => {
  closeSearch();
  if (hit.kind === "task") {
    emit("navigate", "tasks");
    openTaskEditor(hit.item);
  } else if (hit.kind === "recurring") {
    emit("navigate", "recurring");
    openRecurringEditor(hit.item);
  } else {
    emit("navigate", "completed");
    openTaskDetail(hit.item);
  }
};

const openSticky = (hit: SearchHit) => {
  closeSearch();
  void openTaskStickyNote(hit.item as Task);
};
</script>
