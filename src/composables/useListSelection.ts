// 表格多选（v2.2.8，提醒记录与已办事项共用）：勾选切换、Shift 连选、全选本页、全选筛选结果，
// 列表变化（筛选、删除）后只保留仍然可见的行。
import { computed, ref, watch, type Ref } from "vue";
import { emptySelection, pruneSelection, selectAll, selectRange, toggleAll, toggleSelected } from "../selection";

export const useListSelection = (visibleIds: Ref<string[]>, pageIds: Ref<string[]>) => {
  const selection = ref(emptySelection());
  const selectedIds = computed(() => [...selection.value.ids]);
  const count = computed(() => selection.value.ids.size);
  const pageAllSelected = computed(
    () => pageIds.value.length > 0 && pageIds.value.every(id => selection.value.ids.has(id))
  );
  const pageSomeSelected = computed(() => pageIds.value.some(id => selection.value.ids.has(id)));

  const isSelected = (id: string) => selection.value.ids.has(id);
  /** 切换一行；按住 Shift 时选中上一次点击的行到这一行之间的所有行（跨页按筛选后的顺序）。 */
  const toggle = (event: MouseEvent, id: string) => {
    selection.value = event.shiftKey ? selectRange(selection.value, visibleIds.value, id) : toggleSelected(selection.value, id);
  };
  const togglePage = () => {
    selection.value = toggleAll(selection.value, pageIds.value);
  };
  const selectAllVisible = () => {
    selection.value = selectAll(selection.value, visibleIds.value);
  };
  const clear = () => {
    selection.value = emptySelection();
  };

  watch(visibleIds, ids => {
    selection.value = pruneSelection(selection.value, ids);
  });

  return { selection, selectedIds, count, pageAllSelected, pageSomeSelected, isSelected, toggle, togglePage, selectAllVisible, clear };
};
