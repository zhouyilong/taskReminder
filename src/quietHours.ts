// 勿扰时段的界面文案（判断逻辑在后端 `quiet_hours.rs`）。

const toMinutes = (value: string) => {
  const match = /^(\d{1,2}):(\d{2})/.exec(value.trim());
  if (!match) {
    return null;
  }
  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  return hours < 24 && minutes < 60 ? hours * 60 + minutes : null;
};

/** 勿扰时段说明；开始等于结束或时间无效时提示不会生效。 */
export const formatQuietHoursHint = (start: string, end: string) => {
  const from = toMinutes(start);
  const to = toMinutes(end);
  if (from === null || to === null || from === to) {
    return "开始与结束时间相同，勿扰不会生效";
  }
  const range = from < to ? `${start} 至 ${end}` : `${start} 至次日 ${end}`;
  return `${range}：提醒照常记录但不弹窗，结束后一次弹出`;
};
