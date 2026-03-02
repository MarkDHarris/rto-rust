use crate::data::persistence::{get_data_dir, load_yaml_from, save_yaml_to};
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::Path;

const DEFAULT_TIME_PERIODS_FILENAME: &str = "workday-fiscal-quarters.yaml";
const DEFAULT_CALENDAR_DISPLAY_COLUMNS: i32 = 3;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TimePeriod {
    pub key: String,
    pub name: String,
    #[serde(rename = "start_date")]
    pub start_date_raw: String,
    #[serde(rename = "end_date")]
    pub end_date_raw: String,
    #[serde(skip)]
    pub start_date: Option<NaiveDate>,
    #[serde(skip)]
    pub end_date: Option<NaiveDate>,
}

impl TimePeriod {
    pub fn parse_dates(&mut self) -> Result<()> {
        self.start_date = Some(
            NaiveDate::parse_from_str(&self.start_date_raw, "%Y-%m-%d").with_context(|| {
                format!(
                    "parsing start_date {:?} for {}",
                    self.start_date_raw, self.key
                )
            })?,
        );
        self.end_date = Some(
            NaiveDate::parse_from_str(&self.end_date_raw, "%Y-%m-%d").with_context(|| {
                format!("parsing end_date {:?} for {}", self.end_date_raw, self.key)
            })?,
        );
        Ok(())
    }

    pub fn is_date_in_range(&self, date: NaiveDate) -> bool {
        match (self.start_date, self.end_date) {
            (Some(s), Some(e)) => date >= s && date <= e,
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn month_span(&self) -> i32 {
        match (self.start_date, self.end_date) {
            (Some(s), Some(e)) => {
                let start_y = s.year();
                let start_m = s.month() as i32;
                let end_y = e.year();
                let end_m = e.month() as i32;
                (end_y - start_y) * 12 + (end_m - start_m) + 1
            }
            _ => 3,
        }
    }
}

use chrono::Datelike;

#[derive(Serialize, Deserialize, Debug, Default)]
struct TimePeriodDataFile {
    #[serde(default)]
    calendar_display_columns: Option<i32>,
    #[serde(default)]
    timeperiods: Vec<TimePeriod>,
}

#[derive(Debug, Clone)]
pub struct TimePeriodData {
    periods: Vec<TimePeriod>,
    filename: String,
    calendar_display_columns: i32,
}

#[allow(dead_code)]
impl TimePeriodData {
    pub fn new() -> Self {
        TimePeriodData {
            periods: Vec::new(),
            filename: DEFAULT_TIME_PERIODS_FILENAME.to_string(),
            calendar_display_columns: DEFAULT_CALENDAR_DISPLAY_COLUMNS,
        }
    }

    #[allow(dead_code)]
    pub fn new_with_file(filename: &str) -> Self {
        let filename = if filename.is_empty() {
            DEFAULT_TIME_PERIODS_FILENAME
        } else {
            filename
        };
        TimePeriodData {
            periods: Vec::new(),
            filename: filename.to_string(),
            calendar_display_columns: DEFAULT_CALENDAR_DISPLAY_COLUMNS,
        }
    }

    pub fn load() -> Result<Self> {
        let dir = get_data_dir()?;
        let filename = crate::data::AppSettings::load_from(&dir)
            .map(|s| s.active_time_period_file(0).to_string())
            .unwrap_or_default();
        Self::load_from(&dir, &filename)
    }

    pub fn load_from(dir: &Path, filename: &str) -> Result<Self> {
        let filename = if filename.is_empty() {
            DEFAULT_TIME_PERIODS_FILENAME
        } else {
            filename
        };

        let file: Option<TimePeriodDataFile> = load_yaml_from(dir, filename)?;
        let file = file.unwrap_or_default();

        let mut periods = file.timeperiods;
        for tp in &mut periods {
            tp.parse_dates()?;
        }

        let cols = file
            .calendar_display_columns
            .filter(|&c| c > 0)
            .unwrap_or(DEFAULT_CALENDAR_DISPLAY_COLUMNS);

        Ok(TimePeriodData {
            periods,
            filename: filename.to_string(),
            calendar_display_columns: cols,
        })
    }

    pub fn save(&self) -> Result<()> {
        let dir = get_data_dir()?;
        self.save_to(&dir)
    }

    pub fn save_to(&self, dir: &Path) -> Result<()> {
        let file = TimePeriodDataFile {
            calendar_display_columns: Some(self.calendar_display_columns),
            timeperiods: self.periods.clone(),
        };
        save_yaml_to(dir, &self.filename, &file)
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn calendar_display_columns(&self) -> i32 {
        if self.calendar_display_columns <= 0 {
            DEFAULT_CALENDAR_DISPLAY_COLUMNS
        } else {
            self.calendar_display_columns
        }
    }

    pub fn set_calendar_display_columns(&mut self, cols: i32) {
        self.calendar_display_columns = cols;
    }

    pub fn all(&self) -> Vec<TimePeriod> {
        self.periods.clone()
    }

    pub fn len(&self) -> usize {
        self.periods.len()
    }

    pub fn is_empty(&self) -> bool {
        self.periods.is_empty()
    }

    pub fn add(&mut self, tp: TimePeriod) {
        self.periods.push(tp);
    }

    pub fn get_current_period(&self) -> Option<&TimePeriod> {
        let today = chrono::Local::now().date_naive();
        self.get_period_by_date(today)
    }

    pub fn get_period_by_date(&self, date: NaiveDate) -> Option<&TimePeriod> {
        self.periods.iter().find(|tp| tp.is_date_in_range(date))
    }

    pub fn get_period_by_key(&self, key: &str) -> Option<&TimePeriod> {
        self.periods.iter().find(|tp| tp.key == key)
    }

    pub fn nearest_period(&self, date: NaiveDate) -> Result<&TimePeriod> {
        if self.periods.is_empty() {
            bail!("no time periods configured");
        }
        if let Some(tp) = self.get_period_by_date(date) {
            return Ok(tp);
        }
        let mut closest = &self.periods[0];
        let mut min_diff = abs_days(date, closest.start_date.unwrap_or(date));
        for tp in &self.periods[1..] {
            let d = abs_days(date, tp.start_date.unwrap_or(date));
            if d < min_diff {
                min_diff = d;
                closest = tp;
            }
        }
        Ok(closest)
    }
}

#[allow(dead_code)]
pub(crate) fn abs_days(a: NaiveDate, b: NaiveDate) -> i64 {
    (a - b).num_days().abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_period(key: &str, name: &str, start: &str, end: &str) -> TimePeriod {
        let mut tp = TimePeriod {
            key: key.to_string(),
            name: name.to_string(),
            start_date_raw: start.to_string(),
            end_date_raw: end.to_string(),
            start_date: None,
            end_date: None,
        };
        tp.parse_dates().unwrap();
        tp
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn test_parse_dates_populates_fields() {
        let mut tp = TimePeriod {
            key: "TEST".to_string(),
            name: "Test".to_string(),
            start_date_raw: "2025-02-01".to_string(),
            end_date_raw: "2025-04-30".to_string(),
            start_date: None,
            end_date: None,
        };
        tp.parse_dates().unwrap();
        assert_eq!(tp.start_date.unwrap(), date(2025, 2, 1));
        assert_eq!(tp.end_date.unwrap(), date(2025, 4, 30));
    }

    #[test]
    fn test_parse_dates_invalid_returns_error() {
        let mut tp = TimePeriod {
            key: "BAD".to_string(),
            name: "Bad".to_string(),
            start_date_raw: "not-a-date".to_string(),
            end_date_raw: "2025-04-30".to_string(),
            start_date: None,
            end_date: None,
        };
        assert!(tp.parse_dates().is_err());
    }

    #[test]
    fn test_is_date_in_range_middle() {
        let tp = make_period("Q", "Q1", "2025-02-01", "2025-04-30");
        assert!(tp.is_date_in_range(date(2025, 3, 15)));
    }

    #[test]
    fn test_is_date_in_range_boundaries() {
        let tp = make_period("Q", "Q1", "2025-02-01", "2025-04-30");
        assert!(tp.is_date_in_range(date(2025, 2, 1)));
        assert!(tp.is_date_in_range(date(2025, 4, 30)));
    }

    #[test]
    fn test_is_date_in_range_outside() {
        let tp = make_period("Q", "Q1", "2025-02-01", "2025-04-30");
        assert!(!tp.is_date_in_range(date(2025, 1, 31)));
        assert!(!tp.is_date_in_range(date(2025, 5, 1)));
    }

    #[test]
    fn test_is_date_in_range_unparsed() {
        let tp = TimePeriod {
            key: "Q".to_string(),
            start_date: None,
            end_date: None,
            ..Default::default()
        };
        assert!(!tp.is_date_in_range(date(2025, 3, 1)));
    }

    #[test]
    fn test_month_span_quarter() {
        let tp = make_period("Q1", "Q1", "2025-01-01", "2025-03-31");
        assert_eq!(tp.month_span(), 3);
    }

    #[test]
    fn test_month_span_half_year() {
        let tp = make_period("H1", "H1", "2025-01-01", "2025-06-30");
        assert_eq!(tp.month_span(), 6);
    }

    #[test]
    fn test_month_span_full_year() {
        let tp = make_period("Y", "Year", "2025-01-01", "2025-12-31");
        assert_eq!(tp.month_span(), 12);
    }

    #[test]
    fn test_get_period_by_key_found() {
        let mut data = TimePeriodData::new();
        data.add(make_period("Q1_2025", "Q1", "2025-02-01", "2025-04-30"));
        data.add(make_period("Q2_2025", "Q2", "2025-05-01", "2025-07-31"));
        let tp = data.get_period_by_key("Q2_2025");
        assert!(tp.is_some());
        assert_eq!(tp.unwrap().key, "Q2_2025");
    }

    #[test]
    fn test_get_period_by_key_not_found() {
        let mut data = TimePeriodData::new();
        data.add(make_period("Q1_2025", "Q1", "2025-02-01", "2025-04-30"));
        assert!(data.get_period_by_key("MISSING").is_none());
    }

    #[test]
    fn test_get_period_by_date_found() {
        let mut data = TimePeriodData::new();
        data.add(make_period("Q1_2025", "Q1", "2025-02-01", "2025-04-30"));
        data.add(make_period("Q2_2025", "Q2", "2025-05-01", "2025-07-31"));
        let tp = data.get_period_by_date(date(2025, 6, 15));
        assert!(tp.is_some());
        assert_eq!(tp.unwrap().key, "Q2_2025");
    }

    #[test]
    fn test_get_period_by_date_not_found() {
        let mut data = TimePeriodData::new();
        data.add(make_period("Q1_2025", "Q1", "2025-02-01", "2025-04-30"));
        assert!(data.get_period_by_date(date(2026, 1, 1)).is_none());
    }

    #[test]
    fn test_nearest_period_exact_match() {
        let mut data = TimePeriodData::new();
        data.add(make_period("Q1", "Q1", "2025-01-01", "2025-03-31"));
        data.add(make_period("Q2", "Q2", "2025-04-01", "2025-06-30"));
        let tp = data.nearest_period(date(2025, 2, 15)).unwrap();
        assert_eq!(tp.key, "Q1");
    }

    #[test]
    fn test_nearest_period_closest() {
        let mut data = TimePeriodData::new();
        data.add(make_period("Q1", "Q1", "2025-01-01", "2025-03-31"));
        data.add(make_period("Q2", "Q2", "2025-04-01", "2025-06-30"));
        let tp = data.nearest_period(date(2024, 12, 1)).unwrap();
        assert_eq!(tp.key, "Q1");
    }

    #[test]
    fn test_nearest_period_empty_returns_error() {
        let data = TimePeriodData::new();
        assert!(data.nearest_period(date(2025, 1, 1)).is_err());
    }

    #[test]
    fn test_time_period_data_default_is_empty() {
        let data = TimePeriodData::new();
        assert!(data.is_empty());
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mut data = TimePeriodData::new();
        data.add(TimePeriod {
            key: "Q1_2025".to_string(),
            name: "Q1".to_string(),
            start_date_raw: "2025-01-01".to_string(),
            end_date_raw: "2025-03-31".to_string(),
            start_date: None,
            end_date: None,
        });
        data.save_to(tmp.path()).unwrap();
        let loaded = TimePeriodData::load_from(tmp.path(), DEFAULT_TIME_PERIODS_FILENAME).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.periods[0].key, "Q1_2025");
        assert!(loaded.periods[0].start_date.is_some());
    }

    #[test]
    fn test_calendar_display_columns_default() {
        let data = TimePeriodData::new();
        assert_eq!(data.calendar_display_columns(), 3);
    }

    #[test]
    fn test_calendar_display_columns_custom() {
        let mut data = TimePeriodData::new();
        data.set_calendar_display_columns(4);
        assert_eq!(data.calendar_display_columns(), 4);
    }
}
