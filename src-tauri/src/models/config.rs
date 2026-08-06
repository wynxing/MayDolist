use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 2;

/// Allowed range for glass background opacity (40%..=100%).
pub const GLASS_OPACITY_MIN: f64 = 0.4;
pub const GLASS_OPACITY_MAX: f64 = 1.0;

fn default_main_window_glass_opacity() -> f64 {
    0.52
}

fn default_floating_note_glass_opacity() -> f64 {
    0.46
}

fn deserialize_string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

/// Single-instance application config, stored as `config.json` in the data dir.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub data_dir: String,
    /// Hot-corner used to summon the main panel (e.g. "top-right").
    pub hot_corner: String,
    /// Global hotkey for the main panel (e.g. "Ctrl+Alt+M").
    pub hotkey: String,
    pub theme: String,
    pub github_refresh_interval_minutes: u32,
    pub autostart: bool,
    pub first_run: bool,
    /// Glass background opacity (0.4..=1.0) of the main panel window.
    #[serde(default = "default_main_window_glass_opacity")]
    pub main_window_glass_opacity: f64,
    /// Glass background opacity (0.4..=1.0) of floating note windows.
    #[serde(default = "default_floating_note_glass_opacity")]
    pub floating_note_glass_opacity: f64,
}

impl AppConfig {
    /// Clamp glass opacity fields into the supported range and report whether
    /// anything was changed. Called on load so hand-edited config.json values
    /// outside 0.4..=1.0 are corrected instead of leaving an unreadable UI.
    pub fn sanitize(&mut self) -> bool {
        let schema_before = self.schema_version;
        self.schema_version = CONFIG_SCHEMA_VERSION;
        let main_before = self.main_window_glass_opacity;
        self.main_window_glass_opacity = if self.main_window_glass_opacity.is_finite() {
            self.main_window_glass_opacity
                .clamp(GLASS_OPACITY_MIN, GLASS_OPACITY_MAX)
        } else {
            default_main_window_glass_opacity()
        };
        let floating_before = self.floating_note_glass_opacity;
        self.floating_note_glass_opacity = if self.floating_note_glass_opacity.is_finite() {
            self.floating_note_glass_opacity
                .clamp(GLASS_OPACITY_MIN, GLASS_OPACITY_MAX)
        } else {
            default_floating_note_glass_opacity()
        };
        schema_before != self.schema_version
            || main_before != self.main_window_glass_opacity
            || floating_before != self.floating_note_glass_opacity
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            data_dir: String::new(),
            hot_corner: "top-right".into(),
            hotkey: "Ctrl+Alt+M".into(),
            theme: "system".into(),
            github_refresh_interval_minutes: 30,
            autostart: false,
            first_run: true,
            main_window_glass_opacity: default_main_window_glass_opacity(),
            floating_note_glass_opacity: default_floating_note_glass_opacity(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_use_supported_opacity_values() {
        let config = AppConfig::default();
        assert!(config.schema_version >= CONFIG_SCHEMA_VERSION);
        assert!((GLASS_OPACITY_MIN..=GLASS_OPACITY_MAX).contains(&config.main_window_glass_opacity));
        assert!(
            (GLASS_OPACITY_MIN..=GLASS_OPACITY_MAX).contains(&config.floating_note_glass_opacity)
        );
    }

    #[test]
    fn sanitize_clamps_out_of_range_opacity() {
        let mut config = AppConfig {
            main_window_glass_opacity: 0.2,
            floating_note_glass_opacity: 1.5,
            ..Default::default()
        };
        assert!(config.sanitize());
        assert_eq!(config.main_window_glass_opacity, GLASS_OPACITY_MIN);
        assert_eq!(config.floating_note_glass_opacity, GLASS_OPACITY_MAX);
    }

    #[test]
    fn sanitize_keeps_in_range_opacity_unchanged() {
        let mut config = AppConfig {
            main_window_glass_opacity: 0.4,
            floating_note_glass_opacity: 1.0,
            ..Default::default()
        };
        assert!(!config.sanitize());
        assert_eq!(config.main_window_glass_opacity, 0.4);
        assert_eq!(config.floating_note_glass_opacity, 1.0);
    }

    #[test]
    fn sanitize_replaces_non_finite_opacity_values() {
        let mut config = AppConfig {
            main_window_glass_opacity: f64::NAN,
            floating_note_glass_opacity: f64::INFINITY,
            ..Default::default()
        };
        assert!(config.sanitize());
        assert_eq!(config.main_window_glass_opacity, 0.52);
        assert_eq!(config.floating_note_glass_opacity, 0.46);
    }

    #[test]
    fn opacity_fields_roundtrip_through_serde_camel_case() {
        let config = AppConfig {
            main_window_glass_opacity: 0.66,
            floating_note_glass_opacity: 0.42,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"mainWindowGlassOpacity\":0.66"));
        assert!(json.contains("\"floatingNoteGlassOpacity\":0.42"));
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn legacy_config_receives_glass_defaults_without_losing_existing_values() {
        let json = r#"{
            "schemaVersion": 1,
            "dataDir": "D:\\MayDolist-data",
            "hotCorner": "bottom-left",
            "hotkey": "Ctrl+Shift+M",
            "theme": "light",
            "githubRefreshIntervalMinutes": 15,
            "autostart": true,
            "firstRun": false
        }"#;
        let mut restored: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(restored.theme, "light");
        assert_eq!(restored.hotkey, "Ctrl+Shift+M");
        assert_eq!(restored.main_window_glass_opacity, 0.52);
        assert_eq!(restored.floating_note_glass_opacity, 0.46);
        assert!(restored.sanitize());
        assert_eq!(restored.schema_version, CONFIG_SCHEMA_VERSION);
    }
}
