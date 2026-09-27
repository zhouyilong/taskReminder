import { computed, ref, watch, type Ref } from "vue";

/** 列表分页：页码超出范围时自动回退，`resetOn` 变化时回到第一页。 */
export const usePagination = <T>(items: Ref<T[]>, resetOn?: Ref<unknown>) => {
  const pageIndex = ref(1);
  const pageSize = ref(20);
  const totalPages = computed(() => Math.max(1, Math.ceil(items.value.length / pageSize.value)));
  const page = computed(() => {
    const start = (pageIndex.value - 1) * pageSize.value;
    return items.value.slice(start, start + pageSize.value);
  });

  watch([items, pageSize], () => {
    if (pageIndex.value > totalPages.value) {
      pageIndex.value = totalPages.value;
    }
  });
  if (resetOn) {
    watch(resetOn, () => {
      pageIndex.value = 1;
    });
  }

  return { pageIndex, pageSize, totalPages, page };
};
