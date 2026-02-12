use std::str::FromStr;

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use cron::Schedule;

use crate::errors::AppError;
use crate::models::RecurringTask;

pub const REPEAT_MODE_INTERVAL_RANGE: &str = "INTERVAL_RANGE";
pub const REPEAT_MODE_DAILY: &str = "DAILY";
pub const REPEAT_MODE_WEEKLY: &str = "WEEKLY";
pub const REPEAT_MODE_MONTHLY: &str = "MONTHLY";
pub const REPEAT_MODE_CRON: &str = "CRON";

pub fn normalize_repeat_mode(mode: &str) -> String {
    match mode.trim().to_uppercase().as_str() {
        REPEAT_MODE_DAILY => REPEAT_MODE_DAILY.to_string(),
        REPEAT_MODE_WEEKLY => REPEAT_MODE_WEEKLY.to_string(),
        REPEAT_MODE_MONTHLY => REPEAT_MODE_MONTHLY.to_string(),
        REPEAT_MODE_CRON => REPEAT_MODE_CRON.to_string(),
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
            task.schedule_day = None;
            task.cron_expression = None;
        }
        REPEAT_MODE_WEEKLY => {
            if task.schedule_time.is_none() {
                return Err(AppError::Invalid("每周模式需要设置触发时间".to_string()));
            }
            let weekday = task
                .schedule_weekday
                .ok_or_else(|| AppError::Invalid("每周模式需要设置周几".to_string()))?;
            if !(1..=7).contains(&weekday) {
                return Err(AppError::Invalid(
                    "每周模式中的周几必须在 1 到 7 之间".to_string(),
                ));
            }
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
        if now_time < parse_time(start)? {
            return Ok(false);
        }
    }
    if let Some(end) = normalized.end_time.as_deref() {
        if now_time > parse_time(end)? {
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
        if current_time < start_time {
            return Ok(NaiveDateTime::new(base.date(), start_time));
        }
    }

    if let Some(end) = task.end_time.as_deref() {
        let end_time = parse_time(end)?;
        if current_time > end_time {
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
        if next.time() > end_time {
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
        if next.time() < start_time {
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
    let weekday = task
        .schedule_weekday
        .ok_or_else(|| AppError::Invalid("每周模式缺少周几".to_string()))?;
    if !(1..=7).contains(&weekday) {
        return Err(AppError::Invalid(
            "每周模式中的周几必须在 1 到 7 之间".to_string(),
        ));
    }

    let current_weekday = base.weekday().number_from_monday() as i64;
    let mut days_ahead = (weekday - current_weekday + 7) % 7;
    let mut candidate_date = base.date() + Duration::days(days_ahead);
    let mut candidate = NaiveDateTime::new(candidate_date, time);
    if candidate <= base {
        days_ahead += 7;
        candidate_date = base.date() + Duration::days(days_ahead);
        candidate = NaiveDateTime::new(candidate_date, time);
    }
    Ok(candidate)
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
