use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::events::emit_data_changed;
use crate::models::{AppConfig, CONFIG_SCHEMA_VERSION};
use crate::AppState;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppResult<AppConfig> {
    state.storage.load_config()
}

#[tauri::command]
pub fn get_data_dir(state: State<'_, AppState>) -> String {
    state.storage.data_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn set_config(
    state: State<'_, AppState>,
    app: AppHandle,
    config: AppConfig,
) -> AppResult<AppConfig> {
    validate_config(&config)?;
    state.storage.save_config(&config)?;
    emit_data_changed(&app, "config")?;
    Ok(config)
}

fn validate_config(config: &AppConfig) -> AppResult<()> {
    if config.schema_version != CONFIG_SCHEMA_VERSION {
        return Err(AppError::InvalidInput(format!(
            "unsupported schemaVersion {}",
            config.schema_version
        )));
    }
    if config.hot_corner.trim().is_empty()
        || config.hotkey.trim().is_empty()
        || config.theme.trim().is_empty()
    {
        return Err(AppError::InvalidInput("config fields must not be empty".into()));
    }
    if config.github_refresh_interval_minutes == 0 {
        return Err(AppError::InvalidInput(
            "githubRefreshIntervalMinutes must be > 0".into(),
        ));
    }
    Ok(())
}
