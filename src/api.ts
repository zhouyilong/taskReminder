import { invoke } from "@tauri-apps/api/core";
import type {
  Task,
  RecurringTask,
  RecurringMode,
  ReminderRecord,
  StickyNote,
  AppSettings,
  SyncStatus,
  NotificationPayload
} from "./types";

export const api = {
  async listActiveTasks(): Promise<Task[]> {
    return invoke("list_active_tasks");
  },
  async listCompletedTasks(): Promise<Task[]> {
    return invoke("list_completed_tasks");
  },
  async listRecurringTasks(): Promise<RecurringTask[]> {
    return invoke("list_recurring_tasks");
  },
  async listReminderRecords(): Promise<ReminderRecord[]> {
    return invoke("list_reminder_records");
  },
  async createTask(payload: { description: string; stickyContent?: string | null }): Promise<Task> {
    return invoke("create_task", { payload });
  },
  async updateTask(task: {
    id: string;
    description: string;
    stickyContent?: string | null;
    reminderTime?: string | null;
  }): Promise<void> {
    return invoke("update_task", { task });
  },
  async completeTask(id: string): Promise<void> {
    return invoke("complete_task", { id });
  },
  async uncompleteTask(id: string): Promise<void> {
    return invoke("uncomplete_task", { id });
  },
  async deleteTask(id: string): Promise<void> {
    return invoke("delete_task", { id });
  },
  async createRecurringTask(payload: {
    description: string;
    intervalMinutes: number;
    startTime?: string | null;
    endTime?: string | null;
    repeatMode?: RecurringMode;
    scheduleTime?: string | null;
    scheduleWeekday?: number | null;
    scheduleDay?: number | null;
    cronExpression?: string | null;
  }): Promise<RecurringTask> {
    return invoke("create_recurring_task", { payload });
  },
  async updateRecurringTask(task: RecurringTask): Promise<void> {
    return invoke("update_recurring_task", { task });
  },
  async pauseRecurringTask(id: string): Promise<void> {
    return invoke("pause_recurring_task", { id });
  },
  async resumeRecurringTask(id: string): Promise<void> {
    return invoke("resume_recurring_task", { id });
  },
  async deleteRecurringTask(id: string): Promise<void> {
    return invoke("delete_recurring_task", { id });
  },
  async deleteReminderRecord(id: string): Promise<void> {
    return invoke("delete_reminder_record", { id });
  },
  async deleteReminderRecords(ids: string[]): Promise<void> {
    return invoke("delete_reminder_records", { ids });
  },
  async getSettings(): Promise<AppSettings> {
    return invoke("get_settings");
  },
  async saveSettings(settings: AppSettings): Promise<void> {
    return invoke("save_settings", { settings });
  },
  async listStickyNotes(): Promise<StickyNote[]> {
    return invoke("list_sticky_notes");
  },
  async getStickyNoteByWindowLabel(label: string): Promise<StickyNote | null> {
    return invoke("get_sticky_note_by_window_label", { label });
  },
  async openStickyNote(payload: {
    taskId: string;
    title?: string | null;
    defaultX?: number | null;
    defaultY?: number | null;
  }): Promise<StickyNote> {
    return invoke("open_sticky_note", { payload });
  },
  async createStickyNote(payload: {
    title?: string | null;
    defaultX?: number | null;
    defaultY?: number | null;
  }): Promise<StickyNote> {
    return invoke("create_sticky_note", { payload });
  },
  async saveStickyNoteContent(payload: {
    taskId: string;
    content: string;
  }): Promise<void> {
    return invoke("save_sticky_note_content", { payload });
  },
  async updateStickyNoteTitle(payload: {
    taskId: string;
    title: string;
  }): Promise<void> {
    return invoke("update_sticky_note_title", { payload });
  },
  async moveStickyNote(payload: {
    taskId: string;
    x: number;
    y: number;
  }): Promise<void> {
    return invoke("move_sticky_note", { payload });
  },
  async closeStickyNote(taskId: string): Promise<void> {
    return invoke("close_sticky_note", { taskId });
  },
  async closeStickyNoteByWindowLabel(label: string): Promise<void> {
    return invoke("close_sticky_note_by_window_label", { label });
  },
  async setStickyNoteOpacity(opacity: number): Promise<number> {
    return invoke("set_sticky_note_opacity", { opacity });
  },
  async isStickyNoteWindowVisible(): Promise<boolean> {
    return invoke("is_sticky_note_window_visible");
  },
  async setStickyNoteWindowVisible(visible: boolean): Promise<boolean> {
    return invoke("set_sticky_note_window_visible", { visible });
  },
  async forceShowStickyNoteWindow(): Promise<boolean> {
    return invoke("force_show_sticky_note_window");
  },
  async testWebDav(settings: AppSettings): Promise<{ ok: boolean; message: string }> {
    return invoke("test_webdav", { settings });
  },
  async syncNow(reason: string): Promise<void> {
    return invoke("sync_now", { reason });
  },
  async setAutoStart(enabled: boolean): Promise<void> {
    return invoke("set_autostart", { enabled });
  },
  async acknowledgeNotification(payload: {
    recordId: string;
    action: string;
  }): Promise<void> {
    return invoke("ack_notification", { payload });
  },
  async snoozeNotification(payload: {
    recordId: string;
    reminderId: string;
    reminderType: string;
    minutes: number;
  }): Promise<void> {
    return invoke("snooze_notification", { payload });
  },
  async getSyncStatus(): Promise<SyncStatus> {
    return invoke("get_sync_status");
  },
  async getNotificationSnapshot(): Promise<NotificationPayload | null> {
    return invoke("get_notification_snapshot");
  },
  async isDevMode(): Promise<boolean> {
    return invoke("is_dev_mode");
  },
};
