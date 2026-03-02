use crate::data::persistence::Persistable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Holiday {
    pub name: String,
    pub date: String,
}

impl Holiday {
    pub fn new(name: &str, date: &str) -> Self {
        Holiday {
            name: name.to_string(),
            date: date.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct HolidayData {
    pub holidays: Vec<Holiday>,
}

impl Persistable for HolidayData {
    fn filename() -> &'static str {
        "holidays.yaml"
    }
    fn is_json() -> bool {
        false
    }
}

impl HolidayData {
    pub fn add(&mut self, holiday: Holiday) {
        self.holidays.push(holiday);
    }

    pub fn all(&self) -> Vec<Holiday> {
        self.holidays.clone()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.holidays.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.holidays.is_empty()
    }

    pub fn get_holiday_map(&self) -> HashMap<String, &Holiday> {
        let mut map = HashMap::new();
        for h in &self.holidays {
            map.insert(h.date.clone(), h);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_holiday_new_sets_fields() {
        let h = Holiday::new("New Year's Day", "2025-01-01");
        assert_eq!(h.name, "New Year's Day");
        assert_eq!(h.date, "2025-01-01");
    }

    #[test]
    fn test_add_inserts_holiday() {
        let mut data = HolidayData::default();
        data.add(Holiday::new("Test Holiday", "2025-07-04"));
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_add_multiple_holidays() {
        let mut data = HolidayData::default();
        data.add(Holiday::new("Holiday A", "2025-01-01"));
        data.add(Holiday::new("Holiday B", "2025-07-04"));
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn test_all_returns_copy() {
        let mut data = HolidayData::default();
        data.add(Holiday::new("Test", "2025-01-01"));
        let all = data.all();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_get_holiday_map_keyed_by_date() {
        let mut data = HolidayData::default();
        data.add(Holiday::new("Independence Day", "2025-07-04"));
        data.add(Holiday::new("Labor Day", "2025-09-01"));
        let map = data.get_holiday_map();
        assert!(map.contains_key("2025-07-04"));
        assert!(map.contains_key("2025-09-01"));
        assert!(!map.contains_key("2025-12-25"));
        assert_eq!(map["2025-07-04"].name, "Independence Day");
    }

    #[test]
    fn test_get_holiday_map_empty() {
        let data = HolidayData::default();
        assert!(data.get_holiday_map().is_empty());
    }

    #[test]
    fn test_default_holiday_data_is_empty() {
        let data = HolidayData::default();
        assert!(data.is_empty());
    }
}
