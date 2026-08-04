use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::events::emit_data_changed;
use crate::models::{GhAuthStatus, RepoSnapshot, RepoWatch};
use crate::AppState;

#[tauri::command]
pub fn github_auth_status(state: State<'_, AppState>) -> GhAuthStatus {
    state.services.github.auth_status()
}

#[tauri::command]
pub fn github_watchlist(state: State<'_, AppState>) -> Vec<RepoWatch> {
    state.services.github.watchlist()
}

#[tauri::command]
pub fn github_watch_add(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
) -> AppResult<Vec<RepoWatch>> {
    let watchlist = state.services.github.add_watch(full_name)?;
    emit_data_changed(&app, "github")?;
    Ok(watchlist)
}

#[tauri::command]
pub fn github_watch_remove(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
) -> AppResult<Vec<RepoWatch>> {
    let watchlist = state.services.github.remove_watch(full_name)?;
    emit_data_changed(&app, "github")?;
    Ok(watchlist)
}

#[tauri::command]
pub fn github_refresh(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<Vec<RepoSnapshot>> {
    let snapshots = state.services.github.refresh();
    emit_data_changed(&app, "github")?;
    Ok(snapshots)
}
