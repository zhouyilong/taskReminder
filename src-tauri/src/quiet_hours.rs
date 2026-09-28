//! 勿扰时段：期间提醒照常写记录并进入队列，但不弹窗；结束后由巡检一次性弹出。

use chrono::{Local, NaiveTime};

use crate::models::AppSettings;

pub const DEFAULT_START: &str = "22:00";
pub const DEFAULT_END: &str = "08:00";

fn parse_clock(value: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value.trim(), "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(value.trim(), "%H:%M:%S"))
        .ok()
}

/// 规范为 `HH:MM`；无法解析时回退为默认值。
pub fn normalize_clock(value: &str, fallback: &str) -> String {
    parse_clock(value)
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_else(|| fallback.to_string())
}

/// `now` 是否落在 `[start, end)` 内；`start > end` 表示跨午夜（如 22:00–08:00），
/// `start == end` 视为未设置时段。
pub fn is_quiet_at(start: &str, end: &str, now: NaiveTime) -> bool {
    let (Some(start), Some(end)) = (parse_clock(start), parse_clock(end)) else {
        return false;
    };
    if start == end {
        return false;
    }
    if start < end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

pub fn is_quiet_now(settings: &AppSettings) -> bool {
    settings.quiet_hours_enabled
        && is_quiet_at(
            &settings.quiet_hours_start,
            &settings.quiet_hours_end,
            Local::now().time(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M").unwrap()
    }

    #[test]
    fn same_day_range() {
        assert!(!is_quiet_at("12:00", "14:00", at("11:59")));
        assert!(is_quiet_at("12:00", "14:00", at("12:00")));
        assert!(is_quiet_at("12:00", "14:00", at("13:59")));
        assert!(!is_quiet_at("12:00", "14:00", at("14:00")));
    }

    #[test]
    fn range_across_midnight() {
        assert!(is_quiet_at("22:00", "08:00", at("22:00")));
        assert!(is_quiet_at("22:00", "08:00", at("23:59")));
        assert!(is_quiet_at("22:00", "08:00", at("00:00")));
        assert!(is_quiet_at("22:00", "08:00", at("07:59")));
        assert!(!is_quiet_at("22:00", "08:00", at("08:00")));
        assert!(!is_quiet_at("22:00", "08:00", at("21:59")));
    }

    #[test]
    fn empty_or_invalid_range_is_never_quiet() {
        assert!(!is_quiet_at("09:00", "09:00", at("09:00")));
        assert!(!is_quiet_at("", "08:00", at("03:00")));
        assert!(!is_quiet_at("25:00", "08:00", at("03:00")));
    }

    #[test]
    fn normalize_clock_pads_and_falls_back() {
        assert_eq!(normalize_clock("7:05", DEFAULT_END), "07:05");
        assert_eq!(normalize_clock("22:00:00", DEFAULT_START), "22:00");
        assert_eq!(normalize_clock("abc", DEFAULT_START), "22:00");
    }
}
