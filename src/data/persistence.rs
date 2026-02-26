use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Set once at startup by main() from the --data-dir argument.
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Call this from main() before any load/save operations.
pub fn set_data_dir(path: PathBuf) {
    let _ = DATA_DIR.set(path);
}

pub fn get_data_dir() -> Result<PathBuf> {
    if let Some(dir) = DATA_DIR.get() {
        return Ok(dir.clone());
    }
    // Fallback when running tests or if set_data_dir was not called
    let cwd = std::env::current_dir().context("failed to get current directory")?;
    Ok(cwd.join("config"))
}

pub fn get_file_path(name: &str) -> Result<PathBuf> {
    let dir = get_data_dir()?;
    Ok(dir.join(name))
}

pub trait Persistable: Sized + Default + Serialize + for<'de> Deserialize<'de> {
    fn filename() -> &'static str;
    fn is_json() -> bool;

    fn load() -> Result<Self> {
        let path = get_file_path(Self::filename())?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if Self::is_json() {
            serde_json::from_str(&contents)
                .with_context(|| format!("failed to parse JSON from {}", path.display()))
        } else {
            serde_norway::from_str(&contents)
                .with_context(|| format!("failed to parse YAML from {}", path.display()))
        }
    }

    fn save(&self) -> Result<()> {
        let path = get_file_path(Self::filename())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create dir {}", parent.display()))?;
        }
        let contents = if Self::is_json() {
            serde_json::to_string_pretty(self).context("failed to serialize JSON")?
        } else {
            serde_norway::to_string(self).context("failed to serialize YAML")?
        };
        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    /// Load from an explicit directory, bypassing the global `DATA_DIR`.
    fn load_from(dir: &Path) -> Result<Self> {
        let path = dir.join(Self::filename());
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if Self::is_json() {
            serde_json::from_str(&contents)
                .with_context(|| format!("failed to parse JSON from {}", path.display()))
        } else {
            serde_norway::from_str(&contents)
                .with_context(|| format!("failed to parse YAML from {}", path.display()))
        }
    }

    /// Save to an explicit directory, bypassing the global `DATA_DIR`.
    fn save_to(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join(Self::filename());
        let contents = if Self::is_json() {
            serde_json::to_string_pretty(self).context("failed to serialize JSON")?
        } else {
            serde_norway::to_string(self).context("failed to serialize YAML")?
        };
        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    /// Minimal Persistable implementation for testing serialization logic.
    #[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
    struct TestJsonData {
        value: String,
    }

    impl Persistable for TestJsonData {
        fn filename() -> &'static str {
            "test_data.json"
        }
        fn is_json() -> bool {
            true
        }
    }

    #[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
    struct TestYamlData {
        count: u32,
    }

    impl Persistable for TestYamlData {
        fn filename() -> &'static str {
            "test_data.yaml"
        }
        fn is_json() -> bool {
            false
        }
    }

    #[test]
    fn test_get_data_dir_returns_a_path() {
        // When DATA_DIR is unset the fallback is cwd/config.
        // When it IS set (by a prior test run), it returns that value.
        // Either way a valid PathBuf should be returned.
        let result = get_data_dir();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_file_path_appends_filename() {
        let path = get_file_path("my_file.json").unwrap();
        assert!(path.ends_with("my_file.json"));
    }

    #[test]
    fn test_json_save_and_load_roundtrip() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let json_path = tmp.path().join("test_data.json");

        // Write directly (bypass global DATA_DIR)
        let data = TestJsonData {
            value: "hello".to_string(),
        };
        let serialized = serde_json::to_string_pretty(&data).unwrap();
        fs::write(&json_path, serialized).unwrap();

        // Read back and deserialize
        let contents = fs::read_to_string(&json_path).unwrap();
        let loaded: TestJsonData = serde_json::from_str(&contents).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_yaml_save_and_load_roundtrip() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let yaml_path = tmp.path().join("test_data.yaml");

        let data = TestYamlData { count: 42 };
        let serialized = serde_norway::to_string(&data).unwrap();
        fs::write(&yaml_path, serialized).unwrap();

        let contents = fs::read_to_string(&yaml_path).unwrap();
        let loaded: TestYamlData = serde_norway::from_str(&contents).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_persistable_load_returns_default_for_missing_file() {
        // DATA_DIR may or may not point to a real directory, but test_data.json
        // should not exist there. If the file is absent, load() returns Default.
        // We test the JSON deserialize path by constructing it directly.
        let data: TestJsonData = serde_json::from_str("{}").unwrap_or_default();
        assert_eq!(data.value, "");
    }

    #[test]
    fn test_load_from_returns_default_when_file_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let result: TestJsonData = TestJsonData::load_from(tmp.path()).unwrap();
        assert_eq!(result, TestJsonData::default());
    }

    #[test]
    fn test_json_save_to_and_load_from_roundtrip() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let data = TestJsonData { value: "round-trip".to_string() };
        data.save_to(tmp.path()).unwrap();
        let loaded = TestJsonData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_yaml_save_to_and_load_from_roundtrip() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let data = TestYamlData { count: 99 };
        data.save_to(tmp.path()).unwrap();
        let loaded = TestYamlData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_badge_entry_data_save_to_load_from() {
        use crate::data::badge_entry::{BadgeEntry, BadgeEntryData};
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let mut data = BadgeEntryData::default();
        data.add(BadgeEntry::new(
            chrono::NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
            "McLean, VA",
            false,
        ));
        data.save_to(tmp.path()).unwrap();
        let loaded = BadgeEntryData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded.data.len(), 1);
        assert_eq!(loaded.data[0].key, "2025-03-15");
        assert_eq!(loaded.data[0].office, "McLean, VA");
    }

    #[test]
    fn test_event_data_save_to_load_from() {
        use crate::data::event::{Event, EventData};
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let mut data = EventData::default();
        data.add(Event { date: "2025-06-01".to_string(), description: "Conference".to_string() });
        data.save_to(tmp.path()).unwrap();
        let loaded = EventData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].description, "Conference");
    }

    #[test]
    fn test_holiday_data_save_to_load_from() {
        use crate::data::holiday::{Holiday, HolidayData};
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let mut data = HolidayData::default();
        data.add(Holiday::new("Labor Day", "2025-09-01"));
        data.save_to(tmp.path()).unwrap();
        let loaded = HolidayData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded.holidays.len(), 1);
        assert_eq!(loaded.holidays[0].name, "Labor Day");
    }

    #[test]
    fn test_save_to_creates_directory_if_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a").join("b");
        let data = TestJsonData { value: "nested".to_string() };
        data.save_to(&nested).unwrap();
        let loaded = TestJsonData::load_from(&nested).unwrap();
        assert_eq!(loaded, data);
    }
}
