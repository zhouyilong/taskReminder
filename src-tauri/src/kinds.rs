//! 以字符串存库、在前后端之间传递的状态与类型字段的强类型表示。
//!
//! 数据库中的文本与 `invoke` 返回的 JSON 保持不变（与旧版本、前端 `src/types.ts` 一致）。
//! **不认识的值原样保留**（`Unknown`）：新版本可能新增状态或循环模式，并经同步传到本机；
//! 若像以前那样回退成已知值再写回，会把新版本的数据改坏并同步回去。

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
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

            #[allow(dead_code)] // 宏为每个枚举都生成，目前只有部分枚举用到
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

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.as_str()))
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                match value {
                    // 列都是 NOT NULL；万一合并进来空值，按未知值读出，不让整行读取失败。
                    ValueRef::Null => Ok(Self::Unknown(String::new())),
                    other => other.as_str().map(Self::parse),
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

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
    }

    #[test]
    fn unknown_values_are_preserved() {
        let mode = RepeatMode::parse("BIWEEKLY");
        assert_eq!(mode, RepeatMode::Unknown("BIWEEKLY".to_string()));
        assert!(!mode.is_known());
        assert_eq!(mode.as_str(), "BIWEEKLY");
        // 原样保留，不做大小写或空白处理。
        assert_eq!(RepeatMode::parse(" biWeekly ").as_str(), " biWeekly ");
        assert_eq!(TaskStatus::parse("ARCHIVED").as_str(), "ARCHIVED");
        // 状态、类型、处理结果区分大小写，与以前按字符串比较的行为一致。
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
        }
        let row = Row {
            status: TaskStatus::Completed,
            mode: RepeatMode::Unknown("BIWEEKLY".to_string()),
        };
        let json = serde_json::to_string(&row).unwrap();
        assert_eq!(json, r#"{"status":"COMPLETED","mode":"BIWEEKLY"}"#);
        assert_eq!(serde_json::from_str::<Row>(&json).unwrap(), row);
    }

    #[test]
    fn sqlite_stores_and_reads_plain_text() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (mode TEXT)").unwrap();
        for mode in [
            RepeatMode::Weekly,
            RepeatMode::Unknown("BIWEEKLY".to_string()),
        ] {
            conn.execute("DELETE FROM t", []).unwrap();
            conn.execute("INSERT INTO t (mode) VALUES (?)", [&mode])
                .unwrap();
            let text: String = conn
                .query_row("SELECT mode FROM t", [], |row| row.get(0))
                .unwrap();
            assert_eq!(text, mode.as_str());
            let read: RepeatMode = conn
                .query_row("SELECT mode FROM t", [], |row| row.get(0))
                .unwrap();
            assert_eq!(read, mode);
        }
        conn.execute("DELETE FROM t", []).unwrap();
        conn.execute("INSERT INTO t (mode) VALUES (NULL)", [])
            .unwrap();
        let read: RepeatMode = conn
            .query_row("SELECT mode FROM t", [], |row| row.get(0))
            .unwrap();
        assert!(!read.is_known());
    }
}
