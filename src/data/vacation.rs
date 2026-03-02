use crate::data::persistence::Persistable;
use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Vacation {
    pub destination: String,
    pub start_date: String,
    pub end_date: String,
    pub approved: bool,
}

impl Vacation {
    pub fn new(destination: &str, start_date: &str, end_date: &str, approved: bool) -> Self {
        Vacation {
            destination: destination.to_string(),
            start_date: start_date.to_string(),
            end_date: end_date.to_string(),
            approved,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct VacationData {
    pub vacations: Vec<Vacation>,
}

impl Persistable for VacationData {
    fn filename() -> &'static str {
        "vacations.yaml"
    }
    fn is_json() -> bool {
        false
    }
}

impl VacationData {
    pub fn add(&mut self, vacation: Vacation) {
        self.vacations.push(vacation);
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, start_date: &str, end_date: &str) {
        self.vacations
            .retain(|v| !(v.start_date == start_date && v.end_date == end_date));
    }

    pub fn all(&self) -> Vec<Vacation> {
        self.vacations.clone()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.vacations.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.vacations.is_empty()
    }

    /// Expands all vacation date ranges into individual weekday entries.
    /// Only weekdays (Mon-Fri) are included; holidays are NOT excluded here.
    pub fn get_vacation_map(&self) -> HashMap<String, Vacation> {
        let mut map = HashMap::new();
        for v in &self.vacations {
            let start = match NaiveDate::parse_from_str(&v.start_date, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };
            let end = match NaiveDate::parse_from_str(&v.end_date, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };
            let mut current = start;
            while current <= end {
                if !matches!(current.weekday(), Weekday::Sat | Weekday::Sun) {
                    map.insert(current.format("%Y-%m-%d").to_string(), v.clone());
                }
                current = current.succ_opt().unwrap_or(current);
            }
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vacation_new_sets_fields() {
        let v = Vacation::new("Hawaii", "2025-05-10", "2025-05-17", true);
        assert_eq!(v.destination, "Hawaii");
        assert_eq!(v.start_date, "2025-05-10");
        assert_eq!(v.end_date, "2025-05-17");
        assert!(v.approved);
    }

    #[test]
    fn test_vacation_new_unapproved() {
        let v = Vacation::new("Paris", "2025-06-01", "2025-06-07", false);
        assert!(!v.approved);
    }

    #[test]
    fn test_add_inserts_vacation() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Hawaii", "2025-05-10", "2025-05-17", true));
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_remove_deletes_matching_vacation() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Hawaii", "2025-05-10", "2025-05-17", true));
        data.add(Vacation::new("Paris", "2025-06-01", "2025-06-07", true));
        data.remove("2025-05-10", "2025-05-17");
        assert_eq!(data.len(), 1);
        assert_eq!(data.vacations[0].destination, "Paris");
    }

    #[test]
    fn test_remove_no_match_is_noop() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Hawaii", "2025-05-10", "2025-05-17", true));
        data.remove("2025-01-01", "2025-01-05");
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_get_vacation_map_expands_weekdays_only() {
        let mut data = VacationData::default();
        // Mon 2025-01-06 through Sun 2025-01-12 → only 5 weekdays
        data.add(Vacation::new("Beach", "2025-01-06", "2025-01-12", true));
        let map = data.get_vacation_map();
        assert_eq!(map.len(), 5);
        assert!(map.contains_key("2025-01-06")); // Monday
        assert!(map.contains_key("2025-01-10")); // Friday
        assert!(!map.contains_key("2025-01-11")); // Saturday
        assert!(!map.contains_key("2025-01-12")); // Sunday
    }

    #[test]
    fn test_get_vacation_map_single_day() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Day off", "2025-01-06", "2025-01-06", true)); // Monday
        let map = data.get_vacation_map();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("2025-01-06"));
    }

    #[test]
    fn test_get_vacation_map_skips_invalid_dates() {
        let mut data = VacationData::default();
        data.vacations.push(Vacation {
            destination: "Bad".to_string(),
            start_date: "not-a-date".to_string(),
            end_date: "2025-01-10".to_string(),
            approved: true,
        });
        let map = data.get_vacation_map();
        assert!(map.is_empty());
    }

    #[test]
    fn test_get_vacation_map_destination_preserved() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Hawaii", "2025-01-06", "2025-01-08", true)); // Mon-Wed
        let map = data.get_vacation_map();
        assert_eq!(map["2025-01-06"].destination, "Hawaii");
        assert_eq!(map["2025-01-07"].destination, "Hawaii");
        assert_eq!(map["2025-01-08"].destination, "Hawaii");
    }

    #[test]
    fn test_get_vacation_map_multiple_vacations() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Trip A", "2025-01-06", "2025-01-07", true)); // Mon-Tue
        data.add(Vacation::new("Trip B", "2025-01-13", "2025-01-14", true)); // Mon-Tue
        let map = data.get_vacation_map();
        assert_eq!(map.len(), 4);
        assert_eq!(map["2025-01-06"].destination, "Trip A");
        assert_eq!(map["2025-01-13"].destination, "Trip B");
    }

    #[test]
    fn test_all_returns_copy() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Trip", "2025-01-06", "2025-01-07", true));
        let all = data.all();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_default_vacation_data_is_empty() {
        let data = VacationData::default();
        assert!(data.is_empty());
        assert!(data.get_vacation_map().is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let mut data = VacationData::default();
        data.add(Vacation::new("Paris", "2025-07-14", "2025-07-18", true));
        data.add(Vacation::new("London", "2025-08-04", "2025-08-08", false));
        data.save_to(tmp.path()).unwrap();
        let loaded = VacationData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.vacations[0].destination, "Paris");
        assert!(!loaded.vacations[1].approved);
    }
}
