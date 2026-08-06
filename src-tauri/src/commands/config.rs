use crate::{error::AppResult, models::AppConfig, AppState};
use tauri::State;
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppResult<AppConfig> {
    state.storage.load_config()
}
#[tauri::command]
pub fn get_data_dir(state: State<'_, AppState>) -> String {
    state.storage.data_dir().display().to_string()
}
#[tauri::command]
pub fn set_config(state: State<'_, AppState>, config: AppConfig) -> AppResult<AppConfig> {
    let mut config = config;
    config.sanitize();
    state.storage.save_config(&config)?;
    Ok(config)
}
