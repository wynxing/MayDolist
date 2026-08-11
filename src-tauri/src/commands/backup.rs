use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::{
    error::{AppError, AppResult},
    events::emit_entity_changed,
    services::backup::{BackupInfo, ExportInfo, ImportInfo, PackagePreview},
    AppState,
};

#[tauri::command]
pub fn backup_export(
    state: State<'_, AppState>,
    target: String,
    include_cache: bool,
) -> AppResult<ExportInfo> {
    let info = state
        .services
        .backup
        .export_to(&PathBuf::from(target), include_cache)?;
    state
        .log
        .log("info", &format!("exported data package to {}", info.path));
    Ok(info)
}

#[tauri::command]
pub fn backup_inspect(state: State<'_, AppState>, path: String) -> AppResult<PackagePreview> {
    state.services.backup.inspect(&PathBuf::from(path))
}

#[tauri::command]
pub fn backup_import(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> AppResult<ImportInfo> {
    let result = state.services.backup.import_from(&PathBuf::from(path));
    match result {
        Ok(info) => {
            state.log.log(
                "info",
                &format!(
                    "imported data package from {} (pre-import backup at {})",
                    info.path, info.backup_path
                ),
            );
            emit_entity_changed(&app, "todo", "*", "imported")?;
            emit_entity_changed(&app, "note", "*", "imported")?;
            emit_entity_changed(&app, "github", "*", "imported")?;
            let config = state.storage.load_config()?;
            app.emit("settings-changed", config.clone())
                .map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(info)
        }
        Err(err) => {
            state.log.log("error", &format!("import failed: {err}"));
            Err(err)
        }
    }
}

#[tauri::command]
pub fn backup_create(state: State<'_, AppState>) -> AppResult<BackupInfo> {
    let info = state.services.backup.create_backup()?;
    state
        .log
        .log("info", &format!("created backup at {}", info.path));
    Ok(info)
}

#[tauri::command]
pub fn backup_list(state: State<'_, AppState>) -> AppResult<Vec<BackupInfo>> {
    state.services.backup.list_backups()
}

#[tauri::command]
pub fn backup_open_data_dir(state: State<'_, AppState>) -> AppResult<()> {
    let dir = state.storage.data_dir();
    state.services.backup.open_data_dir()?;
    state
        .log
        .log("info", &format!("opened data dir {}", dir.display()));
    Ok(())
}
