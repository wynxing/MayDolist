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

fn default_quick_capture_hotkey() -> String {
    "Ctrl+Alt+Space".into()
}

fn default_quick_capture_enabled() -> bool {
    true
}

fn default_github_stale_days() -> u32 {
    14
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
    /// Global hotkey for the quick capture window (e.g. "Ctrl+Alt+Space").
    #[serde(default = "default_quick_capture_hotkey")]
    pub quick_capture_hotkey: String,
    /// Whether the quick capture window and its hotkey are enabled.
    #[serde(default = "default_quick_capture_enabled")]
    pub quick_capture_enabled: bool,
    /// Days after which an open GitHub item is flagged "长期未更新".
    /// 0 disables the stale signal.
    #[serde(default = "default_github_stale_days")]
    pub github_stale_days: u32,
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
        let quick_hotkey_before = self.quick_capture_hotkey.clone();
        if self.quick_capture_hotkey.trim().is_empty() {
            self.quick_capture_hotkey = default_quick_capture_hotkey();
        }
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
            || quick_hotkey_before != self.quick_capture_hotkey
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
            quick_capture_hotkey: default_quick_capture_hotkey(),
            quick_capture_enabled: default_quick_capture_enabled(),
            github_stale_days: default_github_stale_days(),
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
    fn defaults_include_quick_capture_settings() {
        let config = AppConfig::default();
        assert_eq!(config.quick_capture_hotkey, "Ctrl+Alt+Space");
        assert!(config.quick_capture_enabled);
        assert_eq!(config.github_stale_days, 14);
    }

    #[test]
    fn legacy_config_receives_stale_days_default() {
        let json = r#"{
            "schemaVersion": 2,
            "dataDir": null,
            "hotCorner": "top-right",
            "hotkey": "Ctrl+Alt+M",
            "theme": "dark",
            "githubRefreshIntervalMinutes": 30,
            "autostart": false,
            "firstRun": true
        }"#;
        let restored: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(restored.github_stale_days, 14);
        let json = serde_json::to_string(&restored).unwrap();
        assert!(json.contains("\"githubStaleDays\":14"));
    }

    #[test]
    fn stale_days_roundtrips_and_zero_is_preserved() {
        let config = AppConfig {
            github_stale_days: 0,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.github_stale_days, 0);
    }

    #[test]
    fn sanitize_replaces_empty_quick_capture_hotkey() {
        let mut config = AppConfig {
            quick_capture_hotkey: "  ".into(),
            ..Default::default()
        };
        assert!(config.sanitize());
        assert_eq!(config.quick_capture_hotkey, "Ctrl+Alt+Space");
    }

    #[test]
    fn quick_capture_fields_roundtrip_through_serde_camel_case() {
        let config = AppConfig {
            quick_capture_hotkey: "Ctrl+Shift+Q".into(),
            quick_capture_enabled: false,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"quickCaptureHotkey\":\"Ctrl+Shift+Q\""));
        assert!(json.contains("\"quickCaptureEnabled\":false"));
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, config);
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
        assert_eq!(restored.quick_capture_hotkey, "Ctrl+Alt+Space");
        assert!(restored.quick_capture_enabled);
        assert_eq!(restored.main_window_glass_opacity, 0.52);
        assert_eq!(restored.floating_note_glass_opacity, 0.46);
        assert!(restored.sanitize());
        assert_eq!(restored.schema_version, CONFIG_SCHEMA_VERSION);
    }
}
