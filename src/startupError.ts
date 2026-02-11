const getErrorMessage = (error: unknown): string => {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
};

export const renderStartupError = (
  selector: string,
  title: string,
  error: unknown
): void => {
  console.error("[startup] 初始化失败", error);

  const root = document.querySelector<HTMLElement>(selector);
  const target = root ?? document.body;
  const message = getErrorMessage(error);

  target.innerHTML = `
    <div style="padding:16px;font-family:'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif;color:#1f2937;">
      <div style="font-size:16px;font-weight:600;">${title}</div>
      <div style="margin-top:8px;color:#b91c1c;word-break:break-word;">${message}</div>
    </div>
  `;
};
