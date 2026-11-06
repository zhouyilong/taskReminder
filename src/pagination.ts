// 分页条的页码跳转（v2.2.8）：把输入的页码规范到 1 … 总页数。

/** 解析输入的页码：非数字返回 null；小数取整，超出范围收回到首页或末页。 */
export const clampPage = (input: unknown, totalPages: number): number | null => {
  const text = typeof input === "number" ? String(input) : typeof input === "string" ? input.trim() : "";
  if (!/^[+-]?\d+(\.\d+)?$/.test(text)) return null;
  const last = Math.max(1, Math.floor(totalPages) || 1);
  return Math.min(last, Math.max(1, Math.trunc(Number(text))));
};
