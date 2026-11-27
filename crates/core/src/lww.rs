//! LWW（Last-Write-Wins）冲突裁决辅助函数，客户端与服务端共用同一份逻辑。
//!
//! 规则：
//! - 有效比较时间为 `max(updated_at, deleted_at)`；两者都为空或无法解析时回退至 `fallback`（`created_at` / `trigger_time`）。
//! - 较新的一方胜出；时间戳相同时保留已有（不触发无意义的覆盖）。

use chrono::NaiveDateTime;

/// 解析业务时间字符串（`YYYY-MM-DDTHH:MM[:SS[.fff]]`）。
pub fn parse_business_datetime(value: &str) -> Option<NaiveDateTime> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(trimmed, fmt) {
            return Some(dt);
        }
    }
    None
}

/// 计算单条记录的有效比较时间：`max(updated_at, deleted_at)`，均为空时回退至 `fallback`。
pub fn effective_compare_time(
    updated_at: Option<&str>,
    deleted_at: Option<&str>,
    fallback: Option<&str>,
) -> Option<NaiveDateTime> {
    let u = updated_at.and_then(parse_business_datetime);
    let d = deleted_at.and_then(parse_business_datetime);
    match (u, d) {
        (Some(ut), Some(dt)) => Some(ut.max(dt)),
        (Some(ut), None) => Some(ut),
        (None, Some(dt)) => Some(dt),
        (None, None) => fallback.and_then(parse_business_datetime),
    }
}

/// 返回传入行是否比现有行更新（严格大于时返回 true；现有行无有效时间而传入行有时返回 true）。
pub fn is_incoming_newer(existing: Option<NaiveDateTime>, incoming: Option<NaiveDateTime>) -> bool {
    match (existing, incoming) {
        (Some(e), Some(i)) => i > e,
        (None, Some(_)) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_time_takes_max_of_updated_and_deleted() {
        let t = effective_compare_time(
            Some("2026-11-27T10:00:00"),
            Some("2026-11-27T12:00:00"),
            Some("2026-11-27T08:00:00"),
        );
        assert_eq!(t, parse_business_datetime("2026-11-27T12:00:00"));

        let t2 = effective_compare_time(
            Some("2026-11-27T14:00:00"),
            Some("2026-11-27T12:00:00"),
            Some("2026-11-27T08:00:00"),
        );
        assert_eq!(t2, parse_business_datetime("2026-11-27T14:00:00"));
    }

    #[test]
    fn effective_time_falls_back_when_both_missing() {
        let t = effective_compare_time(None, Some(""), Some("2026-11-27T08:00:00"));
        assert_eq!(t, parse_business_datetime("2026-11-27T08:00:00"));
    }

    #[test]
    fn incoming_newer_strictly_compares() {
        let earlier = parse_business_datetime("2026-11-27T10:00:00");
        let later = parse_business_datetime("2026-11-27T10:00:01");
        assert!(is_incoming_newer(earlier, later));
        assert!(!is_incoming_newer(later, earlier));
        assert!(!is_incoming_newer(later, later));
        assert!(is_incoming_newer(None, earlier));
        assert!(!is_incoming_newer(earlier, None));
    }
}
