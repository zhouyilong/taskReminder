// 打开中的弹窗（按打开顺序）：Esc 只关闭最上层的一个。
const stack: Array<() => void> = [];

export const pushModal = (close: () => void) => {
  stack.push(close);
  return () => {
    const index = stack.lastIndexOf(close);
    if (index >= 0) stack.splice(index, 1);
  };
};

export const hasOpenModal = () => stack.length > 0;

/** 关闭最上层的弹窗；没有弹窗时返回 false。 */
export const closeTopModal = () => {
  const close = stack[stack.length - 1];
  if (!close) return false;
  close();
  return true;
};
