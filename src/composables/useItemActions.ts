// 待办、循环提醒、提醒记录的通用操作与右键菜单，供多个视图复用。
import { api } from "../api";
import { markdownToPlainText } from "../markdown";
import { errorMessage, formatAction, formatDateTime, recordDescription } from "../format";
import { canSkipRecurring } from "../recurring";
import { priorityLabel, priorityOf } from "../tasks";
import type { RecurringTask, ReminderRecord, Task } from "../types";
import { useAppData } from "./useAppData";
import { useContextMenu } from "./useContextMenu";
import { useDialogs } from "./useDialogs";

export const useItemActions = () => {
  const { refreshAll } = useAppData();
  const { showContextMenu } = useContextMenu();
  const { confirmAction, openDetail, openTaskEditor, openRecurringEditor } = useDialogs();

  const toggleTask = async (task: Task) => {
    if (task.status === "COMPLETED") {
      await api.uncompleteTask(task.id);
    } else {
      await api.completeTask(task.id);
    }
    await refreshAll();
  };

  const openTaskStickyNote = async (task: Task) => {
    try {
      await api.openStickyNote({ taskId: task.id, title: task.description });
    } catch (error) {
      console.error("[main] 打开便签卡片失败", error);
      alert(`打开便签卡片失败：${errorMessage(error)}`);
    }
  };

  const confirmDeleteTask = (task: Task, afterDelete?: () => void) => {
    confirmAction({
      message: "确定要删除此任务吗？删除后可在回收站中恢复。",
      action: async () => {
        await api.deleteTask(task.id);
        afterDelete?.();
        await refreshAll();
      },
    });
  };

  const openTaskDetail = (task: Task) => {
    openDetail("任务详情", [
      { label: "标题", value: task.description },
      { label: "描述", value: markdownToPlainText(task.stickyContent) || "-" },
      { label: "状态", value: task.status === "COMPLETED" ? "已完成" : "待办" },
      { label: "创建时间", value: formatDateTime(task.createdAt) },
      { label: "完成时间", value: formatDateTime(task.completedAt) },
      { label: "提醒时间", value: formatDateTime(task.reminderTime) },
      { label: "优先级", value: priorityLabel(priorityOf(task)) },
      { label: "标签", value: task.tags?.length ? task.tags.map(tag => `#${tag}`).join(" ") : "-" },
    ]);
  };

  const openTaskMenu = (event: MouseEvent, task: Task) => {
    showContextMenu(event, [
      { label: "编辑", action: () => openTaskEditor(task) },
      { label: "打开便签卡片", action: () => openTaskStickyNote(task) },
      {
        label: task.status === "COMPLETED" ? "取消完成" : "标记完成",
        action: () => toggleTask(task),
      },
      { label: "删除", action: () => confirmDeleteTask(task), danger: true },
    ]);
  };

  const openCompletedMenu = (event: MouseEvent, task: Task) => {
    showContextMenu(event, [
      { label: "查看详情", action: () => openTaskDetail(task) },
      { label: "取消完成", action: () => toggleTask(task) },
      { label: "删除", action: () => confirmDeleteTask(task), danger: true },
    ]);
  };

  const toggleRecurring = async (task: RecurringTask) => {
    if (task.isPaused) {
      await api.resumeRecurringTask(task.id);
    } else {
      await api.pauseRecurringTask(task.id);
    }
    await refreshAll();
  };

  const skipRecurring = async (task: RecurringTask) => {
    try {
      await api.skipRecurringOccurrence(task.id);
    } catch (error) {
      console.error("[recurring] 跳过本次失败", error);
    }
    await refreshAll();
  };

  const confirmDeleteRecurring = (task: RecurringTask, afterDelete?: () => void) => {
    confirmAction({
      message: "确定要删除此循环提醒吗？删除后可在回收站中恢复。",
      action: async () => {
        await api.deleteRecurringTask(task.id);
        afterDelete?.();
        await refreshAll();
      },
    });
  };

  const openRecurringMenu = (event: MouseEvent, task: RecurringTask) => {
    showContextMenu(event, [
      { label: "编辑", action: () => openRecurringEditor(task) },
      ...(canSkipRecurring(task)
        ? [{ label: `跳过本次（${formatDateTime(task.nextTrigger)}）`, action: () => skipRecurring(task) }]
        : []),
      { label: task.isPaused ? "恢复" : "暂停", action: () => toggleRecurring(task) },
      { label: "删除", action: () => confirmDeleteRecurring(task), danger: true },
    ]);
  };

  const openRecordDetail = (record: ReminderRecord) => {
    openDetail("提醒记录详情", [
      { label: "描述", value: recordDescription(record.description) },
      { label: "类型", value: record.type === "TASK" ? "任务" : "循环" },
      { label: "触发时间", value: formatDateTime(record.triggerTime) },
      { label: "关闭时间", value: formatDateTime(record.closeTime) },
      { label: "操作", value: formatAction(record.action) },
    ]);
  };

  const confirmDeleteRecords = (ids: string[], afterDelete?: () => void) => {
    if (!ids.length) {
      return;
    }
    confirmAction({
      message: ids.length === 1 ? "确定要删除该记录吗？" : `确定要删除 ${ids.length} 条记录吗？`,
      action: async () => {
        if (ids.length === 1) {
          await api.deleteReminderRecord(ids[0]);
        } else {
          await api.deleteReminderRecords(ids);
        }
        afterDelete?.();
        await refreshAll();
      },
    });
  };

  const openRecordMenu = (event: MouseEvent, record: ReminderRecord) => {
    showContextMenu(event, [
      { label: "查看详情", action: () => openRecordDetail(record) },
      { label: "删除", action: () => confirmDeleteRecords([record.id]), danger: true },
    ]);
  };

  return {
    toggleTask,
    openTaskStickyNote,
    confirmDeleteTask,
    openTaskDetail,
    openTaskMenu,
    openCompletedMenu,
    toggleRecurring,
    confirmDeleteRecurring,
    openRecurringMenu,
    openRecordDetail,
    confirmDeleteRecords,
    openRecordMenu,
    openTaskEditor,
    openRecurringEditor,
  };
};
