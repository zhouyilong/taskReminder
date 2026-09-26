use chrono::{Local, NaiveDateTime};

/// 数据库中统一使用的本地时间格式（不带时区）。
pub const DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

pub fn format_datetime(value: &NaiveDateTime) -> String {
    value.format(DATETIME_FORMAT).to_string()
}

pub fn now_string() -> String {
    format_datetime(&Local::now().naive_local())
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
    }
}
