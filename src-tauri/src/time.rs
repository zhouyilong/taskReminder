//! 时间的统一入口：当前时间、数据库时间格式，以及应用中用到的几种固定格式的解析与格式化。
//!
//! 数据库与前后端之间统一使用不带时区的本地时间（`DATETIME_FORMAT`）。其他模块不要直接调用
//! `Local::now()` 或 `parse_from_str`：当前时间走 `now()`（测试中可用 `fix_now` 固定），
//! 格式走这里的函数，避免同一种格式在各处写出细微差别。

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};

/// 数据库中统一使用的本地时间格式（不带时区）。
pub const DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";
/// 钟点（勿扰时段、循环提醒的触发时间等）。
const CLOCK_FORMAT: &str = "%H:%M";
const CLOCK_WITH_SECONDS_FORMAT: &str = "%H:%M:%S";
/// 日期（节假日数据）。
const DATE_FORMAT: &str = "%Y-%m-%d";
/// 给人看的完整时间（调试信息）。
const DISPLAY_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
/// 备份文件名中的时间戳，如 `20260930-083000`。
const STAMP_FORMAT: &str = "%Y%m%d-%H%M%S";
/// 导出文件名中的日期，如 `20260930`。
const COMPACT_DATE_FORMAT: &str = "%Y%m%d";
/// ICS 中的本地时间与 UTC 时间。
const ICS_LOCAL_FORMAT: &str = "%Y%m%dT%H%M%S";
const ICS_UTC_FORMAT: &str = "%Y%m%dT%H%M%SZ";

#[cfg(test)]
thread_local! {
    static FIXED_NOW: std::cell::Cell<Option<NaiveDateTime>> = const { std::cell::Cell::new(None) };
}

/// 当前本地时间。测试中可用 `fix_now` 固定（只影响当前线程）。
pub fn now() -> NaiveDateTime {
    #[cfg(test)]
    if let Some(fixed) = FIXED_NOW.with(|cell| cell.get()) {
        return fixed;
    }
    Local::now().naive_local()
}

/// 测试用：把当前线程的 `now()` 固定为 `value`，返回的守卫离开作用域时恢复。
#[cfg(test)]
pub fn fix_now(value: &str) -> FixedNow {
    let fixed = parse_datetime_any(value).expect("fix_now 需要 YYYY-MM-DDTHH:MM[:SS]");
    FIXED_NOW.with(|cell| cell.set(Some(fixed)));
    FixedNow
}

#[cfg(test)]
#[must_use = "守卫离开作用域时恢复真实时间"]
pub struct FixedNow;

#[cfg(test)]
impl Drop for FixedNow {
    fn drop(&mut self) {
        FIXED_NOW.with(|cell| cell.set(None));
    }
}

pub fn format_datetime(value: &NaiveDateTime) -> String {
    value.format(DATETIME_FORMAT).to_string()
}

pub fn now_string() -> String {
    format_datetime(&now())
}

/// 解析数据库或前端传入的本地时间，兼容带毫秒、带秒与只到分钟三种写法。
pub fn parse_datetime_any(value: &str) -> Option<NaiveDateTime> {
    let candidates = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ];
    for fmt in candidates {
        if let Ok(dt) = NaiveDateTime::parse_from_str(value, fmt) {
            return Some(dt);
        }
    }
    None
}

/// 解析钟点：`HH:MM`，也接受带秒的 `HH:MM:SS`（部分时间输入框会带秒）；忽略首尾空白。
pub fn parse_clock(value: &str) -> Option<NaiveTime> {
    let value = value.trim();
    NaiveTime::parse_from_str(value, CLOCK_FORMAT)
        .or_else(|_| NaiveTime::parse_from_str(value, CLOCK_WITH_SECONDS_FORMAT))
        .ok()
}

/// 钟点统一写成 `HH:MM`。
pub fn format_clock(value: &NaiveTime) -> String {
    value.format(CLOCK_FORMAT).to_string()
}

pub fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, DATE_FORMAT).ok()
}

pub fn format_date(value: &NaiveDate) -> String {
    value.format(DATE_FORMAT).to_string()
}

/// 给人看的完整时间，如 `2026-09-30 08:30:00`。
pub fn format_display(value: &NaiveDateTime) -> String {
    value.format(DISPLAY_FORMAT).to_string()
}

/// 文件修改时间等系统时间转为本地时间。
pub fn from_system_time(value: std::time::SystemTime) -> NaiveDateTime {
    DateTime::<Local>::from(value).naive_local()
}

/// 备份文件名中的时间戳。
pub fn format_stamp(value: &NaiveDateTime) -> String {
    value.format(STAMP_FORMAT).to_string()
}

pub fn parse_stamp(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, STAMP_FORMAT).ok()
}

/// 导出文件名中的日期。
pub fn format_compact_date(value: &NaiveDateTime) -> String {
    value.format(COMPACT_DATE_FORMAT).to_string()
}

/// ICS 中不带时区的本地时间（`DTSTART` 等）。
pub fn format_ics_local(value: &NaiveDateTime) -> String {
    value.format(ICS_LOCAL_FORMAT).to_string()
}

/// 当前 Unix 时间戳（毫秒）。用于跨设备比较的同步锁过期时间，与时区无关。
pub fn unix_millis() -> i64 {
    Utc::now().timestamp_millis()
}

/// ICS 的 `DTSTAMP`：当前 UTC 时间。
pub fn ics_utc_now() -> String {
    Utc::now().format(ICS_UTC_FORMAT).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_formats() {
        assert!(parse_datetime_any("2026-09-26T10:00").is_some());
        assert!(parse_datetime_any("2026-09-26T10:00:05").is_some());
        assert!(parse_datetime_any("2026-09-26T10:00:05.123").is_some());
        assert!(parse_datetime_any("2026-09-26 10:00").is_none());
        assert!(parse_datetime_any("").is_none());
    }

    #[test]
    fn format_roundtrip() {
        let dt = parse_datetime_any("2026-09-26T10:00:05").unwrap();
        assert_eq!(format_datetime(&dt), "2026-09-26T10:00:05");
        assert_eq!(format_display(&dt), "2026-09-26 10:00:05");
        assert_eq!(format_stamp(&dt), "20260926-100005");
        assert_eq!(parse_stamp("20260926-100005"), Some(dt));
        assert_eq!(parse_stamp("2026-09-26"), None);
        assert_eq!(format_compact_date(&dt), "20260926");
        assert_eq!(format_ics_local(&dt), "20260926T100005");
    }

    #[test]
    fn clock_and_date() {
        assert_eq!(
            parse_clock("08:30").map(|t| format_clock(&t)),
            Some("08:30".to_string())
        );
        assert_eq!(
            parse_clock(" 08:30:15 ").map(|t| format_clock(&t)),
            Some("08:30".to_string())
        );
        assert_eq!(parse_clock("8:30"), NaiveTime::from_hms_opt(8, 30, 0));
        assert!(parse_clock("25:00").is_none());
        assert!(parse_clock("").is_none());
        let date = parse_date("2026-10-01").unwrap();
        assert_eq!(format_date(&date), "2026-10-01");
        assert!(parse_date("2026/10/01").is_none());
    }

    #[test]
    fn now_can_be_fixed_in_tests() {
        {
            let _now = fix_now("2026-10-01T09:00");
            assert_eq!(now_string(), "2026-10-01T09:00:00");
        }
        // 守卫释放后恢复真实时间。
        assert_ne!(now_string(), "2026-10-01T09:00:00");
    }

    #[test]
    fn ics_utc_stamp_shape() {
        let stamp = ics_utc_now();
        assert_eq!(stamp.len(), 16);
        assert!(stamp.ends_with('Z') && stamp.as_bytes()[8] == b'T');
    }
}
