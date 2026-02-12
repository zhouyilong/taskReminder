export type TaskStatus = "PENDING" | "COMPLETED";
export type TaskType = "ONE_TIME" | "RECURRING";
export type ReminderType = "TASK" | "RECURRING";
export type UserAction = "DISMISSED" | "SNOOZED" | "COMPLETED" | "PENDING";
export type RecurringMode = "INTERVAL_RANGE" | "DAILY" | "WEEKLY" | "MONTHLY" | "CRON";

export interface Task {
  id: string;
  description: string;
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

export interface AppSettings {
  autoStartEnabled: boolean;
  soundEnabled: boolean;
  snoozeMinutes: number;
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
}
