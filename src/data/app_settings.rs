use crate::data::persistence::Persistable;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppSettings {
    pub default_office: String,
    pub flex_credit: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            default_office: "McLean, VA".to_string(),
            flex_credit: "Flex Credit".to_string(),
        }
    }
}

/// Wrapper that reads the `settings` key from config.yaml.
/// `QuarterData` reads the same file for its `quarters` key — both work
/// independently because serde ignores unknown fields by default.
#[derive(Serialize, Deserialize, Default, Debug)]
struct SettingsWrapper {
    #[serde(default)]
    settings: AppSettings,
}

impl Persistable for SettingsWrapper {
    fn filename() -> &'static str {
        "config.yaml"
    }
    fn is_json() -> bool {
        false
    }
}

impl AppSettings {
    pub fn load() -> Result<Self> {
        Ok(SettingsWrapper::load()?.settings)
    }

    pub fn save(&self) -> Result<()> {
        let wrapper = SettingsWrapper {
            settings: self.clone(),
        };
        wrapper.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_settings_default_values() {
        let settings = AppSettings::default();
        assert_eq!(settings.default_office, "McLean, VA");
        assert_eq!(settings.flex_credit, "Flex Credit");
    }

    #[test]
    fn test_settings_wrapper_default() {
        let wrapper = SettingsWrapper::default();
        assert_eq!(wrapper.settings.default_office, "McLean, VA");
        assert_eq!(wrapper.settings.flex_credit, "Flex Credit");
    }

    #[test]
    fn test_app_settings_clone() {
        let s = AppSettings::default();
        let c = s.clone();
        assert_eq!(c.default_office, s.default_office);
        assert_eq!(c.flex_credit, s.flex_credit);
    }

    #[test]
    fn test_settings_wrapper_yaml_roundtrip() {
        let wrapper = SettingsWrapper {
            settings: AppSettings {
                default_office: "Remote HQ".to_string(),
                flex_credit: "Remote Credit".to_string(),
            },
        };
        let yaml = serde_norway::to_string(&wrapper).unwrap();
        let parsed: SettingsWrapper = serde_norway::from_str(&yaml).unwrap();
        assert_eq!(parsed.settings.default_office, "Remote HQ");
        assert_eq!(parsed.settings.flex_credit, "Remote Credit");
    }

    #[test]
    fn test_settings_wrapper_missing_key_uses_default() {
        // When config.yaml has no 'settings' key, default values kick in
        let yaml = "quarters: []";
        let wrapper: SettingsWrapper = serde_norway::from_str(yaml).unwrap();
        assert_eq!(wrapper.settings.default_office, "McLean, VA");
    }
}
