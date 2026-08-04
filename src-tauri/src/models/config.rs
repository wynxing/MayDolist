use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// Single-instance application config, stored as `config.json` in the data dir.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    /// Reserved for a user-configurable data directory. The skeleton resolves
    /// the data dir from `MAYDOLIST_DATA_DIR` (env) or the default location.
    pub data_dir: Option<String>,
    /// Hot-corner used to summon the main panel (e.g. "top-right").
    pub hot_corner: String,
    /// Global hotkey for the main panel (e.g. "Ctrl+Alt+M").
    pub hotkey: String,
    pub theme: String,
    pub github_refresh_interval_minutes: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            data_dir: None,
            hot_corner: "top-right".into(),
            hotkey: "Ctrl+Alt+M".into(),
            theme: "acrylic-dark".into(),
            github_refresh_interval_minutes: 30,
        }
    }
}
