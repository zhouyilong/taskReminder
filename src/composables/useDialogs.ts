// 主窗口共享的弹窗状态（模块级单例）：确认框、详情、编辑待办、编辑循环提醒。
// 弹窗组件统一挂在 App.vue，各视图只调用这里的打开方法。
import { reactive } from "vue";
import { isSupportedRecurringMode } from "../recurring";
import type { RecurringTask, Task } from "../types";

export type DetailItem = { label: string; value: string };

const confirmState = reactive({
  open: false,
  title: "确认删除",
  message: "",
  action: null as null | (() => Promise<void>),
});

const detailState = reactive({
  open: false,
  title: "",
  items: [] as DetailItem[],
});

const taskEditor = reactive({
  open: false,
  task: null as Task | null,
});

const recurringEditor = reactive({
  open: false,
  task: null as RecurringTask | null,
});

const confirmAction = (options: { title?: string; message: string; action: () => Promise<void> }) => {
  confirmState.title = options.title ?? "确认删除";
  confirmState.message = options.message;
  confirmState.action = options.action;
  confirmState.open = true;
};

const closeConfirm = () => {
  confirmState.open = false;
  confirmState.action = null;
};

const runConfirm = async () => {
  const action = confirmState.action;
  try {
    if (action) {
      await action();
    }
  } finally {
    closeConfirm();
  }
};

const openDetail = (title: string, items: DetailItem[]) => {
  detailState.title = title;
  detailState.items = items;
  detailState.open = true;
};

const openTaskEditor = (task: Task) => {
  taskEditor.task = task;
  taskEditor.open = true;
};

const openRecurringEditor = (task: RecurringTask) => {
  if (!isSupportedRecurringMode(task.repeatMode)) {
    alert(`这条循环提醒使用了本版本不支持的模式（${task.repeatMode}），请升级应用后再编辑。`);
    return;
  }
  recurringEditor.task = task;
  recurringEditor.open = true;
};

export const useDialogs = () => ({
  confirmState,
  detailState,
  taskEditor,
  recurringEditor,
  confirmAction,
  closeConfirm,
  runConfirm,
  openDetail,
  openTaskEditor,
  openRecurringEditor,
});
