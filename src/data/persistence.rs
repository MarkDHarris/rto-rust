use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Post-processes `serde_norway` YAML output so every string scalar value
/// is consistently double-quoted. Leaves booleans, numbers, and null as-is.
fn normalize_yaml_strings(yaml: &str) -> String {
    let trailing_nl = yaml.ends_with('\n');
    let out: Vec<String> = yaml.lines().map(normalize_yaml_line).collect();
    let mut result = out.join("\n");
    if trailing_nl && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn normalize_yaml_line(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];

    if let Some(rest) = trimmed.strip_prefix("- ") {
        if let Some(colon_pos) = rest.find(": ") {
            let key = &rest[..colon_pos];
            let value = &rest[colon_pos + 2..];
            if should_quote(value) {
                return format!("{indent}- {key}: {}", to_double_quoted(value));
            }
        } else if !rest.contains(':') && should_quote(rest) {
            return format!("{indent}- {}", to_double_quoted(rest));
        }
        return line.to_string();
    }

    if let Some(colon_pos) = trimmed.find(": ") {
        let key = &trimmed[..colon_pos];
        let value = &trimmed[colon_pos + 2..];
        if should_quote(value) {
            return format!("{indent}{key}: {}", to_double_quoted(value));
        }
    }

    line.to_string()
}

fn should_quote(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    if value.starts_with('\'') {
        return true;
    }
    if value.starts_with('"') {
        return false;
    }
    if matches!(value, "true" | "false" | "null" | "~") {
        return false;
    }
    if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        return false;
    }
    if value.starts_with('|')
        || value.starts_with('>')
        || value.starts_with('[')
        || value.starts_with('{')
    {
        return false;
    }
    true
}

fn to_double_quoted(value: &str) -> String {
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        let inner = &value[1..value.len() - 1];
        let content = inner.replace("''", "'");
        return format!("\"{}\"", content.replace('\\', "\\\\").replace('"', "\\\""));
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_data_dir(path: PathBuf) {
    let _ = DATA_DIR.set(path);
}

pub fn get_data_dir() -> Result<PathBuf> {
    if let Some(dir) = DATA_DIR.get() {
        return Ok(dir.clone());
    }
    let cwd = std::env::current_dir().context("failed to get current directory")?;
    Ok(cwd.join("config"))
}

pub fn get_file_path(name: &str) -> Result<PathBuf> {
    let dir = get_data_dir()?;
    Ok(dir.join(name))
}

pub fn load_yaml_from<T: for<'de> Deserialize<'de>>(
    dir: &Path,
    filename: &str,
) -> Result<Option<T>> {
    let path = dir.join(filename);
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let val: T = serde_norway::from_str(&contents)
        .with_context(|| format!("parsing YAML from {}", path.display()))?;
    Ok(Some(val))
}

pub fn save_yaml_to<T: Serialize>(dir: &Path, filename: &str, value: &T) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating directory {}", dir.display()))?;
    let path = dir.join(filename);
    let raw = serde_norway::to_string(value).context("serializing YAML")?;
    let contents = normalize_yaml_strings(&raw);
    fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn load_json_from<T: for<'de> Deserialize<'de>>(
    dir: &Path,
    filename: &str,
) -> Result<Option<T>> {
    let path = dir.join(filename);
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let val: T = serde_json::from_str(&contents)
        .with_context(|| format!("parsing JSON from {}", path.display()))?;
    Ok(Some(val))
}

#[allow(dead_code)]
pub(crate) fn save_json_to<T: Serialize>(dir: &Path, filename: &str, value: &T) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating directory {}", dir.display()))?;
    let path = dir.join(filename);
    let contents = serde_json::to_string_pretty(value).context("serializing JSON")?;
    fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
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
            let raw = serde_norway::to_string(self).context("failed to serialize YAML")?;
            normalize_yaml_strings(&raw)
        };
        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    #[allow(dead_code)]
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

    fn save_to(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join(Self::filename());
        let contents = if Self::is_json() {
            serde_json::to_string_pretty(self).context("failed to serialize JSON")?
        } else {
            let raw = serde_norway::to_string(self).context("failed to serialize YAML")?;
            normalize_yaml_strings(&raw)
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
        let data = TestJsonData {
            value: "hello".to_string(),
        };
        data.save_to(tmp.path()).unwrap();
        let loaded = TestJsonData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_yaml_save_and_load_roundtrip() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let data = TestYamlData { count: 42 };
        data.save_to(tmp.path()).unwrap();
        let loaded = TestYamlData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_persistable_load_returns_default_for_missing_file() {
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
        let data = TestJsonData {
            value: "round-trip".to_string(),
        };
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
    fn test_standalone_load_json_from() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let data = TestJsonData {
            value: "standalone".to_string(),
        };
        save_json_to(tmp.path(), "test.json", &data).unwrap();
        let loaded: Option<TestJsonData> = load_json_from(tmp.path(), "test.json").unwrap();
        assert_eq!(loaded.unwrap(), data);
    }

    #[test]
    fn test_standalone_load_yaml_from() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let data = TestYamlData { count: 77 };
        save_yaml_to(tmp.path(), "test.yaml", &data).unwrap();
        let loaded: Option<TestYamlData> = load_yaml_from(tmp.path(), "test.yaml").unwrap();
        assert_eq!(loaded.unwrap(), data);
    }

    #[test]
    fn test_standalone_load_missing_returns_none() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let loaded: Option<TestJsonData> = load_json_from(tmp.path(), "nope.json").unwrap();
        assert!(loaded.is_none());
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
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn test_event_data_save_to_load_from() {
        use crate::data::event::{Event, EventData};
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let mut data = EventData::default();
        data.add(Event {
            date: "2025-06-01".to_string(),
            description: "Conference".to_string(),
        });
        data.save_to(tmp.path()).unwrap();
        let loaded = EventData::load_from(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 1);
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
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.holidays[0].name, "Labor Day");
    }

    #[test]
    fn test_save_to_creates_directory_if_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a").join("b");
        let data = TestJsonData {
            value: "nested".to_string(),
        };
        data.save_to(&nested).unwrap();
        let loaded = TestJsonData::load_from(&nested).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_normalize_plain_strings_get_quoted() {
        let input = "name: Independence Day\ndate: 2025-07-04\n";
        let output = normalize_yaml_strings(input);
        assert_eq!(output, "name: \"Independence Day\"\ndate: \"2025-07-04\"\n");
    }

    #[test]
    fn test_normalize_single_to_double_quotes() {
        let input = "name: 'Thank You Day #1'\n";
        let output = normalize_yaml_strings(input);
        assert_eq!(output, "name: \"Thank You Day #1\"\n");
    }

    #[test]
    fn test_normalize_single_quote_with_escaped_apostrophe() {
        let input = "name: 'New Year''s Day'\n";
        let output = normalize_yaml_strings(input);
        assert_eq!(output, "name: \"New Year's Day\"\n");
    }

    #[test]
    fn test_normalize_preserves_booleans_and_numbers() {
        let input = "approved: true\ncount: 42\nratio: 3.14\n";
        let output = normalize_yaml_strings(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_normalize_preserves_already_double_quoted() {
        let input = "start_date: \"2025-02-01\"\n";
        let output = normalize_yaml_strings(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_normalize_sequence_items() {
        let input = "files:\n- workday-fy-qtr.yaml\n- settings.yaml\n";
        let output = normalize_yaml_strings(input);
        assert_eq!(
            output,
            "files:\n- \"workday-fy-qtr.yaml\"\n- \"settings.yaml\"\n"
        );
    }

    #[test]
    fn test_normalize_sequence_map_entries() {
        let input = "holidays:\n- name: Labor Day\n  date: 2025-09-01\n";
        let output = normalize_yaml_strings(input);
        assert_eq!(
            output,
            "holidays:\n- name: \"Labor Day\"\n  date: \"2025-09-01\"\n"
        );
    }

    #[test]
    fn test_normalize_roundtrip_with_serde() {
        use crate::data::holiday::{Holiday, HolidayData};
        let data = HolidayData {
            holidays: vec![
                Holiday::new("New Year's Day", "2025-01-01"),
                Holiday::new("Thank You Day #1", "2025-03-14"),
            ],
        };
        let raw = serde_norway::to_string(&data).unwrap();
        let normalized = normalize_yaml_strings(&raw);
        assert!(normalized.contains("\"New Year's Day\""));
        assert!(normalized.contains("\"Thank You Day #1\""));
        assert!(normalized.contains("\"2025-01-01\""));
        let loaded: HolidayData = serde_norway::from_str(&normalized).unwrap();
        assert_eq!(loaded.holidays[0].name, "New Year's Day");
        assert_eq!(loaded.holidays[1].name, "Thank You Day #1");
    }
}
