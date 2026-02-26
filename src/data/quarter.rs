use crate::data::persistence::Persistable;
use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct QuarterConfig {
    pub key: String,
    pub quarter: String,
    pub year: String,
    #[serde(rename = "start_date")]
    pub start_date_raw: String,
    #[serde(rename = "end_date")]
    pub end_date_raw: String,
    #[serde(skip)]
    pub start_date: Option<NaiveDate>,
    #[serde(skip)]
    pub end_date: Option<NaiveDate>,
}

impl QuarterConfig {
    pub fn parse_dates(&mut self) -> Result<()> {
        self.start_date = Some(
            NaiveDate::parse_from_str(&self.start_date_raw, "%Y-%m-%d")
                .with_context(|| format!("failed to parse start_date for {}", self.key))?,
        );
        self.end_date = Some(
            NaiveDate::parse_from_str(&self.end_date_raw, "%Y-%m-%d")
                .with_context(|| format!("failed to parse end_date for {}", self.key))?,
        );
        Ok(())
    }

    /// Returns true if date is in [start_date, end_date] inclusive.
    /// NaiveDate comparison is straightforward — no timezone workaround needed.
    pub fn is_date_in_range(&self, date: NaiveDate) -> bool {
        match (self.start_date, self.end_date) {
            (Some(s), Some(e)) => date >= s && date <= e,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct QuarterData {
    pub quarters: Vec<QuarterConfig>,
}

impl Persistable for QuarterData {
    fn filename() -> &'static str {
        "config.yaml"
    }
    fn is_json() -> bool {
        false
    }
}

impl QuarterData {
    pub fn load_and_parse() -> Result<Self> {
        let mut data = Self::load()?;
        for q in &mut data.quarters {
            q.parse_dates()?;
        }
        Ok(data)
    }

    pub fn get_current_quarter(&self) -> Option<&QuarterConfig> {
        let today = Local::now().date_naive();
        self.get_quarter_by_date(today)
    }

    pub fn get_quarter_by_date(&self, date: NaiveDate) -> Option<&QuarterConfig> {
        self.quarters.iter().find(|q| q.is_date_in_range(date))
    }

    pub fn get_quarter_by_key(&self, key: &str) -> Option<&QuarterConfig> {
        self.quarters.iter().find(|q| q.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_quarter(key: &str, start: &str, end: &str) -> QuarterConfig {
        let mut q = QuarterConfig {
            key: key.to_string(),
            quarter: "Q1".to_string(),
            year: "2025".to_string(),
            start_date_raw: start.to_string(),
            end_date_raw: end.to_string(),
            start_date: None,
            end_date: None,
        };
        q.parse_dates().unwrap();
        q
    }

    #[test]
    fn test_parse_dates_populates_fields() {
        let mut q = QuarterConfig {
            key: "TEST".to_string(),
            quarter: "Q1".to_string(),
            year: "2025".to_string(),
            start_date_raw: "2025-02-01".to_string(),
            end_date_raw: "2025-04-30".to_string(),
            start_date: None,
            end_date: None,
        };
        q.parse_dates().unwrap();
        assert_eq!(
            q.start_date.unwrap(),
            NaiveDate::from_ymd_opt(2025, 2, 1).unwrap()
        );
        assert_eq!(
            q.end_date.unwrap(),
            NaiveDate::from_ymd_opt(2025, 4, 30).unwrap()
        );
    }

    #[test]
    fn test_parse_dates_invalid_returns_error() {
        let mut q = QuarterConfig {
            key: "BAD".to_string(),
            quarter: "Q1".to_string(),
            year: "2025".to_string(),
            start_date_raw: "not-a-date".to_string(),
            end_date_raw: "2025-04-30".to_string(),
            start_date: None,
            end_date: None,
        };
        assert!(q.parse_dates().is_err());
    }

    #[test]
    fn test_is_date_in_range_middle() {
        let q = make_quarter("Q", "2025-02-01", "2025-04-30");
        let mid = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        assert!(q.is_date_in_range(mid));
    }

    #[test]
    fn test_is_date_in_range_start_boundary() {
        let q = make_quarter("Q", "2025-02-01", "2025-04-30");
        let start = NaiveDate::from_ymd_opt(2025, 2, 1).unwrap();
        assert!(q.is_date_in_range(start));
    }

    #[test]
    fn test_is_date_in_range_end_boundary() {
        let q = make_quarter("Q", "2025-02-01", "2025-04-30");
        let end = NaiveDate::from_ymd_opt(2025, 4, 30).unwrap();
        assert!(q.is_date_in_range(end));
    }

    #[test]
    fn test_is_date_in_range_before_start() {
        let q = make_quarter("Q", "2025-02-01", "2025-04-30");
        let before = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
        assert!(!q.is_date_in_range(before));
    }

    #[test]
    fn test_is_date_in_range_after_end() {
        let q = make_quarter("Q", "2025-02-01", "2025-04-30");
        let after = NaiveDate::from_ymd_opt(2025, 5, 1).unwrap();
        assert!(!q.is_date_in_range(after));
    }

    #[test]
    fn test_is_date_in_range_unparsed_returns_false() {
        let q = QuarterConfig {
            key: "Q".to_string(),
            quarter: "Q1".to_string(),
            year: "2025".to_string(),
            start_date_raw: "2025-02-01".to_string(),
            end_date_raw: "2025-04-30".to_string(),
            start_date: None, // not parsed
            end_date: None,
        };
        let date = NaiveDate::from_ymd_opt(2025, 3, 1).unwrap();
        assert!(!q.is_date_in_range(date));
    }

    #[test]
    fn test_get_quarter_by_key_found() {
        let data = QuarterData {
            quarters: vec![
                make_quarter("Q1_2025", "2025-02-01", "2025-04-30"),
                make_quarter("Q2_2025", "2025-05-01", "2025-07-31"),
            ],
        };
        let q = data.get_quarter_by_key("Q2_2025");
        assert!(q.is_some());
        assert_eq!(q.unwrap().key, "Q2_2025");
    }

    #[test]
    fn test_get_quarter_by_key_not_found() {
        let data = QuarterData {
            quarters: vec![make_quarter("Q1_2025", "2025-02-01", "2025-04-30")],
        };
        assert!(data.get_quarter_by_key("MISSING").is_none());
    }

    #[test]
    fn test_get_quarter_by_date_found() {
        let data = QuarterData {
            quarters: vec![
                make_quarter("Q1_2025", "2025-02-01", "2025-04-30"),
                make_quarter("Q2_2025", "2025-05-01", "2025-07-31"),
            ],
        };
        let date = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let q = data.get_quarter_by_date(date);
        assert!(q.is_some());
        assert_eq!(q.unwrap().key, "Q2_2025");
    }

    #[test]
    fn test_get_quarter_by_date_not_found() {
        let data = QuarterData {
            quarters: vec![make_quarter("Q1_2025", "2025-02-01", "2025-04-30")],
        };
        let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert!(data.get_quarter_by_date(date).is_none());
    }

    #[test]
    fn test_quarter_data_default_is_empty() {
        let data = QuarterData::default();
        assert!(data.quarters.is_empty());
    }
}
