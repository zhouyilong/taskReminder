// 拖拽排序（v2.1）：计算需要写入的 sortOrder。一般只改被拖动的一行（取前后两行的中间值）；
// 有行还没有位置、或间隔太小时，按新顺序给整个列表重新编号。
export interface OrderedItem {
  id: string;
  sortOrder?: number | null;
}

export const ORDER_STEP = 1024;
const MIN_GAP = 1e-6;

const hasOrder = (item: OrderedItem) => typeof item.sortOrder === "number" && Number.isFinite(item.sortOrder);

/**
 * 把 `id` 移到 `toIndex`（按移动前的列表计算的插入位置，0 … length）。
 * 返回要写入的 `{ id, sortOrder }`；位置没有变化时返回空数组。
 */
export const planReorder = (items: readonly OrderedItem[], id: string, toIndex: number) => {
  const from = items.findIndex(item => item.id === id);
  if (from < 0) return [];
  const target = Math.max(0, Math.min(items.length, toIndex));
  // 插入到自己前面或后面紧挨着的位置：顺序不变。
  if (target === from || target === from + 1) return [];

  const rest = items.filter(item => item.id !== id);
  const insertAt = target > from ? target - 1 : target;
  const reordered = [...rest.slice(0, insertAt), items[from], ...rest.slice(insertAt)];

  const before = rest[insertAt - 1];
  const after = rest[insertAt];
  if (rest.every(hasOrder)) {
    let order: number | null = null;
    if (before && after) {
      const gap = (after.sortOrder as number) - (before.sortOrder as number);
      if (gap > MIN_GAP * 2) order = (before.sortOrder as number) + gap / 2;
    } else if (before) {
      order = (before.sortOrder as number) + ORDER_STEP;
    } else if (after) {
      order = (after.sortOrder as number) - ORDER_STEP;
    } else {
      order = ORDER_STEP;
    }
    if (order !== null) return [{ id, sortOrder: order }];
  }
  return reordered.map((item, index) => ({ id: item.id, sortOrder: (index + 1) * ORDER_STEP }));
};
