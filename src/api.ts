import { invoke } from "@tauri-apps/api/core";
import type {
  TaskBatchPayload,
  Task,
  RecurringTask,
  RecurringMode,
  ReminderRecord,
  StickyNote,
  StickyNoteSummary,
  AppSettings,
  CloudStatus,
  CloudHealthResponse,
  CloudUserInfo,
  AdminUserSummary,
  SwitchToCloudPreview,
  SwitchToCloudResult,
  SwitchToLocalPreview,
  SwitchToLocalResult,
  UserRole,
  NotificationPayload,
  UiStatePayload,
  TrashPayload,
  RecurringPreview,
  ExportResult,
  ImportSummary,
  BackupListPayload,
  AttentionEffect,
  NotificationScreen,
  TodoStickyState
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
  async createTask(payload: {
    description: string;
    stickyContent?: string | null;
    tags?: string[];
    priority?: number;
    reminderTime?: string | null;
    dueAt?: string | null;
    project?: string;
  }): Promise<Task> {
    return invoke("create_task", { payload });
  },
  async updateTask(task: {
    id: string;
    description: string;
    stickyContent?: string | null;
    reminderTime?: string | null;
    tags?: string[];
    priority?: number;
    dueAt?: string | null;
    project?: string;
  }): Promise<void> {
    return invoke("update_task", { task });
  },
  async completeTask(id: string): Promise<void> {
    return invoke("complete_task", { id });
  },
  async uncompleteTask(id: string): Promise<void> {
    return invoke("uncomplete_task", { id });
  },
  async batchUpdateTasks(ids: string[], payload: TaskBatchPayload): Promise<number> {
    return invoke("batch_update_tasks", { ids, payload });
  },
  async renameTag(oldName: string, newName: string): Promise<number> {
    return invoke("rename_tag", { oldName, newName });
  },
  async removeTag(name: string): Promise<number> {
    return invoke("remove_tag", { name });
  },
  async renameProject(oldName: string, newName: string): Promise<number> {
    return invoke("rename_project", { oldName, newName });
  },
  async setStickyNoteColor(taskId: string, color: string): Promise<void> {
    return invoke("set_sticky_note_color", { taskId, color });
  },
  async setTaskOrder(orders: Array<{ id: string; sortOrder: number }>): Promise<number> {
    return invoke("set_task_order", { orders });
  },
  async setTaskUrgent(id: string, urgent: boolean): Promise<boolean> {
    return invoke("set_task_urgent", { id, urgent });
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
    scheduleWeekdays?: number | null;
    scheduleDay?: number | null;
    cronExpression?: string | null;
    tags?: string[];
    endsOn?: string | null;
    remainingCount?: number | null;
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
  async setRecurringTempReminder(id: string, at: string | null, replaceNext: boolean): Promise<RecurringTask> {
    return invoke("set_recurring_temp_reminder", { id, at, replaceNext });
  },
  async previewRecurringTempTarget(id: string, at: string): Promise<string | null> {
    return invoke("preview_recurring_temp_target", { id, at });
  },
  async skipRecurringOccurrence(id: string): Promise<RecurringTask> {
    return invoke("skip_recurring_occurrence", { id });
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
  async deleteReminderRecordsBefore(before: string): Promise<number> {
    return invoke("delete_reminder_records_before", { before });
  },
  async listTrash(): Promise<TrashPayload> {
    return invoke("list_trash");
  },
  async restoreTask(id: string): Promise<void> {
    return invoke("restore_task", { id });
  },
  async restoreRecurringTask(id: string): Promise<void> {
    return invoke("restore_recurring_task", { id });
  },
  async purgeTrash(payload: { taskIds: string[]; recurringIds: string[] }): Promise<void> {
    return invoke("purge_trash", { payload });
  },
  async previewRecurringTriggers(until: string, limit?: number): Promise<RecurringPreview[]> {
    return invoke("preview_recurring_triggers", { until, limit: limit ?? null });
  },
  async exportData(format: "json" | "markdown" | "ics"): Promise<ExportResult | null> {
    return invoke("export_data", { format });
  },
  async importData(): Promise<ImportSummary | null> {
    return invoke("import_data");
  },
  async listBackups(): Promise<BackupListPayload> {
    return invoke("list_backups");
  },
  async createBackupNow(): Promise<void> {
    return invoke("create_backup_now");
  },
  async openBackupDir(): Promise<void> {
    return invoke("open_backup_dir");
  },
  async restoreBackup(name: string): Promise<void> {
    return invoke("restore_backup", { name });
  },
  async getSettings(): Promise<AppSettings> {
    return invoke("get_settings");
  },
  async saveSettings(settings: AppSettings): Promise<void> {
    return invoke("save_settings", { settings });
  },
  async listStickyNoteSummaries(): Promise<StickyNoteSummary[]> {
    return invoke("list_sticky_note_summaries");
  },
  async showStickyNote(taskId: string): Promise<void> {
    return invoke("show_sticky_note", { taskId });
  },
  async getStickyNoteByWindowLabel(label: string): Promise<StickyNote | null> {
    return invoke("get_sticky_note_by_window_label", { label });
  },
  async getStickyNote(noteId: string): Promise<StickyNote | null> {
    return invoke("get_sticky_note", { noteId });
  },
  async getTodoStickyState(): Promise<TodoStickyState> {
    return invoke("get_todo_sticky_state");
  },
  async openTodoSticky(): Promise<TodoStickyState> {
    return invoke("open_todo_sticky");
  },
  async closeTodoSticky(): Promise<TodoStickyState> {
    return invoke("close_todo_sticky");
  },
  async setTodoStickyPinned(pinned: boolean): Promise<TodoStickyState> {
    return invoke("set_todo_sticky_pinned", { pinned });
  },
  async setTodoStickyColor(color: string): Promise<TodoStickyState> {
    return invoke("set_todo_sticky_color", { color });
  },
  async openStickyNote(payload: {
    taskId: string;
    title?: string | null;
    defaultX?: number | null;
    defaultY?: number | null;
    focus?: boolean;
    besideWindow?: string;
  }): Promise<StickyNote> {
    return invoke("open_sticky_note", { payload });
  },
  async createStickyNote(payload: {
    title?: string | null;
    content?: string | null;
    width?: number | null;
    height?: number | null;
    defaultX?: number | null;
    defaultY?: number | null;
    besideWindow?: string;
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
  async updateStickyNoteReminder(payload: {
    taskId: string;
    reminderTime?: string | null;
  }): Promise<void> {
    return invoke("update_sticky_note_reminder", { payload });
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
  async setStickyNotePinnedByWindowLabel(label: string, pinned: boolean): Promise<boolean> {
    return invoke("set_sticky_note_pinned_by_window_label", { label, pinned });
  },
  async getStickyNotePinnedByWindowLabel(label: string): Promise<boolean> {
    return invoke("get_sticky_note_pinned_by_window_label", { label });
  },
  async checkHolidayUpdates(): Promise<{ changed: boolean; years: number[] }> {
    return invoke("check_holiday_updates");
  },
  async setAutoStart(enabled: boolean): Promise<void> {
    return invoke("set_autostart", { enabled });
  },
  async acknowledgeNotification(payload: {
    recordId: string;
    action: string;
  }): Promise<NotificationPayload[]> {
    return invoke("ack_notification", { payload });
  },
  async acknowledgeAllNotifications(): Promise<void> {
    return invoke("ack_all_notifications");
  },
  async snoozeNotification(payload: {
    recordId: string;
    reminderId: string;
    reminderType: string;
    minutes: number;
    until?: string | null;
  }): Promise<NotificationPayload[]> {
    return invoke("snooze_notification", { payload });
  },
  async completeNotification(payload: {
    recordId: string;
    reminderId: string;
  }): Promise<NotificationPayload[]> {
    return invoke("complete_notification", { payload });
  },
  async getHolidayYears(): Promise<number[]> {
    return invoke("get_holiday_years");
  },
  async quickAddTask(payload: {
    description: string;
    reminderTime?: string | null;
    tags?: string[];
    priority?: number;
    dueAt?: string | null;
    project?: string;
  }): Promise<Task> {
    return invoke("quick_add_task", { payload });
  },
  async applyQuickAddShortcut(): Promise<void> {
    return invoke("apply_quick_add_shortcut");
  },
  async previewAttention(payload: {
    effect: AttentionEffect;
    untilHandled: boolean;
    intensity: number;
    color: string;
    screen: NotificationScreen;
    theme: string;
  }): Promise<void> {
    return invoke("preview_attention", { payload });
  },
  async getNotificationQueue(): Promise<NotificationPayload[]> {
    return invoke("get_notification_queue");
  },
  async isDevMode(): Promise<boolean> {
    return invoke("is_dev_mode");
  },
  async emitUiStateChanged(uiScale: number, theme: string, windowOpacity: number): Promise<void> {
    return invoke("emit_ui_state_changed", { uiScale, theme, windowOpacity });
  },
  async getUiState(): Promise<UiStatePayload | null> {
    return invoke("get_ui_state");
  },

  // ---- v3.0.0 云端模式、认证、模式切换与管理员账号管理 ----

  async getCloudStatus(): Promise<CloudStatus> {
    return invoke("get_cloud_status");
  },
  async cloudTestConnection(serverUrl: string): Promise<CloudHealthResponse> {
    return invoke("cloud_test_connection", { serverUrl });
  },
  async cloudSetServerUrl(serverUrl: string): Promise<CloudStatus> {
    return invoke("cloud_set_server_url", { serverUrl });
  },
  async cloudLogin(payload: {
    serverUrl?: string;
    username: string;
    password: string;
  }): Promise<CloudStatus> {
    return invoke("cloud_login", { payload });
  },
  async cloudLogout(): Promise<CloudStatus> {
    return invoke("cloud_logout");
  },
  async cloudSyncNow(): Promise<CloudStatus> {
    return invoke("cloud_sync_now");
  },
  async cloudUpdateProfile(nickname: string): Promise<CloudUserInfo> {
    return invoke("cloud_update_profile", { nickname });
  },
  async cloudChangePassword(payload: {
    currentPassword: string;
    newPassword: string;
  }): Promise<CloudUserInfo> {
    return invoke("cloud_change_password", { payload });
  },
  async previewSwitchToCloud(): Promise<SwitchToCloudPreview> {
    return invoke("preview_switch_to_cloud");
  },
  async switchToCloud(mergeLocal: boolean): Promise<SwitchToCloudResult> {
    return invoke("switch_to_cloud", { mergeLocal });
  },
  async previewSwitchToLocal(): Promise<SwitchToLocalPreview> {
    return invoke("preview_switch_to_local");
  },
  async switchToLocal(mergeCloud: boolean): Promise<SwitchToLocalResult> {
    return invoke("switch_to_local", { mergeCloud });
  },
  async adminListUsers(): Promise<AdminUserSummary[]> {
    return invoke("admin_list_users");
  },
  async adminCreateUser(payload: {
    username: string;
    nickname: string;
    role: UserRole;
    password: string;
    mustChangePassword: boolean;
  }): Promise<AdminUserSummary> {
    return invoke("admin_create_user", { payload });
  },
  async adminUpdateUser(payload: {
    userId: string;
    nickname?: string;
    role?: UserRole;
    enabled?: boolean;
  }): Promise<AdminUserSummary> {
    return invoke("admin_update_user", { payload });
  },
  async adminResetPassword(payload: {
    userId: string;
    newPassword: string;
    mustChangePassword?: boolean;
  }): Promise<AdminUserSummary> {
    return invoke("admin_reset_password", { payload });
  },
  async adminDeleteUser(userId: string): Promise<void> {
    return invoke("admin_delete_user", { userId });
  }
};
