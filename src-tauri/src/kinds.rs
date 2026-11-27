//! 复导出 `task_reminder_core::kinds` 中的强类型枚举（本地与云端严格共用同一定义）。

pub use task_reminder_core::kinds::{
    AppMode, ReminderAction, ReminderKind, RepeatMode, TaskStatus, TaskType,
};
