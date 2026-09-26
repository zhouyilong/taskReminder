use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde_json::Value;

/// 内置的中国法定节假日数据。
const EMBEDDED_HOLIDAYS: &str = include_str!("../data/holidays-cn.json");
/// 数据目录下的覆盖文件名：新年度安排发布后，无需发版即可放置该文件补充数据。
pub const OVERRIDE_FILE_NAME: &str = "holidays-cn.json";

#[derive(Default)]
struct HolidayCalendar {
    off: HashSet<NaiveDate>,
    work: HashSet<NaiveDate>,
    years: BTreeSet<i32>,
}

impl HolidayCalendar {
    fn from_json(raw: &str) -> Result<Self, String> {
        let mut calendar = Self::default();
        calendar.merge_json(raw)?;
        Ok(calendar)
    }

    /// 合并一份 JSON 数据；同一年份以后合并的为准。
    fn merge_json(&mut self, raw: &str) -> Result<(), String> {
        let root: HashMap<String, Value> =
            serde_json::from_str(raw).map_err(|e| format!("节假日数据格式错误: {}", e))?;
        for (key, entry) in root {
            let Ok(year) = key.parse::<i32>() else {
                continue;
            };
            let off = parse_dates(entry.get("off"))?;
            let work = parse_dates(entry.get("work"))?;
            self.off.retain(|date| date.year() != year);
            self.work.retain(|date| date.year() != year);
            self.off.extend(off);
            self.work.extend(work);
            self.years.insert(year);
        }
        Ok(())
    }

    fn is_workday(&self, date: NaiveDate) -> bool {
        if self.work.contains(&date) {
            return true;
        }
        if self.off.contains(&date) {
            return false;
        }
        !matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
    }
}

fn parse_date(value: &Value) -> Result<NaiveDate, String> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("无效日期: {}", value))?;
    NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|_| format!("无效日期: {}", text))
}

fn parse_dates(value: Option<&Value>) -> Result<Vec<NaiveDate>, String> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut dates = Vec::new();
    for item in items {
        match item {
            Value::Array(range) if range.len() == 2 => {
                let start = parse_date(&range[0])?;
                let end = parse_date(&range[1])?;
                if end < start {
                    return Err(format!("日期区间倒置: {} ~ {}", start, end));
                }
                let mut day = start;
                while day <= end {
                    dates.push(day);
                    day += Duration::days(1);
                }
            }
            other => dates.push(parse_date(other)?),
        }
    }
    Ok(dates)
}

fn calendar() -> &'static RwLock<HolidayCalendar> {
    static CALENDAR: OnceLock<RwLock<HolidayCalendar>> = OnceLock::new();
    CALENDAR.get_or_init(|| {
        RwLock::new(HolidayCalendar::from_json(EMBEDDED_HOLIDAYS).unwrap_or_default())
    })
}

/// 启动时加载数据目录中的覆盖文件（若存在），与内置数据按年份合并。
pub fn load_override(data_dir: &Path) {
    let path = data_dir.join(OVERRIDE_FILE_NAME);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut guard = calendar().write().unwrap_or_else(|e| e.into_inner());
    if let Err(err) = guard.merge_json(&raw) {
        eprintln!("[holidays] 忽略无效的覆盖文件 {}: {}", path.display(), err);
    }
}

/// 是否为中国法定工作日：调休上班日为工作日，法定假日为休息日，
/// 其余按周一至周五计算（没有数据的年份也按周一至周五）。
pub fn is_workday(date: NaiveDate) -> bool {
    calendar()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .is_workday(date)
}

/// 已有节假日数据的年份。
pub fn covered_years() -> Vec<i32> {
    calendar()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .years
        .iter()
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn embedded_data_is_valid() {
        let calendar = HolidayCalendar::from_json(EMBEDDED_HOLIDAYS).unwrap();
        assert!(calendar.years.contains(&2025));
        assert!(calendar.years.contains(&2026));
    }

    #[test]
    fn spring_festival_2026() {
        let calendar = HolidayCalendar::from_json(EMBEDDED_HOLIDAYS).unwrap();
        // 2 月 14 日（周六）调休上班，15–23 日放假，24 日（周二）恢复上班。
        assert!(calendar.is_workday(d("2026-02-13")));
        assert!(calendar.is_workday(d("2026-02-14")));
        for day in 15..=23 {
            assert!(!calendar.is_workday(d(&format!("2026-02-{:02}", day))));
        }
        assert!(calendar.is_workday(d("2026-02-24")));
        assert!(calendar.is_workday(d("2026-02-28")));
    }

    #[test]
    fn national_day_2026() {
        let calendar = HolidayCalendar::from_json(EMBEDDED_HOLIDAYS).unwrap();
        assert!(calendar.is_workday(d("2026-09-20"))); // 周日调休上班
        assert!(!calendar.is_workday(d("2026-09-25"))); // 中秋
        assert!(!calendar.is_workday(d("2026-10-01")));
        assert!(!calendar.is_workday(d("2026-10-07")));
        assert!(calendar.is_workday(d("2026-10-08")));
        assert!(calendar.is_workday(d("2026-10-10"))); // 周六调休上班
    }

    #[test]
    fn uncovered_year_falls_back_to_weekdays() {
        let calendar = HolidayCalendar::from_json(EMBEDDED_HOLIDAYS).unwrap();
        assert!(calendar.is_workday(d("2030-01-07"))); // 周一
        assert!(!calendar.is_workday(d("2030-01-05"))); // 周六
    }

    #[test]
    fn override_replaces_same_year() {
        let mut calendar = HolidayCalendar::from_json(EMBEDDED_HOLIDAYS).unwrap();
        calendar
            .merge_json(r#"{"2026": {"off": ["2026-03-02"], "work": []}}"#)
            .unwrap();
        assert!(!calendar.is_workday(d("2026-03-02")));
        // 覆盖后 2026 年原有数据被替换，春节按普通周一至周五计算。
        assert!(calendar.is_workday(d("2026-02-16")));
        assert!(calendar.years.contains(&2025));
    }

    #[test]
    fn rejects_invalid_data() {
        assert!(HolidayCalendar::from_json(r#"{"2026": {"off": ["bad"]}}"#).is_err());
        assert!(
            HolidayCalendar::from_json(r#"{"2026": {"off": [["2026-02-02", "2026-02-01"]]}}"#)
                .is_err()
        );
    }
}
