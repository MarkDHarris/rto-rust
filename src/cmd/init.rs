use crate::data::{
    AppSettings, BadgeEntry, BadgeEntryData, Event, EventData, Holiday, HolidayData, TimePeriod,
    TimePeriodData, Vacation, VacationData,
};
use anyhow::Result;
use chrono::Local;
use std::fs;
use std::path::Path;

pub fn run() -> Result<()> {
    let dir = crate::data::persistence::get_data_dir()?;
    fs::create_dir_all(&dir)?;
    run_in_dir(&dir)?;
    println!("Initialized data files in: {}", dir.display());
    Ok(())
}

/// Non-destructively writes default data files. Existing files are never overwritten.
pub(crate) fn run_in_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;

    let settings = AppSettings::default();
    if !file_exists(dir, "settings.yaml") {
        settings.save_to(dir)?;
    }

    let tp_file = settings.active_time_period_file(0);
    if !file_exists(dir, tp_file) {
        let mut tp_data = TimePeriodData::new();
        for tp in default_time_periods() {
            tp_data.add(tp);
        }
        tp_data.save_to(dir)?;
    }

    if !file_exists(dir, "badge_data.json") {
        let mut badge_data = BadgeEntryData::default();
        badge_data.add(sample_badge_entry(&settings.default_office));
        use crate::data::Persistable;
        badge_data.save_to(dir)?;
    }

    if !file_exists(dir, "holidays.yaml") {
        let mut holiday_data = HolidayData::default();
        for h in default_holidays() {
            holiday_data.add(h);
        }
        use crate::data::Persistable;
        holiday_data.save_to(dir)?;
    }

    if !file_exists(dir, "vacations.yaml") {
        let mut vacation_data = VacationData::default();
        vacation_data.add(sample_vacation());
        use crate::data::Persistable;
        vacation_data.save_to(dir)?;
    }

    if !file_exists(dir, "events.json") {
        let mut event_data = EventData::default();
        event_data.add(sample_event());
        use crate::data::Persistable;
        event_data.save_to(dir)?;
    }

    Ok(())
}

fn file_exists(dir: &Path, name: &str) -> bool {
    dir.join(name).exists()
}

fn sample_badge_entry(office: &str) -> BadgeEntry {
    let today = Local::now().date_naive();
    BadgeEntry::new(today, office, false)
}

fn sample_vacation() -> Vacation {
    Vacation::new("Vacation Destination", "2025-07-04", "2025-07-11", true)
}

fn sample_event() -> Event {
    let today = Local::now().date_naive();
    Event {
        date: today.format("%Y-%m-%d").to_string(),
        description: "Sample event".to_string(),
    }
}

pub fn default_time_periods() -> Vec<TimePeriod> {
    vec![
        tp("Q1_2025", "Q1", "2025-01-01", "2025-03-31"),
        tp("Q2_2025", "Q2", "2025-04-01", "2025-06-30"),
        tp("Q3_2025", "Q3", "2025-07-01", "2025-09-30"),
        tp("Q4_2025", "Q4", "2025-10-01", "2025-12-31"),
        tp("Q1_2026", "Q1", "2026-01-01", "2026-03-31"),
        tp("Q2_2026", "Q2", "2026-04-01", "2026-06-30"),
        tp("Q3_2026", "Q3", "2026-07-01", "2026-09-30"),
        tp("Q4_2026", "Q4", "2026-10-01", "2026-12-31"),
    ]
}

fn tp(key: &str, name: &str, start: &str, end: &str) -> TimePeriod {
    TimePeriod {
        key: key.to_string(),
        name: name.to_string(),
        start_date_raw: start.to_string(),
        end_date_raw: end.to_string(),
        start_date: None,
        end_date: None,
    }
}

pub fn default_holidays() -> Vec<Holiday> {
    vec![
        Holiday::new("New Year's Day", "2025-01-01"),
        Holiday::new("MLK Day", "2025-01-20"),
        Holiday::new("Presidents' Day", "2025-02-17"),
        Holiday::new("Memorial Day", "2025-05-26"),
        Holiday::new("Juneteenth", "2025-06-19"),
        Holiday::new("Independence Day", "2025-07-04"),
        Holiday::new("Labor Day", "2025-09-01"),
        Holiday::new("Columbus Day", "2025-10-13"),
        Holiday::new("Veterans Day", "2025-11-11"),
        Holiday::new("Thanksgiving Day", "2025-11-27"),
        Holiday::new("Christmas Day", "2025-12-25"),
        Holiday::new("New Year's Day", "2026-01-01"),
        Holiday::new("MLK Day", "2026-01-19"),
        Holiday::new("Presidents' Day", "2026-02-16"),
        Holiday::new("Memorial Day", "2026-05-25"),
        Holiday::new("Juneteenth", "2026-06-19"),
        Holiday::new("Independence Day (observed)", "2026-07-03"),
        Holiday::new("Labor Day", "2026-09-07"),
        Holiday::new("Columbus Day", "2026-10-12"),
        Holiday::new("Veterans Day", "2026-11-11"),
        Holiday::new("Thanksgiving Day", "2026-11-26"),
        Holiday::new("Christmas Day", "2026-12-25"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_run_in_dir_creates_all_files() {
        let tmp = TempDir::new().unwrap();
        run_in_dir(tmp.path()).unwrap();
        assert!(tmp.path().join("settings.yaml").exists());
        assert!(tmp.path().join("badge_data.json").exists());
        assert!(tmp.path().join("holidays.yaml").exists());
        assert!(tmp.path().join("vacations.yaml").exists());
        assert!(tmp.path().join("events.json").exists());
        assert!(tmp.path().join("workday-fiscal-quarters.yaml").exists());
    }

    #[test]
    fn test_non_destructive_does_not_overwrite() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("settings.yaml"), "custom: true").unwrap();
        run_in_dir(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("settings.yaml")).unwrap();
        assert!(
            content.contains("custom: true"),
            "existing file should not be overwritten"
        );
    }

    #[test]
    fn test_events_file_has_sample() {
        let tmp = TempDir::new().unwrap();
        run_in_dir(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("events.json")).unwrap();
        let data: EventData = serde_json::from_str(&content).unwrap();
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_settings_yaml_is_parseable() {
        let tmp = TempDir::new().unwrap();
        run_in_dir(tmp.path()).unwrap();
        let loaded = AppSettings::load_from(tmp.path()).unwrap();
        assert_eq!(loaded.default_office, "McLean, VA");
        assert_eq!(loaded.goal, 50);
        assert!(!loaded.time_periods.is_empty());
    }

    #[test]
    fn test_time_period_file_is_parseable() {
        let tmp = TempDir::new().unwrap();
        run_in_dir(tmp.path()).unwrap();
        let loaded = TimePeriodData::load_from(tmp.path(), "workday-fiscal-quarters.yaml").unwrap();
        assert_eq!(loaded.len(), 8);
    }

    #[test]
    fn test_badge_data_has_one_entry() {
        let tmp = TempDir::new().unwrap();
        run_in_dir(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("badge_data.json")).unwrap();
        let data: BadgeEntryData = serde_json::from_str(&content).unwrap();
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_holidays_file_has_expected_count() {
        let tmp = TempDir::new().unwrap();
        run_in_dir(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("holidays.yaml")).unwrap();
        let data: HolidayData = serde_norway::from_str(&content).unwrap();
        assert_eq!(data.len(), 22);
    }

    #[test]
    fn test_vacations_file_has_one_entry() {
        let tmp = TempDir::new().unwrap();
        run_in_dir(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("vacations.yaml")).unwrap();
        let data: VacationData = serde_norway::from_str(&content).unwrap();
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_default_time_periods_count() {
        let periods = default_time_periods();
        assert_eq!(periods.len(), 8);
    }

    #[test]
    fn test_default_time_periods_parseable() {
        for mut tp in default_time_periods() {
            tp.parse_dates().expect("time period dates should be valid");
            assert!(tp.start_date.is_some());
            assert!(tp.end_date.is_some());
        }
    }
}
