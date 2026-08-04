use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// Single-instance application config, stored as `config.json` in the data dir.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    pub data_dir: String,
    /// Hot-corner used to summon the main panel (e.g. "top-right").
    pub hot_corner: String,
    /// Global hotkey for the main panel (e.g. "Ctrl+Alt+M").
    pub hotkey: String,
    pub theme: String,
    pub github_refresh_interval_minutes: u32,
    pub autostart: bool,
    pub first_run: bool,
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
        }
    }
}
