use crate::{
    error::{AppError, AppResult},
    models::config::{GLASS_OPACITY_MAX, GLASS_OPACITY_MIN},
    models::AppConfig,
    AppState,
};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;

fn validate_glass_opacity(config: &AppConfig) -> AppResult<()> {
    for (name, value) in [
        ("mainWindowGlassOpacity", config.main_window_glass_opacity),
        (
            "floatingNoteGlassOpacity",
            config.floating_note_glass_opacity,
        ),
    ] {
        if !(GLASS_OPACITY_MIN..=GLASS_OPACITY_MAX).contains(&value) {
            return Err(AppError::InvalidInput(format!(
                "{name} must be between {} and {}",
                GLASS_OPACITY_MIN, GLASS_OPACITY_MAX
            )));
        }
    }
    Ok(())
}

fn sanitize_config(mut config: AppConfig) -> AppConfig {
    config.sanitize();
    config
}

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
    let config = sanitize_config(config);
    validate_glass_opacity(&config)?;
    crate::app::apply_hotkeys(&app, &config)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(main: f64, floating: f64) -> AppConfig {
        AppConfig {
            main_window_glass_opacity: main,
            floating_note_glass_opacity: floating,
            ..Default::default()
        }
    }

    #[test]
    fn accepts_boundary_opacity_values() {
        assert!(validate_glass_opacity(&config_with(GLASS_OPACITY_MIN, GLASS_OPACITY_MIN)).is_ok());
        assert!(validate_glass_opacity(&config_with(GLASS_OPACITY_MAX, GLASS_OPACITY_MAX)).is_ok());
        assert!(validate_glass_opacity(&config_with(0.5, 0.75)).is_ok());
    }

    #[test]
    fn rejects_opacity_below_minimum() {
        let err = validate_glass_opacity(&config_with(0.39, 0.5)).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("mainWindowGlassOpacity"));
    }

    #[test]
    fn rejects_opacity_above_maximum() {
        let err = validate_glass_opacity(&config_with(0.5, 1.01)).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("floatingNoteGlassOpacity"));
    }
}
