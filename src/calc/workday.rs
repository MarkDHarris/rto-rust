use chrono::{Datelike, NaiveDate, Weekday};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Workday {
    pub date: NaiveDate,
    pub work_date: String,
    pub is_workday: bool,
    pub is_badged_in: bool,
    pub is_flex_credit: bool,
    pub is_holiday: bool,
    pub is_vacation: bool,
}

/// Returns true for Monday–Friday, false for Saturday/Sunday.
pub fn is_workday(date: NaiveDate) -> bool {
    !matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Builds a map of date-string -> Workday for all weekdays in [start, end] inclusive.
pub fn create_workday_map(start: NaiveDate, end: NaiveDate) -> HashMap<String, Workday> {
    let mut map = HashMap::new();
    let mut current = start;
    while current <= end {
        if is_workday(current) {
            let key = current.format("%Y-%m-%d").to_string();
            map.insert(
                key.clone(),
                Workday {
                    date: current,
                    work_date: key,
                    is_workday: false, // set in quarter_calc after holiday/vacation check
                    is_badged_in: false,
                    is_flex_credit: false,
                    is_holiday: false,
                    is_vacation: false,
                },
            );
        }
        current = current.succ_opt().unwrap_or(current);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_is_workday_for_each_weekday() {
        // 2025-01-06 is Monday
        let monday = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        assert!(is_workday(monday));
        assert!(is_workday(monday.succ_opt().unwrap())); // Tuesday
        assert!(is_workday(monday.succ_opt().unwrap().succ_opt().unwrap())); // Wednesday
        let thursday = NaiveDate::from_ymd_opt(2025, 1, 9).unwrap();
        let friday = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        assert!(is_workday(thursday));
        assert!(is_workday(friday));
    }

    #[test]
    fn test_is_not_workday_for_weekend() {
        let saturday = NaiveDate::from_ymd_opt(2025, 1, 11).unwrap();
        let sunday = NaiveDate::from_ymd_opt(2025, 1, 12).unwrap();
        assert!(!is_workday(saturday));
        assert!(!is_workday(sunday));
    }

    #[test]
    fn test_workday_map_excludes_weekends() {
        // 2025-01-06 (Mon) to 2025-01-12 (Sun) = 5 weekdays
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 12).unwrap();
        let map = create_workday_map(start, end);
        assert_eq!(map.len(), 5);
        assert!(!map.contains_key("2025-01-11")); // Saturday
        assert!(!map.contains_key("2025-01-12")); // Sunday
    }

    #[test]
    fn test_workday_map_inclusive_range() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday
        let end = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(); // Friday
        let map = create_workday_map(start, end);
        assert_eq!(map.len(), 5);
        assert!(map.contains_key("2025-01-06"));
        assert!(map.contains_key("2025-01-10"));
    }
}
