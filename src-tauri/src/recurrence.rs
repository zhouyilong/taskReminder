use std::str::FromStr;

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike};
use cron::Schedule;

use crate::errors::AppError;
use crate::holidays;
use crate::models::RecurringTask;

pub const REPEAT_MODE_INTERVAL_RANGE: &str = "INTERVAL_RANGE";
pub const REPEAT_MODE_DAILY: &str = "DAILY";
pub const REPEAT_MODE_WEEKLY: &str = "WEEKLY";
pub const REPEAT_MODE_MONTHLY: &str = "MONTHLY";
pub const REPEAT_MODE_CRON: &str = "CRON";
/// 中国法定工作日：跳过法定节假日，调休上班日照常提醒。
pub const REPEAT_MODE_WORKDAY: &str = "WORKDAY";

/// 每周多天的位掩码：周一 = bit0 … 周日 = bit6。
pub const WEEKDAY_MASK_ALL: i64 = 0b111_1111;

/// 周几（1 = 周一 … 7 = 周日）对应的位。
pub fn weekday_bit(weekday: i64) -> i64 {
    1 << (weekday - 1)
}

/// 位掩码中最早的一天（1–7），用于兼容只认 `schedule_weekday` 的旧版本。
fn first_weekday(mask: i64) -> Option<i64> {
    (1..=7).find(|day| mask & weekday_bit(*day) != 0)
}

/// 读取任务的每周位掩码；旧数据只有 `schedule_weekday` 时由它换算。
fn weekday_mask(task: &RecurringTask) -> Option<i64> {
    match task.schedule_weekdays {
        Some(mask) if mask & WEEKDAY_MASK_ALL != 0 => Some(mask & WEEKDAY_MASK_ALL),
        _ => task
            .schedule_weekday
            .filter(|day| (1..=7).contains(day))
            .map(weekday_bit),
    }
}

pub fn normalize_repeat_mode(mode: &str) -> String {
    match mode.trim().to_uppercase().as_str() {
        REPEAT_MODE_DAILY => REPEAT_MODE_DAILY.to_string(),
        REPEAT_MODE_WEEKLY => REPEAT_MODE_WEEKLY.to_string(),
        REPEAT_MODE_MONTHLY => REPEAT_MODE_MONTHLY.to_string(),
        REPEAT_MODE_CRON => REPEAT_MODE_CRON.to_string(),
        REPEAT_MODE_WORKDAY => REPEAT_MODE_WORKDAY.to_string(),
        "INTERVAL" | "INTERVAL-RANGE" | REPEAT_MODE_INTERVAL_RANGE => {
            REPEAT_MODE_INTERVAL_RANGE.to_string()
        }
        _ => REPEAT_MODE_INTERVAL_RANGE.to_string(),
    }
}

pub fn sanitize_recurring_task(task: &mut RecurringTask) -> Result<(), AppError> {
    task.description = task.description.trim().to_string();
    task.repeat_mode = normalize_repeat_mode(&task.repeat_mode);
    task.interval_minutes = task.interval_minutes.max(1);
    task.start_time = normalize_time_field(task.start_time.as_deref(), "开始时间")?;
    task.end_time = normalize_time_field(task.end_time.as_deref(), "结束时间")?;
    task.schedule_time = normalize_time_field(task.schedule_time.as_deref(), "触发时间")?;
    task.cron_expression = normalize_text(task.cron_expression.as_deref());

    if let (Some(start), Some(end)) = (task.start_time.as_deref(), task.end_time.as_deref()) {
        if parse_time(start)? > parse_time(end)? {
            return Err(AppError::Invalid(
                "间隔模式中开始时间不能晚于结束时间".to_string(),
            ));
        }
    }

    match task.repeat_mode.as_str() {
        REPEAT_MODE_INTERVAL_RANGE => {
            task.schedule_time = None;
            task.schedule_weekday = None;
            task.schedule_weekdays = None;
            task.schedule_day = None;
            task.cron_expression = None;
        }
        REPEAT_MODE_DAILY => {
            if task.schedule_time.is_none() {
                return Err(AppError::Invalid("每日模式需要设置触发时间".to_string()));
            }
            task.start_time = None;
            task.end_time = None;
            task.schedule_weekday = None;
            task.schedule_weekdays = None;
            task.schedule_day = None;
            task.cron_expression = None;
        }
        REPEAT_MODE_WEEKLY => {
            if task.schedule_time.is_none() {
                return Err(AppError::Invalid("每周模式需要设置触发时间".to_string()));
            }
            if let Some(mask) = task.schedule_weekdays {
                if mask & !WEEKDAY_MASK_ALL != 0 {
                    return Err(AppError::Invalid("每周模式中的周几无效".to_string()));
                }
            }
            if let Some(weekday) = task.schedule_weekday {
                if !(1..=7).contains(&weekday) {
                    return Err(AppError::Invalid(
                        "每周模式中的周几必须在 1 到 7 之间".to_string(),
                    ));
                }
            }
            let mask = weekday_mask(task)
                .ok_or_else(|| AppError::Invalid("每周模式需要至少选择一天".to_string()))?;
            task.schedule_weekdays = Some(mask);
            task.schedule_weekday = first_weekday(mask);
            task.start_time = None;
            task.end_time = None;
            task.schedule_day = None;
            task.cron_expression = None;
        }
        REPEAT_MODE_MONTHLY => {
            if task.schedule_time.is_none() {
                return Err(AppError::Invalid("每月模式需要设置触发时间".to_string()));
            }
            let day = task
                .schedule_day
                .ok_or_else(|| AppError::Invalid("每月模式需要设置几号".to_string()))?;
            if !(1..=31).contains(&day) {
                return Err(AppError::Invalid(
                    "每月模式中的几号必须在 1 到 31 之间".to_string(),
                ));
            }
            task.start_time = None;
            task.end_time = None;
            task.schedule_weekday = None;
            task.schedule_weekdays = None;
            task.cron_expression = None;
        }
        REPEAT_MODE_WORKDAY => {
            if task.schedule_time.is_none() {
                return Err(AppError::Invalid("工作日模式需要设置触发时间".to_string()));
            }
            task.start_time = None;
            task.end_time = None;
            task.schedule_weekday = None;
            task.schedule_weekdays = None;
            task.schedule_day = None;
            task.cron_expression = None;
        }
        REPEAT_MODE_CRON => {
            let expr = task
                .cron_expression
                .as_deref()
                .ok_or_else(|| AppError::Invalid("Cron 模式需要表达式".to_string()))?;
            task.cron_expression = Some(sanitize_cron_expression(expr)?);
            task.start_time = None;
            task.end_time = None;
            task.schedule_time = None;
            task.schedule_weekday = None;
            task.schedule_weekdays = None;
            task.schedule_day = None;
        }
        _ => {}
    }
    Ok(())
}

pub fn compute_next_trigger(
    task: &RecurringTask,
    base: Option<NaiveDateTime>,
) -> Result<String, AppError> {
    let mut normalized = task.clone();
    sanitize_recurring_task(&mut normalized)?;

    let base = base.unwrap_or_else(|| Local::now().naive_local());
    let next = match normalized.repeat_mode.as_str() {
        REPEAT_MODE_INTERVAL_RANGE => compute_interval_next(&normalized, base)?,
        REPEAT_MODE_DAILY => compute_daily_next(&normalized, base)?,
        REPEAT_MODE_WEEKLY => compute_weekly_next(&normalized, base)?,
        REPEAT_MODE_MONTHLY => compute_monthly_next(&normalized, base)?,
        REPEAT_MODE_CRON => compute_cron_next(&normalized, base)?,
        REPEAT_MODE_WORKDAY => compute_workday_next(&normalized, base)?,
        _ => compute_interval_next(&normalized, base)?,
    };
    Ok(next.format("%Y-%m-%dT%H:%M:%S").to_string())
}

pub fn should_trigger_now(task: &RecurringTask, now: NaiveDateTime) -> Result<bool, AppError> {
    let mut normalized = task.clone();
    sanitize_recurring_task(&mut normalized)?;
    if normalized.repeat_mode != REPEAT_MODE_INTERVAL_RANGE {
        return Ok(true);
    }
    let now_time = now.time();
    if let Some(start) = normalized.start_time.as_deref() {
        if minute_of_day(now_time) < minute_of_day(parse_time(start)?) {
            return Ok(false);
        }
    }
    if let Some(end) = normalized.end_time.as_deref() {
        if minute_of_day(now_time) > minute_of_day(parse_time(end)?) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn compute_interval_next(
    task: &RecurringTask,
    base: NaiveDateTime,
) -> Result<NaiveDateTime, AppError> {
    let current_time = base.time();
    if let Some(start) = task.start_time.as_deref() {
        let start_time = parse_time(start)?;
        if minute_of_day(current_time) < minute_of_day(start_time) {
            return Ok(NaiveDateTime::new(base.date(), start_time));
        }
    }

    if let Some(end) = task.end_time.as_deref() {
        let end_time = parse_time(end)?;
        if minute_of_day(current_time) > minute_of_day(end_time) {
            let next_date = next_date(base.date());
            let next_time = task
                .start_time
                .as_deref()
                .map(parse_time)
                .transpose()?
                .unwrap_or_else(midnight_time);
            return Ok(NaiveDateTime::new(next_date, next_time));
        }
    }

    let mut next = base + Duration::minutes(task.interval_minutes.max(1));
    if let Some(end) = task.end_time.as_deref() {
        let end_time = parse_time(end)?;
        if minute_of_day(next.time()) > minute_of_day(end_time) {
            let next_date = next_date(base.date());
            let start_time = task
                .start_time
                .as_deref()
                .map(parse_time)
                .transpose()?
                .unwrap_or_else(midnight_time);
            next = NaiveDateTime::new(next_date, start_time);
        }
    }
    if let Some(start) = task.start_time.as_deref() {
        let start_time = parse_time(start)?;
        if minute_of_day(next.time()) < minute_of_day(start_time) {
            next = NaiveDateTime::new(next.date(), start_time);
        }
    }
    if next <= base {
        next = base + Duration::minutes(task.interval_minutes.max(1));
    }
    Ok(next)
}

fn compute_daily_next(
    task: &RecurringTask,
    base: NaiveDateTime,
) -> Result<NaiveDateTime, AppError> {
    let time = parse_time(
        task.schedule_time
            .as_deref()
            .ok_or_else(|| AppError::Invalid("每日模式缺少触发时间".to_string()))?,
    )?;
    let mut next = NaiveDateTime::new(base.date(), time);
    if next <= base {
        next = NaiveDateTime::new(next_date(base.date()), time);
    }
    Ok(next)
}

fn compute_weekly_next(
    task: &RecurringTask,
    base: NaiveDateTime,
) -> Result<NaiveDateTime, AppError> {
    let time = parse_time(
        task.schedule_time
            .as_deref()
            .ok_or_else(|| AppError::Invalid("每周模式缺少触发时间".to_string()))?,
    )?;
    let mask =
        weekday_mask(task).ok_or_else(|| AppError::Invalid("每周模式缺少周几".to_string()))?;

    // 从今天起最多看 8 天，一定能找到下一个选中的日子。
    for offset in 0..=7 {
        let date = base.date() + Duration::days(offset);
        let weekday = date.weekday().number_from_monday() as i64;
        let candidate = NaiveDateTime::new(date, time);
        if mask & weekday_bit(weekday) != 0 && candidate > base {
            return Ok(candidate);
        }
    }
    Err(AppError::Invalid(
        "每周模式无法计算下次触发时间".to_string(),
    ))
}

fn compute_workday_next(
    task: &RecurringTask,
    base: NaiveDateTime,
) -> Result<NaiveDateTime, AppError> {
    let time = parse_time(
        task.schedule_time
            .as_deref()
            .ok_or_else(|| AppError::Invalid("工作日模式缺少触发时间".to_string()))?,
    )?;
    // 最长的法定假期（含调休）不超过两周，向后查找 60 天足够。
    for offset in 0..=60 {
        let date = base.date() + Duration::days(offset);
        let candidate = NaiveDateTime::new(date, time);
        if candidate > base && holidays::is_workday(date) {
            return Ok(candidate);
        }
    }
    Err(AppError::Invalid(
        "工作日模式无法计算下次触发时间".to_string(),
    ))
}

fn compute_monthly_next(
    task: &RecurringTask,
    base: NaiveDateTime,
) -> Result<NaiveDateTime, AppError> {
    let time = parse_time(
        task.schedule_time
            .as_deref()
            .ok_or_else(|| AppError::Invalid("每月模式缺少触发时间".to_string()))?,
    )?;
    let day = task
        .schedule_day
        .ok_or_else(|| AppError::Invalid("每月模式缺少几号".to_string()))?;
    if !(1..=31).contains(&day) {
        return Err(AppError::Invalid(
            "每月模式中的几号必须在 1 到 31 之间".to_string(),
        ));
    }

    let mut candidate = month_datetime(base.date().year(), base.date().month(), day as u32, time)?;
    if candidate <= base {
        let (next_year, next_month) = next_month(base.date().year(), base.date().month());
        candidate = month_datetime(next_year, next_month, day as u32, time)?;
    }
    Ok(candidate)
}

fn compute_cron_next(task: &RecurringTask, base: NaiveDateTime) -> Result<NaiveDateTime, AppError> {
    let expr = task
        .cron_expression
        .as_deref()
        .ok_or_else(|| AppError::Invalid("Cron 模式缺少表达式".to_string()))?;
    let schedule_expr = cron_schedule_expr(expr)?;
    let schedule = Schedule::from_str(&schedule_expr)
        .map_err(|e| AppError::Invalid(format!("Cron 表达式无效: {}", e)))?;
    let local_base = Local
        .from_local_datetime(&base)
        .single()
        .or_else(|| Local.from_local_datetime(&base).earliest())
        .or_else(|| Local.from_local_datetime(&base).latest())
        .ok_or_else(|| AppError::Invalid("无法解析本地时间".to_string()))?;
    let next = schedule
        .after(&local_base)
        .next()
        .ok_or_else(|| AppError::Invalid("Cron 表达式没有未来触发时间".to_string()))?;
    Ok(next.naive_local())
}

fn sanitize_cron_expression(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Invalid("Cron 表达式不能为空".to_string()));
    }
    let schedule_expr = cron_schedule_expr(trimmed)?;
    let _ = Schedule::from_str(&schedule_expr)
        .map_err(|e| AppError::Invalid(format!("Cron 表达式无效: {}", e)))?;
    Ok(trimmed.to_string())
}

fn cron_schedule_expr(value: &str) -> Result<String, AppError> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    match parts.len() {
        5 => Ok(format!("0 {}", value)),
        6 | 7 => Ok(value.to_string()),
        _ => Err(AppError::Invalid(
            "Cron 表达式需为 5、6 或 7 段".to_string(),
        )),
    }
}

fn normalize_time_field(value: Option<&str>, field: &str) -> Result<Option<String>, AppError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = parse_time(trimmed)
        .map_err(|_| AppError::Invalid(format!("{}格式错误，应为 HH:mm，例如 08:30", field)))?;
    Ok(Some(parsed.format("%H:%M").to_string()))
}

fn normalize_text(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_time(value: &str) -> Result<NaiveTime, AppError> {
    NaiveTime::parse_from_str(value, "%H:%M").map_err(|e| AppError::Invalid(e.to_string()))
}

fn month_datetime(
    year: i32,
    month: u32,
    day: u32,
    time: NaiveTime,
) -> Result<NaiveDateTime, AppError> {
    let max_day = last_day_of_month(year, month)?;
    let actual_day = day.min(max_day);
    let date = NaiveDate::from_ymd_opt(year, month, actual_day)
        .ok_or_else(|| AppError::Invalid("无法生成每月触发日期".to_string()))?;
    Ok(NaiveDateTime::new(date, time))
}

fn last_day_of_month(year: i32, month: u32) -> Result<u32, AppError> {
    let (next_year, next_month) = next_month(year, month);
    let first_of_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .ok_or_else(|| AppError::Invalid("无法解析月份".to_string()))?;
    Ok((first_of_next - Duration::days(1)).day())
}

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

fn next_date(date: NaiveDate) -> NaiveDate {
    date.succ_opt().unwrap_or(date)
}

fn midnight_time() -> NaiveTime {
    NaiveTime::from_hms_opt(0, 0, 0).unwrap_or(NaiveTime::MIN)
}

fn minute_of_day(time: NaiveTime) -> i32 {
    (time.hour() as i32) * 60 + (time.minute() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(value: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M").unwrap()
    }

    fn task(mode: &str) -> RecurringTask {
        RecurringTask {
            id: "t".to_string(),
            description: "test".to_string(),
            task_type: "RECURRING".to_string(),
            status: "PENDING".to_string(),
            created_at: "2026-01-01T00:00:00".to_string(),
            completed_at: None,
            reminder_time: None,
            updated_at: None,
            deleted_at: None,
            interval_minutes: 60,
            last_triggered: None,
            next_trigger: String::new(),
            is_paused: false,
            start_time: None,
            end_time: None,
            repeat_mode: mode.to_string(),
            schedule_time: Some("09:00".to_string()),
            schedule_weekday: None,
            schedule_weekdays: None,
            schedule_day: None,
            cron_expression: None,
        }
    }

    fn next(task: &RecurringTask, base: &str) -> String {
        compute_next_trigger(task, Some(dt(base))).unwrap()
    }

    #[test]
    fn weekly_multiple_days() {
        let mut t = task(REPEAT_MODE_WEEKLY);
        // 周一、三、五
        t.schedule_weekdays = Some(weekday_bit(1) | weekday_bit(3) | weekday_bit(5));
        // 2026-09-21 为周一
        assert_eq!(next(&t, "2026-09-21T08:00"), "2026-09-21T09:00:00");
        assert_eq!(next(&t, "2026-09-21T09:00"), "2026-09-23T09:00:00");
        assert_eq!(next(&t, "2026-09-25T10:00"), "2026-09-28T09:00:00");
    }

    #[test]
    fn weekly_legacy_single_weekday_still_works() {
        let mut t = task(REPEAT_MODE_WEEKLY);
        t.schedule_weekday = Some(7); // 周日
        assert_eq!(next(&t, "2026-09-21T08:00"), "2026-09-27T09:00:00");
        sanitize_recurring_task(&mut t).unwrap();
        assert_eq!(t.schedule_weekdays, Some(weekday_bit(7)));
    }

    #[test]
    fn weekly_sanitize_keeps_first_day_for_old_clients() {
        let mut t = task(REPEAT_MODE_WEEKLY);
        t.schedule_weekday = Some(1);
        t.schedule_weekdays = Some(weekday_bit(3) | weekday_bit(6));
        sanitize_recurring_task(&mut t).unwrap();
        assert_eq!(t.schedule_weekday, Some(3));
        assert_eq!(t.schedule_weekdays, Some(weekday_bit(3) | weekday_bit(6)));
    }

    #[test]
    fn weekly_requires_a_day() {
        let mut t = task(REPEAT_MODE_WEEKLY);
        t.schedule_weekdays = Some(0);
        assert!(sanitize_recurring_task(&mut t).is_err());
        t.schedule_weekdays = Some(1 << 7);
        assert!(sanitize_recurring_task(&mut t).is_err());
    }

    #[test]
    fn other_modes_clear_weekday_mask() {
        let mut t = task(REPEAT_MODE_DAILY);
        t.schedule_weekdays = Some(WEEKDAY_MASK_ALL);
        sanitize_recurring_task(&mut t).unwrap();
        assert_eq!(t.schedule_weekdays, None);
    }

    #[test]
    fn workday_skips_holidays_and_includes_makeup_days() {
        let t = task(REPEAT_MODE_WORKDAY);
        // 2026-09-18（周五）之后：19 日周六、20 日周日调休上班。
        assert_eq!(next(&t, "2026-09-18T10:00"), "2026-09-20T09:00:00");
        // 9 月 24 日（周四）之后：25–27 中秋放假，28 日（周一）上班。
        assert_eq!(next(&t, "2026-09-24T10:00"), "2026-09-28T09:00:00");
        // 9 月 30 日之后：10 月 1–7 日国庆放假，8 日上班。
        assert_eq!(next(&t, "2026-09-30T10:00"), "2026-10-08T09:00:00");
        // 10 月 9 日（周五）之后：10 日周六调休上班。
        assert_eq!(next(&t, "2026-10-09T10:00"), "2026-10-10T09:00:00");
    }

    #[test]
    fn workday_mode_is_normalized_and_requires_time() {
        assert_eq!(normalize_repeat_mode("workday"), REPEAT_MODE_WORKDAY);
        let mut t = task(REPEAT_MODE_WORKDAY);
        t.schedule_time = None;
        assert!(sanitize_recurring_task(&mut t).is_err());
    }

    #[test]
    fn monthly_clamps_to_month_end() {
        let mut t = task(REPEAT_MODE_MONTHLY);
        t.schedule_day = Some(31);
        assert_eq!(next(&t, "2026-02-10T10:00"), "2026-02-28T09:00:00");
        assert_eq!(next(&t, "2026-02-28T10:00"), "2026-03-31T09:00:00");
    }
}
