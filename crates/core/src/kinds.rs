//! 以字符串存库、在前后端及云端之间传递的状态与类型字段的强类型表示。
//!
//! **不认识的值原样保留**（`Unknown`）：新版本可能新增状态或循环模式，并经同步传到旧客户端；
//! 若回退成已知值再写回，会把新版本的数据改坏并同步回去。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// 本版本不认识的值，读写时原样保留。
            Unknown(String),
        }

        impl $name {
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $text, )+
                    Self::Unknown(value) => value,
                }
            }

            /// 精确匹配已知值（区分大小写）。
            fn from_known(value: &str) -> Option<Self> {
                match value {
                    $( $text => Some(Self::$variant), )+
                    _ => None,
                }
            }

            pub fn is_known(&self) -> bool {
                !matches!(self, Self::Unknown(_))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::parse(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Ok(Self::parse(&value))
            }
        }
    };
}

string_enum! {
    /// 待办 / 循环提醒的状态（`tasks.status`、`recurring_tasks.status`）。
    TaskStatus {
        Pending => "PENDING",
        Completed => "COMPLETED",
    }
}

impl TaskStatus {
    pub fn parse(value: &str) -> Self {
        Self::from_known(value).unwrap_or_else(|| Self::Unknown(value.to_string()))
    }
}

string_enum! {
    /// 待办的类型（`tasks.type`、`recurring_tasks.type`）。
    TaskType {
        OneTime => "ONE_TIME",
        Recurring => "RECURRING",
    }
}

impl TaskType {
    pub fn parse(value: &str) -> Self {
        Self::from_known(value).unwrap_or_else(|| Self::Unknown(value.to_string()))
    }
}

string_enum! {
    /// 提醒记录与提醒弹窗的来源（`reminder_records.type`）。
    ReminderKind {
        Task => "TASK",
        Recurring => "RECURRING",
    }
}

impl ReminderKind {
    pub fn parse(value: &str) -> Self {
        Self::from_known(value).unwrap_or_else(|| Self::Unknown(value.to_string()))
    }
}

string_enum! {
    /// 提醒记录的处理结果（`reminder_records.action`）。
    ReminderAction {
        Pending => "PENDING",
        Completed => "COMPLETED",
        Dismissed => "DISMISSED",
        Snoozed => "SNOOZED",
    }
}

impl ReminderAction {
    pub fn parse(value: &str) -> Self {
        Self::from_known(value).unwrap_or_else(|| Self::Unknown(value.to_string()))
    }
}

string_enum! {
    /// 循环提醒的模式（`recurring_tasks.repeat_mode`）。
    RepeatMode {
        IntervalRange => "INTERVAL_RANGE",
        Daily => "DAILY",
        Weekly => "WEEKLY",
        Monthly => "MONTHLY",
        Cron => "CRON",
        /// 中国法定工作日：跳过法定节假日，调休上班日照常提醒。
        Workday => "WORKDAY",
        /// 每月最后一天（v2.1）。
        MonthlyLastDay => "MONTHLY_LAST_DAY",
        /// 每月最后一个法定工作日（v2.1）。
        MonthlyLastWorkday => "MONTHLY_LAST_WORKDAY",
    }
}

impl RepeatMode {
    /// 兼容旧数据：不区分大小写，`INTERVAL` / `INTERVAL-RANGE` 视为区间间隔，空值视为区间间隔
    /// （V1.4.1 迁移的默认值）。其他不认识的值原样保留，不再回退为区间间隔。
    pub fn parse(value: &str) -> Self {
        let normalized = value.trim().to_uppercase();
        match normalized.as_str() {
            "" | "INTERVAL" | "INTERVAL-RANGE" => Self::IntervalRange,
            other => Self::from_known(other).unwrap_or_else(|| Self::Unknown(value.to_string())),
        }
    }
}

/// 极简两级账号角色（`users.role`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    #[default]
    User,
}

impl UserRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "admin" => Some(Self::Admin),
            "user" => Some(Self::User),
            _ => None,
        }
    }

    pub fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 客户端运行模式（本地模式 / 云端模式）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    #[default]
    Local,
    Cloud,
}

impl AppMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "cloud" => Self::Cloud,
            _ => Self::Local,
        }
    }

    pub fn is_cloud(self) -> bool {
        matches!(self, Self::Cloud)
    }
}

impl std::fmt::Display for AppMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_roundtrip_as_the_same_text() {
        for status in [TaskStatus::Pending, TaskStatus::Completed] {
            assert_eq!(TaskStatus::parse(status.as_str()), status);
        }
        for action in [
            ReminderAction::Pending,
            ReminderAction::Completed,
            ReminderAction::Dismissed,
            ReminderAction::Snoozed,
        ] {
            assert_eq!(ReminderAction::parse(action.as_str()), action);
        }
        assert_eq!(TaskType::parse("ONE_TIME"), TaskType::OneTime);
        assert_eq!(ReminderKind::parse("TASK"), ReminderKind::Task);
        assert_eq!(RepeatMode::parse("WORKDAY").as_str(), "WORKDAY");
        assert_eq!(UserRole::parse("admin"), Some(UserRole::Admin));
        assert_eq!(UserRole::parse("USER"), Some(UserRole::User));
        assert_eq!(UserRole::parse("guest"), None);
        assert_eq!(AppMode::parse("cloud"), AppMode::Cloud);
        assert_eq!(AppMode::parse("local"), AppMode::Local);
        assert_eq!(AppMode::parse("unknown"), AppMode::Local);
    }

    #[test]
    fn unknown_values_are_preserved() {
        let mode = RepeatMode::parse("BIWEEKLY");
        assert_eq!(mode, RepeatMode::Unknown("BIWEEKLY".to_string()));
        assert!(!mode.is_known());
        assert_eq!(mode.as_str(), "BIWEEKLY");
        assert_eq!(RepeatMode::parse(" biWeekly ").as_str(), " biWeekly ");
        assert_eq!(TaskStatus::parse("ARCHIVED").as_str(), "ARCHIVED");
        assert!(!TaskStatus::parse("completed").is_known());
    }

    #[test]
    fn repeat_mode_accepts_legacy_spellings() {
        assert_eq!(RepeatMode::parse("workday"), RepeatMode::Workday);
        assert_eq!(RepeatMode::parse(" daily "), RepeatMode::Daily);
        assert_eq!(RepeatMode::parse("INTERVAL"), RepeatMode::IntervalRange);
        assert_eq!(
            RepeatMode::parse("interval-range"),
            RepeatMode::IntervalRange
        );
        assert_eq!(RepeatMode::parse(""), RepeatMode::IntervalRange);
    }

    #[test]
    fn json_matches_the_previous_string_fields() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Row {
            status: TaskStatus,
            mode: RepeatMode,
            role: UserRole,
            app_mode: AppMode,
        }
        let row = Row {
            status: TaskStatus::Completed,
            mode: RepeatMode::Unknown("BIWEEKLY".to_string()),
            role: UserRole::Admin,
            app_mode: AppMode::Cloud,
        };
        let json = serde_json::to_string(&row).unwrap();
        assert_eq!(
            json,
            r#"{"status":"COMPLETED","mode":"BIWEEKLY","role":"admin","app_mode":"cloud"}"#
        );
        assert_eq!(serde_json::from_str::<Row>(&json).unwrap(), row);
    }
}
