//! Global application preferences. These never authorize a hardware profile.
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguagePreference {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "en")]
    English,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppSettings {
    pub schema: u32,
    pub language: LanguagePreference,
    pub start_in_tray: bool,
    pub close_to_tray: bool,
    /// Mode or preset applied automatically without the charger, as a
    /// [`crate::battery::Selection`] key; `None` turns this off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_preset: Option<String>,
    /// Rewrite the applied preset's CPU limits and GPU flags when firmware
    /// changes them (see [`crate::hold`]). Off until the user turns it on.
    pub hold_preset: bool,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema: 1,
            language: LanguagePreference::System,
            start_in_tray: false,
            close_to_tray: true,
            battery_preset: None,
            hold_preset: false,
        }
    }
}
impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 {
            return Err(crate::message::msg("settings.invalid_schema", &[]));
        }
        if self
            .battery_preset
            .as_deref()
            .is_some_and(|key| crate::battery::Selection::parse(key).is_none())
        {
            return Err(crate::message::msg("settings.invalid_battery", &[]));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_close_preference_keeps_its_value_without_enabling_any_startup() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"schema":1,"close_to_tray":false}"#).unwrap();
        assert!(!settings.close_to_tray);
        assert!(!settings.start_in_tray);
        assert_eq!(settings.language, LanguagePreference::System);
        assert_eq!(settings.battery_preset, None);
        assert!(!settings.hold_preset);
    }
    #[test]
    fn battery_preset_is_a_valid_selection_key() {
        let mut settings = AppSettings {
            battery_preset: Some("mode:quiet".into()),
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
        settings.battery_preset = Some("mode:turbo".into());
        assert!(settings.validate().is_err());
        // Without a battery preset the file keeps its previous shape.
        assert!(
            !serde_json::to_string(&AppSettings::default())
                .unwrap()
                .contains("battery")
        );
    }
    #[test]
    fn unknown_preferences_and_future_schemas_cannot_be_overwritten_as_defaults() {
        assert!(serde_json::from_str::<AppSettings>(r#"{"schema":1,"language":"fr"}"#).is_err());
        assert!(serde_json::from_str::<AppSettings>(r#"{"schema":1,"future":true}"#).is_err());
        let settings: AppSettings = serde_json::from_str(r#"{"schema":2}"#).unwrap();
        assert!(settings.validate().is_err());
    }
}
