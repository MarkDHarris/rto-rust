use crate::data::persistence::Persistable;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Default, Debug)]
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

    /// Expands all vacation date ranges to individual day entries.
    pub fn get_vacation_map(&self) -> std::collections::HashMap<String, Vacation> {
        let mut map = std::collections::HashMap::new();
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
                map.insert(current.format("%Y-%m-%d").to_string(), v.clone());
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
        assert_eq!(data.vacations.len(), 1);
    }

    #[test]
    fn test_get_vacation_map_expands_range() {
        let mut data = VacationData::default();
        // Monday through Friday (5 days)
        data.add(Vacation::new("Beach", "2025-01-06", "2025-01-10", true));
        let map = data.get_vacation_map();
        assert_eq!(map.len(), 5);
        assert!(map.contains_key("2025-01-06"));
        assert!(map.contains_key("2025-01-10"));
        assert!(!map.contains_key("2025-01-11"));
    }

    #[test]
    fn test_get_vacation_map_single_day() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Day off", "2025-03-15", "2025-03-15", true));
        let map = data.get_vacation_map();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("2025-03-15"));
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
        data.add(Vacation::new("Hawaii", "2025-05-10", "2025-05-12", true));
        let map = data.get_vacation_map();
        assert_eq!(map["2025-05-10"].destination, "Hawaii");
        assert_eq!(map["2025-05-11"].destination, "Hawaii");
        assert_eq!(map["2025-05-12"].destination, "Hawaii");
    }

    #[test]
    fn test_get_vacation_map_multiple_vacations() {
        let mut data = VacationData::default();
        data.add(Vacation::new("Trip A", "2025-01-06", "2025-01-07", true));
        data.add(Vacation::new("Trip B", "2025-02-10", "2025-02-11", true));
        let map = data.get_vacation_map();
        assert_eq!(map.len(), 4);
        assert_eq!(map["2025-01-06"].destination, "Trip A");
        assert_eq!(map["2025-02-10"].destination, "Trip B");
    }

    #[test]
    fn test_default_vacation_data_is_empty() {
        let data = VacationData::default();
        assert!(data.vacations.is_empty());
        assert!(data.get_vacation_map().is_empty());
    }
}
