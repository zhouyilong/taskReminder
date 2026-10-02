use std::str::FromStr;

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike};
use cron::Schedule;

use crate::errors::AppError;
use crate::holidays;
use crate::kinds::RepeatMode;
use crate::models::RecurringTask;
use crate::time;

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
pub fn weekday_mask(task: &RecurringTask) -> Option<i64> {
    match task.schedule_weekdays {
        Some(mask) if mask & WEEKDAY_MASK_ALL != 0 => Some(mask & WEEKDAY_MASK_ALL),
        _ => task
            .schedule_weekday
            .filter(|day| (1..=7).contains(day))
            .map(weekday_bit),
    }
}

/// 本版本不认识的循环模式（通常来自更新版本的设备）：不计算、不触发，也不改写。
fn unsupported_mode(mode: &str) -> AppError {
    AppError::Invalid(format!("此版本不支持的循环模式：{}，请升级应用", mode))
}

pub fn sanitize_recurring_task(task: &mut RecurringTask) -> Result<(), AppError> {
    // 不认识的模式原样保留：先报错返回，不清理任何字段（新版本的模式可能用到它们）。
    // 以前会回退成区间间隔并清掉其他字段，写回后经同步改坏新版本设备上的数据。
    if let RepeatMode::Unknown(mode) = &task.repeat_mode {
        return Err(unsupported_mode(mode));
    }
    task.description = task.description.trim().to_string();
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

    match &task.repeat_mode {
        RepeatMode::IntervalRange => {
            task.schedule_time = None;
            task.schedule_weekday = None;
            task.schedule_weekdays = None;
            task.schedule_day = None;
            task.cron_expression = None;
        }
        RepeatMode::Daily => {
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
        RepeatMode::Weekly => {
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
        RepeatMode::Monthly => {
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
        RepeatMode::Workday => {
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
        RepeatMode::Cron => {
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
        RepeatMode::Unknown(mode) => return Err(unsupported_mode(mode)),
    }
    Ok(())
}

pub fn compute_next_trigger(
    task: &RecurringTask,
    base: Option<NaiveDateTime>,
) -> Result<String, AppError> {
    let mut normalized = task.clone();
    sanitize_recurring_task(&mut normalized)?;

    let base = base.unwrap_or_else(time::now);
    let next = match &normalized.repeat_mode {
        RepeatMode::IntervalRange => compute_interval_next(&normalized, base)?,
        RepeatMode::Daily => compute_daily_next(&normalized, base)?,
        RepeatMode::Weekly => compute_weekly_next(&normalized, base)?,
        RepeatMode::Monthly => compute_monthly_next(&normalized, base)?,
        RepeatMode::Cron => compute_cron_next(&normalized, base)?,
        RepeatMode::Workday => compute_workday_next(&normalized, base)?,
        RepeatMode::Unknown(mode) => return Err(unsupported_mode(mode)),
    };
    Ok(time::format_datetime(&next))
}

/// “跳过本次”：返回跳过即将到来的这一次之后的下一次触发时间。
///
/// 要跳过的是当前的 `next_trigger`（已过期还没触发的也算这一次）；按当前规则从
/// `max(next_trigger, now)` 往后推算。暂停或不认识模式的提醒不能跳过。
pub fn skipped_trigger(task: &RecurringTask, now: NaiveDateTime) -> Result<String, AppError> {
    if task.is_paused {
        return Err(AppError::Invalid("已暂停的循环提醒不需要跳过".to_string()));
    }
    let current = time::parse_datetime_any(&task.next_trigger).unwrap_or(now);
    let base = current.max(now);
    let next = compute_next_trigger(task, Some(base))?;
    match time::parse_datetime_any(&next) {
        Some(value) if value > current => Ok(next),
        _ => Err(AppError::Invalid("无法计算下一次提醒时间".to_string())),
    }
}

/// 节假日数据更新后，“法定工作日”提醒的下次触发可能变化（例如新公布的调休上班日）。
/// 返回需要改成的时间；不是运行中的法定工作日提醒、已经到点等待触发（交给巡检处理，
/// 避免跳过一次提醒）或时间不变时返回 None。
pub fn refreshed_workday_trigger(
    task: &RecurringTask,
    now: NaiveDateTime,
) -> Result<Option<String>, AppError> {
    if task.is_paused || task.deleted_at.is_some() || task.repeat_mode != RepeatMode::Workday {
        return Ok(None);
    }
    let pending = time::parse_datetime_any(&task.next_trigger);
    if pending.is_some_and(|scheduled| scheduled <= now) {
        return Ok(None);
    }
    let next = compute_next_trigger(task, Some(now))?;
    let unchanged = match (pending, time::parse_datetime_any(&next)) {
        (Some(current), Some(recomputed)) => current == recomputed,
        _ => next == task.next_trigger,
    };
    Ok(if unchanged { None } else { Some(next) })
}

/// 预估 `until`（含）之前的触发时间，用于“今天”视图展示即将到来的循环提醒。
///
/// 从任务当前的 `next_trigger` 开始，按调度器的方式（以上一次触发时间为基准）
/// 逐个推算，最多返回 `limit` 个。暂停的任务返回空列表。
pub fn upcoming_triggers(
    task: &RecurringTask,
    until: NaiveDateTime,
    limit: usize,
) -> Result<Vec<String>, AppError> {
    let mut result = Vec::new();
    // 不认识的模式本机不会触发，也不预估。
    if task.is_paused || limit == 0 || !task.repeat_mode.is_known() {
        return Ok(result);
    }
    let Some(mut current) = time::parse_datetime_any(&task.next_trigger) else {
        return Ok(result);
    };
    while current <= until && result.len() < limit {
        result.push(time::format_datetime(&current));
        let next = compute_next_trigger(task, Some(current))?;
        match time::parse_datetime_any(&next) {
            Some(value) if value > current => current = value,
            _ => break,
        }
    }
    Ok(result)
}

pub fn should_trigger_now(task: &RecurringTask, now: NaiveDateTime) -> Result<bool, AppError> {
    let mut normalized = task.clone();
    sanitize_recurring_task(&mut normalized)?;
    if normalized.repeat_mode != RepeatMode::IntervalRange {
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

/// 转成 `cron` crate 的表达式（秒 分 时 日 月 周 [年]）。
///
/// 5 段按标准 Unix Cron 理解：周字段 0 和 7 都是周日、1 是周一。`cron` crate 的数字周几是
/// 1 = 周日 … 7 = 周六，直接透传会让 `1-5` 变成周日到周四，所以把数字周几换成英文缩写。
/// 6、7 段沿用 `cron` crate 的写法，原样透传。
fn cron_schedule_expr(value: &str) -> Result<String, AppError> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    match parts.len() {
        5 => {
            let weekday = unix_weekday_field(parts[4])?;
            Ok(format!(
                "0 {} {} {} {} {}",
                parts[0], parts[1], parts[2], parts[3], weekday
            ))
        }
        6 | 7 => Ok(parts.join(" ")),
        _ => Err(AppError::Invalid(
            "Cron 表达式需为 5、6 或 7 段".to_string(),
        )),
    }
}

const CRON_WEEKDAY_NAMES: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

/// 把 Unix Cron 的周字段（0/7 = 周日）中的数字换成英文缩写；`*`、`?` 与英文缩写原样保留。
fn unix_weekday_field(field: &str) -> Result<String, AppError> {
    let invalid = || AppError::Invalid(format!("Cron 表达式的周字段无效: {}", field));
    let mut items = Vec::new();
    for item in field.split(',') {
        if item.is_empty() {
            return Err(invalid());
        }
        let (base, step) = match item.split_once('/') {
            Some((base, step)) => (
                base,
                Some(
                    step.parse::<u32>()
                        .ok()
                        .filter(|v| *v > 0)
                        .ok_or_else(invalid)?,
                ),
            ),
            None => (item, None),
        };
        let numeric = base
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '*');
        if !numeric || (base == "*" && step.is_none()) || base == "?" {
            items.push(item.to_string());
            continue;
        }
        let parse_day = |raw: &str| {
            raw.parse::<u32>()
                .ok()
                .filter(|v| *v <= 7)
                .ok_or_else(invalid)
        };
        let (start, end) = if base == "*" {
            (0, 6)
        } else if let Some((a, b)) = base.split_once('-') {
            (parse_day(a)?, parse_day(b)?)
        } else {
            let day = parse_day(base)?;
            // 单个数字带步长（如 1/2）表示从该天起到周六。
            (day, if step.is_some() { 6 } else { day })
        };
        if start > end {
            return Err(invalid());
        }
        let step = step.unwrap_or(1) as usize;
        for day in (start..=end).step_by(step) {
            items.push(CRON_WEEKDAY_NAMES[(day % 7) as usize].to_string());
        }
    }
    Ok(items.join(","))
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
    Ok(Some(time::format_clock(&parsed)))
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
    time::parse_clock(value).ok_or_else(|| AppError::Invalid(format!("无法解析时间: {}", value)))
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
    use crate::kinds::{TaskStatus, TaskType};

    fn dt(value: &str) -> NaiveDateTime {
        time::parse_datetime_any(value).unwrap()
    }

    fn task(mode: RepeatMode) -> RecurringTask {
        RecurringTask {
            id: "t".to_string(),
            description: "test".to_string(),
            task_type: TaskType::Recurring,
            status: TaskStatus::Pending,
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
            repeat_mode: mode,
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
        let mut t = task(RepeatMode::Weekly);
        // 周一、三、五
        t.schedule_weekdays = Some(weekday_bit(1) | weekday_bit(3) | weekday_bit(5));
        // 2026-09-21 为周一
        assert_eq!(next(&t, "2026-09-21T08:00"), "2026-09-21T09:00:00");
        assert_eq!(next(&t, "2026-09-21T09:00"), "2026-09-23T09:00:00");
        assert_eq!(next(&t, "2026-09-25T10:00"), "2026-09-28T09:00:00");
    }

    #[test]
    fn weekly_legacy_single_weekday_still_works() {
        let mut t = task(RepeatMode::Weekly);
        t.schedule_weekday = Some(7); // 周日
        assert_eq!(next(&t, "2026-09-21T08:00"), "2026-09-27T09:00:00");
        sanitize_recurring_task(&mut t).unwrap();
        assert_eq!(t.schedule_weekdays, Some(weekday_bit(7)));
    }

    #[test]
    fn weekly_sanitize_keeps_first_day_for_old_clients() {
        let mut t = task(RepeatMode::Weekly);
        t.schedule_weekday = Some(1);
        t.schedule_weekdays = Some(weekday_bit(3) | weekday_bit(6));
        sanitize_recurring_task(&mut t).unwrap();
        assert_eq!(t.schedule_weekday, Some(3));
        assert_eq!(t.schedule_weekdays, Some(weekday_bit(3) | weekday_bit(6)));
    }

    #[test]
    fn weekly_requires_a_day() {
        let mut t = task(RepeatMode::Weekly);
        t.schedule_weekdays = Some(0);
        assert!(sanitize_recurring_task(&mut t).is_err());
        t.schedule_weekdays = Some(1 << 7);
        assert!(sanitize_recurring_task(&mut t).is_err());
    }

    #[test]
    fn other_modes_clear_weekday_mask() {
        let mut t = task(RepeatMode::Daily);
        t.schedule_weekdays = Some(WEEKDAY_MASK_ALL);
        sanitize_recurring_task(&mut t).unwrap();
        assert_eq!(t.schedule_weekdays, None);
    }

    #[test]
    fn workday_skips_holidays_and_includes_makeup_days() {
        let t = task(RepeatMode::Workday);
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
    fn refreshed_workday_trigger_picks_up_new_holiday_data() {
        let mut t = task(RepeatMode::Workday);
        // 按没有节假日数据时算出的下次触发：10 月 9 日之后为 10 月 12 日（周一）。
        // 有了数据后，10 日周六调休上班，应改到 10 日。
        t.next_trigger = "2026-10-12T09:00:00".to_string();
        assert_eq!(
            refreshed_workday_trigger(&t, dt("2026-10-09T10:00")).unwrap(),
            Some("2026-10-10T09:00:00".to_string())
        );
        // 已是正确时间：不改，避免产生同步改动。
        t.next_trigger = "2026-10-10T09:00:00".to_string();
        assert_eq!(
            refreshed_workday_trigger(&t, dt("2026-10-09T10:00")).unwrap(),
            None
        );
        // 已经到点、等待巡检触发：不改，免得跳过这次提醒。
        t.next_trigger = "2026-10-09T09:00:00".to_string();
        assert_eq!(
            refreshed_workday_trigger(&t, dt("2026-10-09T10:00")).unwrap(),
            None
        );
        // 暂停的、其他模式的不处理。
        t.next_trigger = "2026-10-12T09:00:00".to_string();
        t.is_paused = true;
        assert_eq!(
            refreshed_workday_trigger(&t, dt("2026-10-09T10:00")).unwrap(),
            None
        );
        let mut daily = task(RepeatMode::Daily);
        daily.next_trigger = "2026-10-12T09:00:00".to_string();
        assert_eq!(
            refreshed_workday_trigger(&daily, dt("2026-10-09T10:00")).unwrap(),
            None
        );
    }

    #[test]
    fn unknown_mode_is_rejected_without_touching_the_task() {
        // 例如 v2.1 新增的模式经同步来到本机：以前会被改成区间间隔、清掉其他字段并写回。
        let mut t = task(RepeatMode::Unknown("BIWEEKLY".to_string()));
        t.schedule_time = Some("09:00".to_string());
        t.schedule_weekdays = Some(0b1);
        t.cron_expression = Some("custom".to_string());
        t.next_trigger = "2026-10-12T09:00:00".to_string();
        let before = serde_json::to_string(&t).unwrap();

        let mut sanitized = t.clone();
        let err = sanitize_recurring_task(&mut sanitized)
            .unwrap_err()
            .to_string();
        assert!(err.contains("BIWEEKLY") && err.contains("升级"), "{}", err);
        assert_eq!(serde_json::to_string(&sanitized).unwrap(), before);

        assert!(compute_next_trigger(&t, Some(dt("2026-10-09T10:00"))).is_err());
        assert!(should_trigger_now(&t, dt("2026-10-09T10:00")).is_err());
        assert!(upcoming_triggers(&t, dt("2026-12-31T00:00"), 10)
            .unwrap()
            .is_empty());
        assert_eq!(
            refreshed_workday_trigger(&t, dt("2026-10-09T10:00")).unwrap(),
            None
        );
    }

    #[test]
    fn workday_mode_is_normalized_and_requires_time() {
        assert_eq!(RepeatMode::parse("workday"), RepeatMode::Workday);
        let mut t = task(RepeatMode::Workday);
        t.schedule_time = None;
        assert!(sanitize_recurring_task(&mut t).is_err());
    }

    #[test]
    fn upcoming_triggers_until_end_of_day() {
        let mut t = task(RepeatMode::IntervalRange);
        t.interval_minutes = 120;
        t.start_time = Some("08:00".to_string());
        t.end_time = Some("17:00".to_string());
        t.next_trigger = "2026-09-21T13:00:00".to_string();
        let times = upcoming_triggers(&t, dt("2026-09-21T23:59"), 10).unwrap();
        assert_eq!(
            times,
            vec![
                "2026-09-21T13:00:00",
                "2026-09-21T15:00:00",
                "2026-09-21T17:00:00"
            ]
        );
        assert_eq!(
            upcoming_triggers(&t, dt("2026-09-21T23:59"), 2)
                .unwrap()
                .len(),
            2
        );

        let mut daily = task(RepeatMode::Daily);
        daily.next_trigger = "2026-09-22T09:00:00".to_string();
        assert!(upcoming_triggers(&daily, dt("2026-09-21T23:59"), 10)
            .unwrap()
            .is_empty());

        t.is_paused = true;
        assert!(upcoming_triggers(&t, dt("2026-09-21T23:59"), 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn monthly_clamps_to_month_end() {
        let mut t = task(RepeatMode::Monthly);
        t.schedule_day = Some(31);
        assert_eq!(next(&t, "2026-02-10T10:00"), "2026-02-28T09:00:00");
        assert_eq!(next(&t, "2026-02-28T10:00"), "2026-03-31T09:00:00");
    }

    fn cron(expr: &str) -> RecurringTask {
        let mut t = task(RepeatMode::Cron);
        t.schedule_time = None;
        t.cron_expression = Some(expr.to_string());
        t
    }

    #[test]
    fn cron_five_fields_use_unix_weekdays() {
        // 2026-10-01 为周四。
        assert_eq!(
            next(&cron("0 9 * * 1-5"), "2026-10-01T10:00"),
            "2026-10-02T09:00:00"
        );
        assert_eq!(
            next(&cron("0 9 * * 1-5"), "2026-10-02T10:00"),
            "2026-10-05T09:00:00"
        );
        assert_eq!(
            next(&cron("0 9 * * 5"), "2026-10-01T10:00"),
            "2026-10-02T09:00:00"
        );
        // 0 与 7 都是周日。
        assert_eq!(
            next(&cron("0 9 * * 0"), "2026-10-01T10:00"),
            "2026-10-04T09:00:00"
        );
        assert_eq!(
            next(&cron("0 9 * * 7"), "2026-10-01T10:00"),
            "2026-10-04T09:00:00"
        );
        // 跨到周日的区间、列表与步长（*/3 为周日、周三、周六）。
        assert_eq!(
            next(&cron("0 9 * * 6-7"), "2026-10-01T10:00"),
            "2026-10-03T09:00:00"
        );
        assert_eq!(
            next(&cron("0 9 * * 1,3,5"), "2026-10-01T10:00"),
            "2026-10-02T09:00:00"
        );
        assert_eq!(
            next(&cron("0 9 * * 1-5/2"), "2026-10-02T10:00"),
            "2026-10-05T09:00:00"
        );
        assert_eq!(
            next(&cron("0 9 * * */3"), "2026-10-01T10:00"),
            "2026-10-03T09:00:00"
        );
        // 英文缩写不受影响。
        assert_eq!(
            next(&cron("0 9 * * MON-FRI"), "2026-10-01T10:00"),
            "2026-10-02T09:00:00"
        );
        assert_eq!(
            next(&cron("0 9 * * SUN"), "2026-10-01T10:00"),
            "2026-10-04T09:00:00"
        );
    }

    #[test]
    fn cron_weekday_field_translation() {
        assert_eq!(unix_weekday_field("*").unwrap(), "*");
        assert_eq!(unix_weekday_field("?").unwrap(), "?");
        assert_eq!(unix_weekday_field("1-5").unwrap(), "MON,TUE,WED,THU,FRI");
        assert_eq!(unix_weekday_field("0,7").unwrap(), "SUN,SUN");
        assert_eq!(unix_weekday_field("*/2").unwrap(), "SUN,TUE,THU,SAT");
        assert_eq!(unix_weekday_field("MON-FRI").unwrap(), "MON-FRI");
        assert!(unix_weekday_field("8").is_err());
        assert!(unix_weekday_field("5-1").is_err());
        assert!(unix_weekday_field("1/0").is_err());
        assert!(unix_weekday_field("1,,2").is_err());
    }

    #[test]
    fn cron_six_and_seven_fields_pass_through() {
        // 6 段带秒，沿用 cron crate 写法。
        assert_eq!(
            next(&cron("30 0 9 * * *"), "2026-10-01T10:00"),
            "2026-10-02T09:00:30"
        );
        // 7 段带年份（cron crate 对跨到未来年份的计算不可靠，这里只验证包含当年的范围）。
        assert_eq!(
            next(&cron("0 0 9 * * * 2026-2030"), "2026-10-01T10:00"),
            "2026-10-02T09:00:00"
        );
    }

    #[test]
    fn cron_calendar_edges() {
        // 每月 31 日跳过小月。
        assert_eq!(
            next(&cron("0 9 31 * *"), "2026-09-01T10:00"),
            "2026-10-31T09:00:00"
        );
        // 2 月 29 日只在闰年触发。
        assert_eq!(
            next(&cron("0 9 29 2 *"), "2026-10-01T10:00"),
            "2028-02-29T09:00:00"
        );
        // 跨年。
        assert_eq!(
            next(&cron("0 0 1 1 *"), "2026-12-31T23:30"),
            "2027-01-01T00:00:00"
        );
        // 分钟步长；恰好在触发时刻时取下一次。
        assert_eq!(
            next(&cron("*/15 * * * *"), "2026-10-01T10:00"),
            "2026-10-01T10:15:00"
        );
        assert_eq!(
            next(&cron("*/15 * * * *"), "2026-10-01T10:07"),
            "2026-10-01T10:15:00"
        );
    }

    #[test]
    fn cron_sanitize_rejects_invalid_expressions() {
        for expr in [
            "",
            "   ",
            "0 9 * *",
            "0 9 * * * * * *",
            "61 9 * * *",
            "0 25 * * *",
            "0 9 * * 8",
            "abc def ghi jkl mno",
        ] {
            let mut t = cron(expr);
            assert!(sanitize_recurring_task(&mut t).is_err(), "{expr:?}");
        }
        // 缺少表达式。
        let mut t = task(RepeatMode::Cron);
        t.cron_expression = None;
        assert!(sanitize_recurring_task(&mut t).is_err());
        // 没有未来触发时间。
        assert!(
            compute_next_trigger(&cron("0 0 9 1 1 * 2020"), Some(dt("2026-10-01T10:00"))).is_err()
        );
    }

    #[test]
    fn cron_sanitize_trims_and_clears_other_fields() {
        let mut t = cron("  0 9 * * 1-5  ");
        t.schedule_time = Some("08:00".to_string());
        t.schedule_day = Some(3);
        t.start_time = Some("08:00".to_string());
        sanitize_recurring_task(&mut t).unwrap();
        assert_eq!(t.cron_expression.as_deref(), Some("0 9 * * 1-5"));
        assert_eq!(t.schedule_time, None);
        assert_eq!(t.schedule_day, None);
        assert_eq!(t.start_time, None);
    }

    #[test]
    fn skip_moves_to_the_occurrence_after_next() {
        // 每天 09:00，下一次是明天 09:00：跳过后为后天。
        let mut daily = task(RepeatMode::Daily);
        daily.next_trigger = "2026-09-22T09:00:00".to_string();
        assert_eq!(
            skipped_trigger(&daily, dt("2026-09-21T20:00")).unwrap(),
            "2026-09-23T09:00:00"
        );
        // 已到点还没触发的那一次也算“本次”。
        daily.next_trigger = "2026-09-21T09:00:00".to_string();
        assert_eq!(
            skipped_trigger(&daily, dt("2026-09-21T09:00:30")).unwrap(),
            "2026-09-22T09:00:00"
        );

        // 每周一、三、五：周一的这次跳过后是周三。
        let mut weekly = task(RepeatMode::Weekly);
        weekly.schedule_weekdays = Some(weekday_bit(1) | weekday_bit(3) | weekday_bit(5));
        weekly.next_trigger = "2026-09-21T09:00:00".to_string();
        assert_eq!(
            skipped_trigger(&weekly, dt("2026-09-20T12:00")).unwrap(),
            "2026-09-23T09:00:00"
        );

        // 法定工作日：9 月 30 日之后是国庆假期，跳过后到 10 月 8 日。
        let mut workday = task(RepeatMode::Workday);
        workday.next_trigger = "2026-09-30T09:00:00".to_string();
        assert_eq!(
            skipped_trigger(&workday, dt("2026-09-29T18:00")).unwrap(),
            "2026-10-08T09:00:00"
        );

        // 区间间隔：每 60 分钟，跳过 10:00 的这一次。
        let mut interval = task(RepeatMode::IntervalRange);
        interval.schedule_time = None;
        interval.next_trigger = "2026-09-21T10:00:00".to_string();
        assert_eq!(
            skipped_trigger(&interval, dt("2026-09-21T09:30")).unwrap(),
            "2026-09-21T11:00:00"
        );

        // Cron：工作日 9 点（5 段，1-5 为周一到周五）；周五的这次跳过后到下周一。
        let mut cron = task(RepeatMode::Cron);
        cron.schedule_time = None;
        cron.cron_expression = Some("0 9 * * 1-5".to_string());
        cron.next_trigger = "2026-09-25T09:00:00".to_string();
        assert_eq!(
            skipped_trigger(&cron, dt("2026-09-24T12:00")).unwrap(),
            "2026-09-28T09:00:00"
        );
    }

    #[test]
    fn skip_rejects_paused_and_unknown_modes() {
        let mut paused = task(RepeatMode::Daily);
        paused.next_trigger = "2026-09-22T09:00:00".to_string();
        paused.is_paused = true;
        assert!(skipped_trigger(&paused, dt("2026-09-21T20:00")).is_err());

        let mut unknown = task(RepeatMode::parse("BIWEEKLY"));
        unknown.next_trigger = "2026-09-22T09:00:00".to_string();
        assert!(skipped_trigger(&unknown, dt("2026-09-21T20:00")).is_err());
    }
}
