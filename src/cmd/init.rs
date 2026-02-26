use crate::data::{
    AppSettings, BadgeEntry, BadgeEntryData, EventData, Holiday, HolidayData, QuarterConfig,
    Vacation, VacationData,
};
use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;
use std::fs;
use std::path::Path;

/// Saves updated settings to config.yaml while preserving the existing quarters section.
pub(crate) fn save_settings_to(settings: &AppSettings, dir: &Path) -> Result<()> {
    use crate::data::persistence::Persistable;
    // Load the existing quarters so we don't lose them
    let quarter_data = crate::data::QuarterData::load_from(dir).unwrap_or_default();
    let config = ConfigFile {
        settings: settings.clone(),
        quarters: quarter_data.quarters,
    };
    let yaml = serde_norway::to_string(&config)?;
    fs::write(dir.join("config.yaml"), yaml)?;
    Ok(())
}

/// Combined struct for serializing config.yaml in one pass.
/// `QuarterData` and `SettingsWrapper` both read config.yaml independently,
/// but writing them separately would overwrite each other — so we combine them here.
#[derive(Serialize)]
struct ConfigFile {
    settings: AppSettings,
    quarters: Vec<QuarterConfig>,
}

pub fn run() -> Result<()> {
    let dir = crate::data::persistence::get_data_dir()?;
    fs::create_dir_all(&dir)?;
    run_in_dir(&dir)?;
    println!("Data files initialized successfully.");
    Ok(())
}

/// Writes all default data files into `dir`. Exposed for unit testing.
pub(crate) fn run_in_dir(dir: &Path) -> Result<()> {
    write_config(dir)?;
    write_badge_data(dir)?;
    write_holidays(dir)?;
    write_vacations(dir)?;
    write_events(dir)?;
    Ok(())
}

fn write_config(dir: &Path) -> Result<()> {
    let config = ConfigFile {
        settings: AppSettings::default(),
        quarters: default_quarters(),
    };
    let yaml = serde_norway::to_string(&config)?;
    fs::write(dir.join("config.yaml"), yaml)?;
    Ok(())
}

fn write_badge_data(dir: &Path) -> Result<()> {
    let mut data = BadgeEntryData::default();
    data.add(BadgeEntry::new(d(2025, 2, 13), "McLean, VA", false));
    let json = serde_json::to_string_pretty(&data)?;
    fs::write(dir.join("badge_data.json"), json)?;
    Ok(())
}

fn write_holidays(dir: &Path) -> Result<()> {
    let mut data = HolidayData::default();
    init_holidays(&mut data);
    let yaml = serde_norway::to_string(&data)?;
    fs::write(dir.join("holidays.yaml"), yaml)?;
    Ok(())
}

fn write_vacations(dir: &Path) -> Result<()> {
    let mut data = VacationData::default();
    data.add(Vacation::new("Hawaii", "2025-05-10", "2025-05-17", true));
    let yaml = serde_norway::to_string(&data)?;
    fs::write(dir.join("vacations.yaml"), yaml)?;
    Ok(())
}

fn write_events(dir: &Path) -> Result<()> {
    let data = EventData::default();
    let json = serde_json::to_string_pretty(&data)?;
    fs::write(dir.join("events.json"), json)?;
    Ok(())
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn q(key: &str, quarter: &str, year: &str, start: &str, end: &str) -> QuarterConfig {
    QuarterConfig {
        key: key.to_string(),
        quarter: quarter.to_string(),
        year: year.to_string(),
        start_date_raw: start.to_string(),
        end_date_raw: end.to_string(),
        start_date: None,
        end_date: None,
    }
}

fn default_quarters() -> Vec<QuarterConfig> {
    vec![
        q("Q1_2025", "Q1", "2025", "2025-02-01", "2025-04-30"),
        q("Q2_2025", "Q2", "2025", "2025-05-01", "2025-07-31"),
        q("Q3_2025", "Q3", "2025", "2025-08-01", "2025-10-31"),
        q("Q4_2025", "Q4", "2025", "2025-11-01", "2026-01-31"),
        q("Q1_2026", "Q1", "2026", "2026-02-01", "2026-04-30"),
        q("Q2_2026", "Q2", "2026", "2026-05-01", "2026-07-31"),
        q("Q3_2026", "Q3", "2026", "2026-08-01", "2026-10-31"),
        q("Q4_2026", "Q4", "2026", "2026-11-01", "2027-01-31"),
    ]
}

fn init_holidays(data: &mut HolidayData) {
    data.add(Holiday::new("New Year's Day", "2025-01-01"));
    data.add(Holiday::new("Martin Luther King Jr. Day", "2025-01-20"));
    data.add(Holiday::new("Presidents' Day", "2025-02-17"));
    data.add(Holiday::new("Thank You Day #1", "2025-03-14"));
    data.add(Holiday::new("Thank You Day #2", "2025-05-23"));
    data.add(Holiday::new("Memorial Day", "2025-05-26"));
    data.add(Holiday::new("Juneteenth", "2025-06-19"));
    data.add(Holiday::new("Thank You Day #3", "2025-06-20"));
    data.add(Holiday::new("Independence Day", "2025-07-04"));
    data.add(Holiday::new("Thank You Day #4", "2025-08-29"));
    data.add(Holiday::new("Labor Day", "2025-09-01"));
    data.add(Holiday::new("Veterans Day", "2025-11-11"));
    data.add(Holiday::new("Thanksgiving Day", "2025-11-27"));
    data.add(Holiday::new("Thanksgiving Day After", "2025-11-28"));
    data.add(Holiday::new("Christmas Eve", "2025-12-24"));
    data.add(Holiday::new("Christmas Day", "2025-12-25"));

    data.add(Holiday::new("New Year's Day", "2026-01-01"));
    data.add(Holiday::new("Martin Luther King Jr. Day", "2026-01-19"));
    data.add(Holiday::new("Presidents' Day", "2026-02-16"));
    data.add(Holiday::new("Thank You Day #1", "2026-03-27"));
    data.add(Holiday::new("Thank You Day #2", "2026-05-22"));
    data.add(Holiday::new("Memorial Day", "2026-05-25"));
    data.add(Holiday::new("Thank You Day #3", "2026-06-18"));
    data.add(Holiday::new("Juneteenth", "2026-06-19"));
    data.add(Holiday::new("Independence Day", "2026-07-03"));
    data.add(Holiday::new("Thank You Day #4", "2026-09-04"));
    data.add(Holiday::new("Labor Day", "2026-09-07"));
    data.add(Holiday::new("Veterans Day", "2026-11-11"));
    data.add(Holiday::new("Thanksgiving Day", "2026-11-26"));
    data.add(Holiday::new("Thanksgiving Day After", "2026-11-27"));
    data.add(Holiday::new("Christmas Eve", "2026-12-24"));
    data.add(Holiday::new("Christmas Day", "2026-12-25"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_run_in_dir_creates_all_files() {
        let tmp = TempDir::new().unwrap();
        run_in_dir(tmp.path()).unwrap();
        assert!(tmp.path().join("config.yaml").exists(), "config.yaml missing");
        assert!(
            tmp.path().join("badge_data.json").exists(),
            "badge_data.json missing"
        );
        assert!(
            tmp.path().join("holidays.yaml").exists(),
            "holidays.yaml missing"
        );
        assert!(
            tmp.path().join("vacations.yaml").exists(),
            "vacations.yaml missing"
        );
        assert!(
            tmp.path().join("events.json").exists(),
            "events.json missing"
        );
    }

    #[test]
    fn test_events_file_is_empty_array() {
        let tmp = TempDir::new().unwrap();
        write_events(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("events.json")).unwrap();
        let data: EventData = serde_json::from_str(&content).unwrap();
        assert!(data.events.is_empty(), "events should be empty on init");
    }

    #[test]
    fn test_config_yaml_contains_settings_and_quarters() {
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("config.yaml")).unwrap();
        assert!(content.contains("settings"), "config.yaml missing 'settings' key");
        assert!(content.contains("quarters"), "config.yaml missing 'quarters' key");
        assert!(content.contains("Q1_2025"), "config.yaml missing Q1_2025");
        assert!(content.contains("Q4_2026"), "config.yaml missing Q4_2026");
        assert!(content.contains("default_office"), "config.yaml missing 'default_office'");
    }

    #[test]
    fn test_config_yaml_is_parseable_as_quarter_data() {
        use crate::data::QuarterData;
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("config.yaml")).unwrap();
        let qdata: QuarterData = serde_norway::from_str(&content).unwrap();
        assert_eq!(qdata.quarters.len(), 8, "expected 8 quarters");
        assert_eq!(qdata.quarters[0].key, "Q1_2025");
    }

    #[test]
    fn test_config_yaml_is_parseable_as_settings() {
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("config.yaml")).unwrap();
        // Parse the settings section via serde_norway
        #[derive(serde::Deserialize)]
        struct Wrapper {
            settings: AppSettings,
        }
        let w: Wrapper = serde_norway::from_str(&content).unwrap();
        assert_eq!(w.settings.default_office, "McLean, VA");
        assert_eq!(w.settings.flex_credit, "Flex Credit");
    }

    #[test]
    fn test_badge_data_file_has_one_entry() {
        let tmp = TempDir::new().unwrap();
        write_badge_data(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("badge_data.json")).unwrap();
        let data: BadgeEntryData = serde_json::from_str(&content).unwrap();
        assert_eq!(data.data.len(), 1);
        assert_eq!(data.data[0].key, "2025-02-13");
        assert_eq!(data.data[0].office, "McLean, VA");
    }

    #[test]
    fn test_holidays_file_has_expected_count() {
        let tmp = TempDir::new().unwrap();
        write_holidays(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("holidays.yaml")).unwrap();
        let data: HolidayData = serde_norway::from_str(&content).unwrap();
        assert_eq!(data.holidays.len(), 32, "expected 32 holidays");
    }

    #[test]
    fn test_vacations_file_has_one_entry() {
        let tmp = TempDir::new().unwrap();
        write_vacations(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("vacations.yaml")).unwrap();
        let data: VacationData = serde_norway::from_str(&content).unwrap();
        assert_eq!(data.vacations.len(), 1);
        assert_eq!(data.vacations[0].destination, "Hawaii");
    }

    #[test]
    fn test_default_quarters_count() {
        let quarters = default_quarters();
        assert_eq!(quarters.len(), 8);
    }

    #[test]
    fn test_default_quarters_dates_parseable() {
        for mut q in default_quarters() {
            q.parse_dates().expect("quarter dates should be valid");
            assert!(q.start_date.is_some());
            assert!(q.end_date.is_some());
        }
    }
}
