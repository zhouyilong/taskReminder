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
  /** 标签（已去重、不含 #）。 */
  tags?: string[];
  /** 优先级：0 无、1 低、2 中、3 高。 */
  priority?: number;
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
  /** 云同步端到端加密。 */
  syncEncryptionEnabled: boolean;
  /** 同步密码，只保存在本机。 */
  syncPassphrase: string;
  /** 勿扰时段（HH:mm，支持跨午夜）：期间提醒只记录不弹窗，结束后一次性弹出。 */
  quietHoursEnabled: boolean;
  quietHoursStart: string;
  quietHoursEnd: string;
  /** 弹出提醒时同时发送系统原生通知。 */
  nativeNotificationEnabled: boolean;
  /** 显示/隐藏全部便签的全局快捷键，为空表示不启用。 */
  stickyToggleShortcut: string;
  /** 拖动便签后贴边吸附（屏幕边缘与其他便签，仅 Windows）。 */
  stickySnapEnabled: boolean;
  /** 密码存放位置（只读）：keyring 为 Windows 凭据管理器，db 为本机数据库。 */
  secretStorage?: "db" | "keyring" | string;
}

export interface UiStatePayload {
  uiScale: number;
  theme: string;
  windowOpacity: number;
}

export interface SyncStatus {
  /** 状态码（`SyncStateCode`），文案用 `syncStateLabel` 映射。 */
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

export interface TrashPayload {
  tasks: Task[];
  recurringTasks: RecurringTask[];
  /** 墓碑保留天数：超过后自动永久删除。 */
  retentionDays: number;
}

export interface RecurringPreview {
  taskId: string;
  /** 预估的触发时间（本地时间），按先后排列。 */
  times: string[];
}

export interface ExportResult {
  path: string;
  /** 导出日历时无法表达而跳过的循环提醒数量。 */
  skipped: number;
}

export interface ImportSummary {
  inserted: number;
  updated: number;
  skipped: number;
}

export interface BackupInfo {
  name: string;
  size: number;
  createdAt: string;
}

export interface BackupListPayload {
  dir: string;
  backups: BackupInfo[];
}
