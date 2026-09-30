// Milkdown 载入内容后会按自己的格式重新序列化（如 `-` 列表变为 `*`、补空行），并触发一次
// markdownUpdated。这不是用户修改：如果照常向外发出，便签会自动保存、产生一次多余的同步改动。

export interface MarkdownBaselineState {
  /** 编辑器创建完成前触发的更新都属于载入。 */
  ready: boolean;
  /** 编辑器对载入内容的序列化结果；创建完成前为 null。 */
  baseline: string | null;
  /** 用户是否已经改过内容（之后的更新一律发出，包括改回原样）。 */
  userEdited: boolean;
}

/** 这次 markdownUpdated 是否应当作为用户修改向外发出。 */
export const isUserMarkdownChange = (state: MarkdownBaselineState, next: string): boolean => {
  if (!state.ready) {
    return false;
  }
  if (state.userEdited) {
    return true;
  }
  return next !== state.baseline;
};
