use crate::data::persistence::Persistable;
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BadgeEntry {
    #[serde(rename = "entry_date")]
    pub key: String,
    #[serde(rename = "date_time")]
    pub date_time: NaiveDateTime,
    pub office: String,
    #[serde(default = "default_true")]
    pub is_badged_in: bool,
    #[serde(default)]
    pub is_flex_credit: bool,
}

fn default_true() -> bool {
    true
}

impl BadgeEntry {
    pub fn new(date: NaiveDate, office: &str, is_flex_credit: bool) -> Self {
        let key = date.format("%Y-%m-%d").to_string();
        let date_time = date.and_hms_opt(0, 0, 0).unwrap();
        BadgeEntry {
            key,
            date_time,
            office: office.to_string(),
            is_badged_in: true,
            is_flex_credit,
        }
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

    pub fn add(&mut self, entry: BadgeEntry) {
        self.data.push(entry);
    }

    pub fn remove(&mut self, key: &str) {
        self.data.retain(|e| e.key != key);
    }

    /// Returns a map of date-key -> BadgeEntry for entries within [start, end] inclusive.
    pub fn get_badge_map(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> std::collections::HashMap<String, &BadgeEntry> {
        let mut map = std::collections::HashMap::new();
        for entry in &self.data {
            let entry_date = entry.date_time.date();
            if entry_date >= start && entry_date <= end {
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
        assert_eq!(entry.date_time.date(), date(2025, 3, 15));
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
    fn test_add_increases_count() {
        let mut data = BadgeEntryData::default();
        assert_eq!(data.data.len(), 0);
        data.add(BadgeEntry::new(date(2025, 1, 10), "Office", false));
        assert_eq!(data.data.len(), 1);
        data.add(BadgeEntry::new(date(2025, 1, 11), "Office", false));
        assert_eq!(data.data.len(), 2);
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
        assert_eq!(data.data.len(), 1);
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
        assert_eq!(cloned.data.len(), 1);
    }
}
