use crate::time;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde_json::Value;

/// 内置的中国法定节假日数据。
const EMBEDDED_HOLIDAYS: &str = include_str!("../data/holidays-cn.json");
/// 数据目录下的覆盖文件名：用户手动放置，优先级最高。
pub const OVERRIDE_FILE_NAME: &str = "holidays-cn.json";
/// 在线更新下载的数据缓存（见 `holiday_update.rs`）。只取内置数据没有的年份，
/// 内置数据已有的年份以内置为准（更正随应用版本发布）；用户覆盖文件优先级最高。
pub const REMOTE_CACHE_FILE_NAME: &str = "holidays-cn.remote.json";
/// 接受的年份范围，防止异常数据。
const MIN_YEAR: i32 = 2000;
const MAX_YEAR: i32 = 2100;

#[derive(Default, Debug, PartialEq)]
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

    /// 按年份合并另一份数据：`other` 中的年份整体替换本数据中的同一年份。
    fn merge_calendar(&mut self, other: &HolidayCalendar) {
        for &year in &other.years {
            self.off.retain(|date| date.year() != year);
            self.work.retain(|date| date.year() != year);
            self.off.extend(Self::dates_in_year(&other.off, year));
            self.work.extend(Self::dates_in_year(&other.work, year));
            self.years.insert(year);
        }
    }

    /// 去掉 `known` 中已有的年份，只保留新的年份。
    fn without_years_of(mut self, known: &HolidayCalendar) -> HolidayCalendar {
        self.off.retain(|date| !known.years.contains(&date.year()));
        self.work.retain(|date| !known.years.contains(&date.year()));
        self.years.retain(|year| !known.years.contains(year));
        self
    }

    fn dates_in_year(dates: &HashSet<NaiveDate>, year: i32) -> HashSet<NaiveDate> {
        dates
            .iter()
            .copied()
            .filter(|date| date.year() == year)
            .collect()
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
    time::parse_date(text).ok_or_else(|| format!("无效日期: {}", text))
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
    CALENDAR.get_or_init(|| RwLock::new(embedded_calendar()))
}

fn embedded_calendar() -> HolidayCalendar {
    HolidayCalendar::from_json(EMBEDDED_HOLIDAYS).unwrap_or_default()
}

/// 校验在线下载的数据，返回其中内置数据没有的年份。
///
/// - 能完整解析，年份在合理范围内，至少有一个年份；
/// - 新年份都有放假日；
/// - 已缓存的年份不能消失或被删减（`cached` 为已有缓存中的新年份），这样下载到损坏或被
///   篡改的文件也不会把已有数据改少。内置数据已有的年份不看，始终以内置为准。
fn validate_remote(
    raw: &str,
    embedded: &HolidayCalendar,
    cached: Option<&HolidayCalendar>,
) -> Result<HolidayCalendar, String> {
    let remote = HolidayCalendar::from_json(raw)?;
    if remote.years.is_empty() {
        return Err("节假日数据为空".to_string());
    }
    if let Some(year) = remote
        .years
        .iter()
        .find(|year| !(MIN_YEAR..=MAX_YEAR).contains(*year))
    {
        return Err(format!("年份超出范围: {}", year));
    }
    let added = remote.without_years_of(embedded);
    for &year in &added.years {
        if HolidayCalendar::dates_in_year(&added.off, year).is_empty() {
            return Err(format!("{} 年没有放假日", year));
        }
    }
    if let Some(cached) = cached {
        for &year in &cached.years {
            if !added.years.contains(&year) {
                return Err(format!("缺少已有年份 {}", year));
            }
            let off = HolidayCalendar::dates_in_year(&added.off, year);
            let work = HolidayCalendar::dates_in_year(&added.work, year);
            if !off.is_superset(&HolidayCalendar::dates_in_year(&cached.off, year))
                || !work.is_superset(&HolidayCalendar::dates_in_year(&cached.work, year))
            {
                return Err(format!("{} 年的数据比现有数据少", year));
            }
        }
    }
    Ok(added)
}

/// 缓存中内置数据没有的年份；缓存不存在或损坏时为 None。
fn cached_years(data_dir: &Path, embedded: &HolidayCalendar) -> Option<HolidayCalendar> {
    let path = data_dir.join(REMOTE_CACHE_FILE_NAME);
    let raw = std::fs::read_to_string(&path).ok()?;
    match validate_remote(&raw, embedded, None) {
        Ok(added) => Some(added),
        Err(err) => {
            eprintln!("[holidays] 忽略节假日缓存 {}: {}", path.display(), err);
            None
        }
    }
}

/// 按优先级载入：内置数据 < 在线更新缓存中的新年份 < 用户覆盖文件（同一年份后者为准）。
fn build_calendar(data_dir: &Path) -> HolidayCalendar {
    let embedded = embedded_calendar();
    let mut calendar = embedded_calendar();
    if let Some(added) = cached_years(data_dir, &embedded) {
        calendar.merge_calendar(&added);
    }
    let path = data_dir.join(OVERRIDE_FILE_NAME);
    if let Ok(raw) = std::fs::read_to_string(&path) {
        // 先完整解析再合并，文件中途出错时不会留下合并了一半的数据。
        match HolidayCalendar::from_json(&raw) {
            Ok(user) => calendar.merge_calendar(&user),
            Err(err) => eprintln!("[holidays] 忽略无效的覆盖文件 {}: {}", path.display(), err),
        }
    }
    calendar
}

/// 启动时载入数据目录中的在线更新缓存与覆盖文件。
pub fn load(data_dir: &Path) {
    let calendar_data = build_calendar(data_dir);
    *calendar().write().unwrap_or_else(|e| e.into_inner()) = calendar_data;
}

/// 应用在线下载的数据：校验通过、且新年份与现有缓存不同时写入缓存并重新载入。
/// 返回生效的节假日数据是否因此改变（用户覆盖文件中的年份不受影响）。
pub fn apply_remote(data_dir: &Path, raw: &str) -> Result<bool, String> {
    let embedded = embedded_calendar();
    let cached = cached_years(data_dir, &embedded);
    let added = validate_remote(raw, &embedded, cached.as_ref())?;
    if cached.as_ref() == Some(&added) || (cached.is_none() && added.years.is_empty()) {
        return Ok(false);
    }
    // 先写临时文件再改名，写到一半断电也不会留下损坏的缓存。
    let cache_path = data_dir.join(REMOTE_CACHE_FILE_NAME);
    let temp_path = data_dir.join(format!("{}.tmp", REMOTE_CACHE_FILE_NAME));
    std::fs::write(&temp_path, raw).map_err(|e| format!("写入节假日缓存失败: {}", e))?;
    std::fs::rename(&temp_path, &cache_path).map_err(|e| format!("写入节假日缓存失败: {}", e))?;

    let updated = build_calendar(data_dir);
    let mut guard = calendar().write().unwrap_or_else(|e| e.into_inner());
    let changed = *guard != updated;
    *guard = updated;
    Ok(changed)
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
        time::parse_date(value).unwrap()
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

    /// 内置数据加上一段额外年份的 JSON（模拟仓库发布了新一年的安排）。
    fn embedded_plus(extra: serde_json::Value) -> String {
        let mut root: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(EMBEDDED_HOLIDAYS).unwrap();
        for (key, value) in extra.as_object().unwrap() {
            root.insert(key.clone(), value.clone());
        }
        serde_json::to_string(&root).unwrap()
    }

    fn with_2027() -> String {
        embedded_plus(serde_json::json!({
            "2027": { "off": ["2027-01-01", ["2027-02-06", "2027-02-12"]], "work": ["2027-02-20"] }
        }))
    }

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("holidays-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn remote_data_only_adds_new_years() {
        let embedded = embedded_calendar();
        let added = validate_remote(&with_2027(), &embedded, None).unwrap();
        assert_eq!(added.years.iter().copied().collect::<Vec<_>>(), vec![2027]);
        assert!(!added.is_workday(d("2027-02-08"))); // 周一，春节
        assert!(added.is_workday(d("2027-02-20"))); // 周六，调休上班
                                                    // 与内置数据相同：没有新年份，也不算错误。
        assert!(validate_remote(EMBEDDED_HOLIDAYS, &embedded, None)
            .unwrap()
            .years
            .is_empty());
        // 内置年份的内容不看：远端改动 2026 年不会生效，也不会被拒绝。
        let changed_2026 =
            embedded_plus(serde_json::json!({ "2026": { "off": ["2026-01-01"], "work": [] } }));
        assert!(validate_remote(&changed_2026, &embedded, None)
            .unwrap()
            .years
            .is_empty());

        // 已缓存的新年份不能消失或被删减。
        let cached = validate_remote(&with_2027(), &embedded, None).unwrap();
        assert!(validate_remote(EMBEDDED_HOLIDAYS, &embedded, Some(&cached))
            .unwrap_err()
            .contains("缺少已有年份 2027"));
        let reduced = embedded_plus(serde_json::json!({ "2027": { "off": ["2027-01-01"] } }));
        assert!(validate_remote(&reduced, &embedded, Some(&cached))
            .unwrap_err()
            .contains("2027"));

        // 新年份没有放假日、年份越界、格式错误、空文件。
        let empty_year = embedded_plus(serde_json::json!({ "2027": { "off": [], "work": [] } }));
        assert!(validate_remote(&empty_year, &embedded, None).is_err());
        let far = embedded_plus(serde_json::json!({ "2999": { "off": ["2999-01-01"] } }));
        assert!(validate_remote(&far, &embedded, None)
            .unwrap_err()
            .contains("超出范围"));
        assert!(validate_remote("<html>404</html>", &embedded, None).is_err());
        assert!(validate_remote("{}", &embedded, None).is_err());
    }

    #[test]
    fn override_file_wins_over_remote_cache() {
        let dir = temp_dir();
        std::fs::write(dir.join(REMOTE_CACHE_FILE_NAME), with_2027()).unwrap();
        std::fs::write(
            dir.join(OVERRIDE_FILE_NAME),
            r#"{"2027": {"off": ["2027-03-01"], "work": []}}"#,
        )
        .unwrap();
        let calendar = build_calendar(&dir);
        assert!(!calendar.is_workday(d("2027-03-01")));
        assert!(calendar.is_workday(d("2027-02-08"))); // 缓存中的 2027 被覆盖文件整体替换
        assert!(!calendar.is_workday(d("2026-10-01"))); // 其他年份不受影响

        // 缓存只补充新年份：缓存里改动过的内置年份不生效（更正随应用版本发布）。
        std::fs::remove_file(dir.join(OVERRIDE_FILE_NAME)).unwrap();
        let tampered = embedded_plus(serde_json::json!({
            "2026": { "off": ["2026-01-01"], "work": [] },
            "2027": { "off": ["2027-01-01"] }
        }));
        std::fs::write(dir.join(REMOTE_CACHE_FILE_NAME), tampered).unwrap();
        let calendar = build_calendar(&dir);
        assert!(!calendar.is_workday(d("2026-10-01")));
        assert!(!calendar.is_workday(d("2027-01-01")));

        // 缓存损坏时忽略，退回内置数据。
        std::fs::write(dir.join(REMOTE_CACHE_FILE_NAME), "garbage").unwrap();
        assert_eq!(build_calendar(&dir), embedded_calendar());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_remote_writes_cache_and_reports_changes() {
        let dir = temp_dir();
        assert!(apply_remote(&dir, &with_2027()).unwrap());
        assert!(dir.join(REMOTE_CACHE_FILE_NAME).exists());
        assert!(!dir.join(format!("{}.tmp", REMOTE_CACHE_FILE_NAME)).exists());
        assert!(covered_years().contains(&2027));
        assert!(is_workday(d("2027-02-20")));
        // 同样的数据再来一次：不写、不算变化。
        assert!(!apply_remote(&dir, &with_2027()).unwrap());
        // 之后下载到比缓存少的文件（例如仓库回退到只有内置年份）会被拒绝，缓存不变。
        assert!(apply_remote(&dir, EMBEDDED_HOLIDAYS).is_err());
        assert!(covered_years().contains(&2027));

        // 恢复为内置数据，避免影响其他测试。
        load(&temp_dir());
        assert!(!covered_years().contains(&2027));
        let _ = std::fs::remove_dir_all(dir);
    }
}
