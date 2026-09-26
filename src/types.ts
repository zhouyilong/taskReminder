export type TaskStatus = "PENDING" | "COMPLETED";
export type TaskType = "ONE_TIME" | "RECURRING";
export type ReminderType = "TASK" | "RECURRING";
export type UserAction = "DISMISSED" | "SNOOZED" | "COMPLETED" | "PENDING";
export type RecurringMode = "INTERVAL_RANGE" | "DAILY" | "WORKDAY" | "WEEKLY" | "MONTHLY" | "CRON";

export interface Task {
  id: string;
  description: string;
  stickyContent?: string | null;
  type: TaskType;
  status: TaskStatus;
  createdAt: string;
  completedAt?: string | null;
  reminderTime?: string | null;
  updatedAt?: string | null;
  deletedAt?: string | null;
}

export interface RecurringTask {
  id: string;
  description: string;
  type: TaskType;
  status: TaskStatus;
  createdAt: string;
  completedAt?: string | null;
  reminderTime?: string | null;
  updatedAt?: string | null;
  deletedAt?: string | null;
  intervalMinutes: number;
  lastTriggered?: string | null;
  nextTrigger: string;
  isPaused: boolean;
  startTime?: string | null;
  endTime?: string | null;
  repeatMode: RecurringMode;
  scheduleTime?: string | null;
  scheduleWeekday?: number | null;
  /** 每周多天位掩码：周一 = bit0 … 周日 = bit6。 */
  scheduleWeekdays?: number | null;
  scheduleDay?: number | null;
  cronExpression?: string | null;
}

export interface ReminderRecord {
  id: string;
  reminderId: string;
  description: string;
  type: ReminderType;
  triggerTime: string;
  closeTime?: string | null;
  action: UserAction;
  updatedAt?: string | null;
  deletedAt?: string | null;
}

export interface StickyNote {
  taskId: string;
  title: string;
  noteType: "TASK" | "CUSTOM";
  content: string;
  posX: number;
  posY: number;
  width: number;
  height: number;
  isOpen: boolean;
  isPinned: boolean;
  createdAt: string;
  updatedAt: string;
  reminderTime?: string | null;
}

export interface AppSettings {
  autoStartEnabled: boolean;
  soundEnabled: boolean;
  snoozeMinutes: number;
  stickyNoteEnabled: boolean;
  stickyNoteContent: string;
  stickyNoteWidth: number;
  stickyNoteHeight: number;
  stickyNoteX?: number | null;
  stickyNoteY?: number | null;
  stickyNoteOpacity: number;
  windowOpacity: number;
  webdavEnabled: boolean;
  webdavUrl: string;
  webdavUsername: string;
  webdavPassword: string;
  webdavRootPath: string;
  webdavSyncIntervalMinutes: number;
  webdavLastSyncTime?: string | null;
  webdavLastLocalChangeTime?: string | null;
  webdavLastSyncStatus?: string | null;
  webdavLastSyncError?: string | null;
  webdavDeviceId: string;
  notificationTheme: "system" | "app" | "light" | "dark";
  quickAddEnabled: boolean;
  /** Tauri 加速键格式，如 CommandOrControl+Alt+N。 */
  quickAddShortcut: string;
}

export interface UiStatePayload {
  uiScale: number;
  theme: string;
  windowOpacity: number;
}

export interface SyncStatus {
  status: string;
  error?: string | null;
  time?: string | null;
}

export interface NotificationPayload {
  recordId: string;
  reminderId: string;
  reminderType: ReminderType;
  description: string;
  snoozeMinutes: number;
  /** 原定触发时间；明显早于弹出时间时视为“错过的提醒”。 */
  scheduledTime?: string | null;
}
