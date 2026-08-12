use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

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

fn default_command_palette_hotkey() -> String {
    "Ctrl+K".into()
}

fn default_command_palette_enabled() -> bool {
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

/// Optional quiet window for due reminders: no toast is shown between
/// `start` and `end` (local `HH:MM`, 24h). A window that crosses midnight
/// (e.g. 22:00–07:00) is supported. Invalid or equal start/end values are
/// treated as "no quiet hours" so a hand-edited config never blocks toasts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuietHours {
    pub start: String,
    pub end: String,
}

impl QuietHours {
    /// Parse an `HH:MM` value into `(hours, minutes)`.
    pub fn parse_time(value: &str) -> Option<(u32, u32)> {
        let mut parts = value.split(':');
        let hours: u32 = parts.next()?.trim().parse().ok()?;
        let minutes: u32 = parts.next()?.trim().parse().ok()?;
        if parts.next().is_some() || hours > 23 || minutes > 59 {
            return None;
        }
        Some((hours, minutes))
    }

    pub fn is_valid(&self) -> bool {
        Self::parse_time(&self.start).is_some() && Self::parse_time(&self.end).is_some()
    }

    /// Whether `now` falls inside the quiet window (end-exclusive; an
    /// equal start/end or unparseable values mean "no quiet hours").
    pub fn contains(&self, now: chrono::NaiveTime) -> bool {
        let (Some((sh, sm)), Some((eh, em))) =
            (Self::parse_time(&self.start), Self::parse_time(&self.end))
        else {
            return false;
        };
        let Some(start) = chrono::NaiveTime::from_hms_opt(sh, sm, 0) else {
            return false;
        };
        let Some(end) = chrono::NaiveTime::from_hms_opt(eh, em, 0) else {
            return false;
        };
        if start == end {
            return false;
        }
        if start < end {
            now >= start && now < end
        } else {
            now >= start || now < end
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        if !self.is_valid() {
            return Err(AppError::InvalidInput(
                "quiet hours must be HH:MM values (24h)".into(),
            ));
        }
        Ok(())
    }
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
    /// Global hotkey for the command palette window (e.g. "Ctrl+K").
    #[serde(default = "default_command_palette_hotkey")]
    pub command_palette_hotkey: String,
    /// Whether the command palette window and its hotkey are enabled.
    #[serde(default = "default_command_palette_enabled")]
    pub command_palette_enabled: bool,
    /// Days after which an open GitHub item is flagged "长期未更新".
    /// 0 disables the stale signal.
    #[serde(default = "default_github_stale_days")]
    pub github_stale_days: u32,
    /// Optional quiet window for due reminders; `None` keeps reminders
    /// always enabled (legacy behavior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet_hours: Option<QuietHours>,
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
        let palette_hotkey_before = self.command_palette_hotkey.clone();
        if self.command_palette_hotkey.trim().is_empty() {
            self.command_palette_hotkey = default_command_palette_hotkey();
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
        let quiet_before = self.quiet_hours.clone();
        if self
            .quiet_hours
            .as_ref()
            .is_some_and(|quiet| !quiet.is_valid())
        {
            self.quiet_hours = None;
        }
        schema_before != self.schema_version
            || quick_hotkey_before != self.quick_capture_hotkey
            || palette_hotkey_before != self.command_palette_hotkey
            || main_before != self.main_window_glass_opacity
            || floating_before != self.floating_note_glass_opacity
            || quiet_before != self.quiet_hours
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
            command_palette_hotkey: default_command_palette_hotkey(),
            command_palette_enabled: default_command_palette_enabled(),
            github_stale_days: default_github_stale_days(),
            quiet_hours: None,
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
    fn defaults_include_command_palette_settings() {
        let config = AppConfig::default();
        assert_eq!(config.command_palette_hotkey, "Ctrl+K");
        assert!(config.command_palette_enabled);
    }

    #[test]
    fn legacy_config_receives_command_palette_defaults() {
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
        assert_eq!(restored.command_palette_hotkey, "Ctrl+K");
        assert!(restored.command_palette_enabled);
        let json = serde_json::to_string(&restored).unwrap();
        assert!(json.contains("\"commandPaletteHotkey\":\"Ctrl+K\""));
        assert!(json.contains("\"commandPaletteEnabled\":true"));
    }

    #[test]
    fn command_palette_fields_roundtrip_through_serde_camel_case() {
        let config = AppConfig {
            command_palette_hotkey: "Ctrl+Shift+P".into(),
            command_palette_enabled: false,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"commandPaletteHotkey\":\"Ctrl+Shift+P\""));
        assert!(json.contains("\"commandPaletteEnabled\":false"));
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn sanitize_replaces_empty_command_palette_hotkey() {
        let mut config = AppConfig {
            command_palette_hotkey: "  ".into(),
            ..Default::default()
        };
        assert!(config.sanitize());
        assert_eq!(config.command_palette_hotkey, "Ctrl+K");
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

    #[test]
    fn quiet_hours_parse_and_validate() {
        assert_eq!(QuietHours::parse_time("22:30"), Some((22, 30)));
        assert_eq!(QuietHours::parse_time("07:05"), Some((7, 5)));
        assert_eq!(QuietHours::parse_time("24:00"), None);
        assert_eq!(QuietHours::parse_time("7:60"), None);
        assert_eq!(QuietHours::parse_time("abc"), None);
        let quiet = QuietHours {
            start: "22:00".into(),
            end: "07:00".into(),
        };
        assert!(quiet.is_valid());
        let bad = QuietHours {
            start: "25:00".into(),
            end: "07:00".into(),
        };
        assert!(!bad.is_valid());
        assert!(bad.validate().is_err());
    }

    #[test]
    fn quiet_hours_window_may_cross_midnight() {
        let quiet = QuietHours {
            start: "22:00".into(),
            end: "07:00".into(),
        };
        let time = |h: u32, m: u32| chrono::NaiveTime::from_hms_opt(h, m, 0).unwrap();
        assert!(quiet.contains(time(23, 30)));
        assert!(quiet.contains(time(6, 59)));
        assert!(!quiet.contains(time(7, 0)));
        assert!(!quiet.contains(time(12, 0)));
        assert!(!quiet.contains(time(21, 59)));
    }

    #[test]
    fn quiet_hours_within_same_day_and_equal_values() {
        let quiet = QuietHours {
            start: "09:00".into(),
            end: "18:00".into(),
        };
        let time = |h: u32, m: u32| chrono::NaiveTime::from_hms_opt(h, m, 0).unwrap();
        assert!(quiet.contains(time(9, 0)));
        assert!(quiet.contains(time(17, 59)));
        assert!(!quiet.contains(time(18, 0)));
        assert!(!quiet.contains(time(8, 59)));
        // Equal start/end means the window is empty -> never quiet.
        let empty = QuietHours {
            start: "10:00".into(),
            end: "10:00".into(),
        };
        assert!(!empty.contains(time(10, 0)));
    }

    #[test]
    fn legacy_config_receives_no_quiet_hours() {
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
        assert_eq!(restored.quiet_hours, None);
        let json = serde_json::to_string(&restored).unwrap();
        assert!(!json.contains("quietHours"));
    }

    #[test]
    fn quiet_hours_roundtrip_and_invalid_values_are_sanitized() {
        let quiet = Some(QuietHours {
            start: "22:00".into(),
            end: "07:00".into(),
        });
        let config = AppConfig {
            quiet_hours: quiet.clone(),
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"quietHours\":{\"start\":\"22:00\",\"end\":\"07:00\"}"));
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.quiet_hours, quiet);

        let mut invalid = AppConfig {
            quiet_hours: Some(QuietHours {
                start: "oops".into(),
                end: "07:00".into(),
            }),
            ..Default::default()
        };
        assert!(invalid.sanitize());
        assert_eq!(invalid.quiet_hours, None);
    }
}
