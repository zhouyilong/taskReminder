// 列表多选：点击切换、Shift 连选（以上一次点击为锚点）、全选当前页、列表变化后清理。
export interface SelectionState {
  ids: Set<string>;
  /** Shift 连选的起点：上一次单独点击的行。 */
  anchor: string | null;
}

export const emptySelection = (): SelectionState => ({ ids: new Set(), anchor: null });

export const toggleSelected = (state: SelectionState, id: string): SelectionState => {
  const ids = new Set(state.ids);
  if (ids.has(id)) ids.delete(id);
  else ids.add(id);
  return { ids, anchor: id };
};

/** 选中锚点到 `id` 之间（按 `ordered` 的顺序，含两端）的所有行；没有锚点或锚点不在列表中时等同于切换。 */
export const selectRange = (state: SelectionState, ordered: readonly string[], id: string): SelectionState => {
  const from = state.anchor ? ordered.indexOf(state.anchor) : -1;
  const to = ordered.indexOf(id);
  if (from < 0 || to < 0) return toggleSelected(state, id);
  const ids = new Set(state.ids);
  const [start, end] = from <= to ? [from, to] : [to, from];
  for (const item of ordered.slice(start, end + 1)) ids.add(item);
  return { ids, anchor: state.anchor };
};

/** 全选 `ordered`；已经全部选中时取消这些行的选择。 */
export const toggleAll = (state: SelectionState, ordered: readonly string[]): SelectionState => {
  const ids = new Set(state.ids);
  const allSelected = ordered.length > 0 && ordered.every(id => ids.has(id));
  for (const id of ordered) {
    if (allSelected) ids.delete(id);
    else ids.add(id);
  }
  return { ids, anchor: state.anchor };
};

/** 只保留仍然可见的行（筛选、完成、删除后调用）。 */
export const pruneSelection = (state: SelectionState, visible: readonly string[]): SelectionState => {
  const keep = new Set(visible);
  const ids = new Set([...state.ids].filter(id => keep.has(id)));
  const anchor = state.anchor && keep.has(state.anchor) ? state.anchor : null;
  return ids.size === state.ids.size && anchor === state.anchor ? state : { ids, anchor };
};
