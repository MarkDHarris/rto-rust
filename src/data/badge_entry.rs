use crate::data::persistence::Persistable;
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

const BADGE_DATE_FORMAT: &str = "%Y-%m-%d";

const FLEX_TIME_FORMATS: &[&str] = &[
    "%Y-%m-%dT%H:%M:%S%:z", // RFC3339-like with offset
    "%Y-%m-%dT%H:%M:%S",    // naive (no timezone)
    "%Y-%m-%dT%H:%M:%SZ",   // explicit UTC Z
    "%Y-%m-%d",             // date-only fallback
];

#[derive(Clone, Debug)]
pub struct FlexTime(pub NaiveDateTime);

impl FlexTime {
    pub fn from_date(date: NaiveDate) -> Self {
        FlexTime(date.and_hms_opt(0, 0, 0).unwrap())
    }
}

impl Serialize for FlexTime {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = self.0.format("%Y-%m-%dT%H:%M:%S").to_string();
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for FlexTime {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        for fmt in FLEX_TIME_FORMATS {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&s, fmt) {
                return Ok(FlexTime(dt));
            }
            if *fmt == BADGE_DATE_FORMAT
                && let Ok(d) = NaiveDate::parse_from_str(&s, fmt)
            {
                return Ok(FlexTime(d.and_hms_opt(0, 0, 0).unwrap()));
            }
        }
        Err(serde::de::Error::custom(format!(
            "cannot parse datetime {:?}",
            s
        )))
    }
}

impl fmt::Display for FlexTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%dT%H:%M:%S"))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BadgeEntry {
    #[serde(rename = "entry_date")]
    pub key: String,
    #[serde(rename = "date_time")]
    pub date_time: FlexTime,
    pub office: String,
    #[serde(default)]
    pub is_badged_in: bool,
    #[serde(default)]
    pub is_flex_credit: bool,
}

impl BadgeEntry {
    pub fn new(date: NaiveDate, office: &str, is_flex_credit: bool) -> Self {
        let key = date.format(BADGE_DATE_FORMAT).to_string();
        BadgeEntry {
            key,
            date_time: FlexTime::from_date(date),
            office: office.to_string(),
            is_badged_in: true,
            is_flex_credit,
        }
    }

    pub fn entry_date(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&self.key, BADGE_DATE_FORMAT).ok()
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct BadgeEntryData {
    #[serde(rename = "badge_data")]
    pub data: Vec<BadgeEntry>,
}

impl Persistable for BadgeEntryData {
    fn filename() -> &'static str {
        "badge_data.json"
    }
    fn is_json() -> bool {
        true
    }
}

impl BadgeEntryData {
    pub fn has(&self, key: &str) -> bool {
        self.data.iter().any(|e| e.key == key)
    }

    #[allow(dead_code)]
    pub fn get(&self, key: &str) -> Option<&BadgeEntry> {
        self.data.iter().find(|e| e.key == key)
    }

    pub fn add(&mut self, entry: BadgeEntry) {
        self.data.push(entry);
    }

    pub fn remove(&mut self, key: &str) {
        self.data.retain(|e| e.key != key);
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[allow(dead_code)]
    pub fn all(&self) -> Vec<BadgeEntry> {
        self.data.clone()
    }

    pub fn get_badge_map(&self, start: NaiveDate, end: NaiveDate) -> HashMap<String, &BadgeEntry> {
        let mut map = HashMap::new();
        for entry in &self.data {
            if let Some(entry_date) = entry.entry_date()
                && entry_date >= start
                && entry_date <= end
            {
                map.insert(entry.key.clone(), entry);
            }
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn test_badge_entry_new_sets_fields() {
        let entry = BadgeEntry::new(date(2025, 3, 15), "McLean, VA", false);
        assert_eq!(entry.key, "2025-03-15");
        assert_eq!(entry.office, "McLean, VA");
        assert!(entry.is_badged_in);
        assert!(!entry.is_flex_credit);
    }

    #[test]
    fn test_badge_entry_new_flex_sets_flag() {
        let entry = BadgeEntry::new(date(2025, 3, 15), "Flex Credit", true);
        assert!(entry.is_badged_in);
        assert!(entry.is_flex_credit);
    }

    #[test]
    fn test_has_returns_true_when_present() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        assert!(data.has("2025-01-10"));
    }

    #[test]
    fn test_has_returns_false_when_absent() {
        let data = BadgeEntryData::default();
        assert!(!data.has("2025-01-10"));
    }

    #[test]
    fn test_get_returns_entry() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        let entry = data.get("2025-01-10");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().office, "Office");
    }

    #[test]
    fn test_get_returns_none_when_absent() {
        let data = BadgeEntryData::default();
        assert!(data.get("2025-01-10").is_none());
    }

    #[test]
    fn test_add_increases_count() {
        let mut data = BadgeEntryData::default();
        assert_eq!(data.len(), 0);
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        assert_eq!(data.len(), 1);
        data.add(BadgeEntry::new(date(2025, 1, 11), "Office", false));
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn test_remove_deletes_entry() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        data.add(BadgeEntry::new(date(2025, 1, 11), "Office", false));
        data.remove("2025-01-10");
        assert!(!data.has("2025-01-10"));
        assert!(data.has("2025-01-11"));
    }

    #[test]
    fn test_remove_nonexistent_is_noop() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        data.remove("2025-12-31");
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_all_returns_copy() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        let all = data.all();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_get_badge_map_includes_entries_in_range() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 1, 5), "Office", false));
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        data.add(BadgeEntry::new(date(2025, 1, 20), "Office", false));
        let map = data.get_badge_map(date(2025, 1, 6), date(2025, 1, 15));
        assert!(map.contains_key("2025-01-10"));
        assert!(!map.contains_key("2025-01-05"));
        assert!(!map.contains_key("2025-01-20"));
    }

    #[test]
    fn test_get_badge_map_includes_boundary_dates() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 1, 1), "Office", false));
        data.add(BadgeEntry::new(date(2025, 1, 31), "Office", false));
        let map = data.get_badge_map(date(2025, 1, 1), date(2025, 1, 31));
        assert!(map.contains_key("2025-01-01"));
        assert!(map.contains_key("2025-01-31"));
    }

    #[test]
    fn test_get_badge_map_empty_range() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 3, 1), "Office", false));
        let map = data.get_badge_map(date(2025, 1, 1), date(2025, 1, 31));
        assert!(map.is_empty());
    }

    #[test]
    fn test_badge_entry_data_clone() {
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        let cloned = data.clone();
        assert_eq!(cloned.len(), 1);
    }

    #[test]
    fn test_flex_time_deserialize_naive() {
        let json = r#"{"entry_date":"2025-01-10","date_time":"2025-01-10T00:00:00","office":"Test","is_badged_in":true,"is_flex_credit":false}"#;
        let entry: BadgeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.key, "2025-01-10");
    }

    #[test]
    fn test_flex_time_deserialize_date_only() {
        let json = r#"{"entry_date":"2025-01-10","date_time":"2025-01-10","office":"Test","is_badged_in":true,"is_flex_credit":false}"#;
        let entry: BadgeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.key, "2025-01-10");
    }

    #[test]
    fn test_flex_time_serialize() {
        let ft = FlexTime::from_date(date(2025, 3, 15));
        let json = serde_json::to_string(&ft).unwrap();
        assert!(json.contains("2025-03-15T00:00:00"));
    }

    #[test]
    fn test_flex_time_deserialize_utc_z() {
        let json = r#"{"entry_date":"2025-01-10","date_time":"2025-01-10T14:30:00Z","office":"Test","is_badged_in":true,"is_flex_credit":false}"#;
        let entry: BadgeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.key, "2025-01-10");
    }
}
