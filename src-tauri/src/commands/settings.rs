use crate::{
    error::{AppError, AppResult},
    models::AppConfig,
    AppState,
};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;
#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<AppConfig> {
    state.storage.load_config()
}
#[tauri::command]
pub fn settings_update(
    state: State<'_, AppState>,
    app: AppHandle,
    config: AppConfig,
) -> AppResult<AppConfig> {
    crate::app::register_hotkey(&app, &config.hotkey)?;
    state.storage.save_config(&config)?;
    app.emit("settings-changed", config.clone())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(config)
}
#[tauri::command]
pub fn settings_migrate_data_dir(state: State<'_, AppState>, target: String) -> AppResult<String> {
    let target = PathBuf::from(target);
    state.storage.migrate(&target)?;
    let mut config = state.storage.load_config()?;
    config.data_dir = target.display().to_string();
    state.storage.save_config(&config)?;
    Ok(config.data_dir)
}
#[tauri::command]
pub fn settings_set_autostart(
    state: State<'_, AppState>,
    app: AppHandle,
    enabled: bool,
) -> AppResult<bool> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|e| AppError::Internal(e.to_string()))?;
    let mut config = state.storage.load_config()?;
    config.autostart = enabled;
    state.storage.save_config(&config)?;
    Ok(enabled)
}
