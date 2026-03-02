use crate::data::persistence::{load_yaml_from, save_yaml_to};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

const SETTINGS_FILENAME: &str = "settings.yaml";
const DEFAULT_OFFICE: &str = "McLean, VA";
const DEFAULT_FLEX: &str = "Flex Credit";
const DEFAULT_GOAL: i32 = 50;
const DEFAULT_TIME_PERIOD_FILE: &str = "workday-fiscal-quarters.yaml";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppSettings {
    pub default_office: String,
    pub flex_credit: String,
    pub goal: i32,
    pub time_periods: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            default_office: DEFAULT_OFFICE.to_string(),
            flex_credit: DEFAULT_FLEX.to_string(),
            goal: DEFAULT_GOAL,
            time_periods: vec![DEFAULT_TIME_PERIOD_FILE.to_string()],
        }
    }
}

impl AppSettings {
    pub fn load() -> Result<Self> {
        let dir = crate::data::persistence::get_data_dir()?;
        Self::load_from(&dir)
    }

    pub fn load_from(dir: &Path) -> Result<Self> {
        let mut settings = Self::default();
        let loaded: Option<AppSettings> = load_yaml_from(dir, SETTINGS_FILENAME)?;
        if let Some(loaded) = loaded {
            if !loaded.default_office.is_empty() {
                settings.default_office = loaded.default_office;
            }
            if !loaded.flex_credit.is_empty() {
                settings.flex_credit = loaded.flex_credit;
            }
            if loaded.goal > 0 {
                settings.goal = loaded.goal;
            }
            if !loaded.time_periods.is_empty() {
                settings.time_periods = loaded.time_periods;
            }
        }
        Ok(settings)
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let dir = crate::data::persistence::get_data_dir()?;
        self.save_to(&dir)
    }

    pub fn save_to(&self, dir: &Path) -> Result<()> {
        save_yaml_to(dir, SETTINGS_FILENAME, self)
    }

    pub fn active_time_period_file(&self, idx: usize) -> &str {
        if idx < self.time_periods.len() {
            &self.time_periods[idx]
        } else if !self.time_periods.is_empty() {
            &self.time_periods[0]
        } else {
            DEFAULT_TIME_PERIOD_FILE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_app_settings_default_values() {
        let settings = AppSettings::default();
        assert_eq!(settings.default_office, "McLean, VA");
        assert_eq!(settings.flex_credit, "Flex Credit");
        assert_eq!(settings.goal, 50);
        assert_eq!(settings.time_periods.len(), 1);
    }

    #[test]
    fn test_app_settings_clone() {
        let s = AppSettings::default();
        let c = s.clone();
        assert_eq!(c.default_office, s.default_office);
        assert_eq!(c.flex_credit, s.flex_credit);
        assert_eq!(c.goal, s.goal);
    }

    #[test]
    fn test_settings_yaml_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let settings = AppSettings {
            default_office: "Remote HQ".to_string(),
            flex_credit: "Remote Credit".to_string(),
            goal: 60,
            time_periods: vec!["quarters.yaml".to_string(), "halves.yaml".to_string()],
        };
        settings.save_to(tmp.path()).unwrap();
        let loaded = AppSettings::load_from(tmp.path()).unwrap();
        assert_eq!(loaded.default_office, "Remote HQ");
        assert_eq!(loaded.flex_credit, "Remote Credit");
        assert_eq!(loaded.goal, 60);
        assert_eq!(loaded.time_periods.len(), 2);
    }

    #[test]
    fn test_settings_load_missing_file_uses_defaults() {
        let tmp = TempDir::new().unwrap();
        let loaded = AppSettings::load_from(tmp.path()).unwrap();
        assert_eq!(loaded.default_office, "McLean, VA");
        assert_eq!(loaded.goal, 50);
    }

    #[test]
    fn test_active_time_period_file() {
        let settings = AppSettings {
            time_periods: vec!["a.yaml".to_string(), "b.yaml".to_string()],
            ..AppSettings::default()
        };
        assert_eq!(settings.active_time_period_file(0), "a.yaml");
        assert_eq!(settings.active_time_period_file(1), "b.yaml");
        assert_eq!(settings.active_time_period_file(99), "a.yaml");
    }

    #[test]
    fn test_active_time_period_file_empty_list() {
        let settings = AppSettings {
            time_periods: vec![],
            ..AppSettings::default()
        };
        assert_eq!(
            settings.active_time_period_file(0),
            "workday-fiscal-quarters.yaml"
        );
    }
}
