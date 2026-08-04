use crate::{
    error::AppResult,
    events::emit_entity_changed,
    models::{GhAuthStatus, RepoSnapshot, RepoWatch},
    AppState,
};
use tauri::{AppHandle, State};
#[tauri::command]
pub fn github_status(state: State<'_, AppState>) -> GhAuthStatus {
    state.services.github.status()
}
#[tauri::command]
pub fn github_watchlist(state: State<'_, AppState>) -> AppResult<Vec<RepoWatch>> {
    state.services.github.watchlist()
}
#[tauri::command]
pub fn github_watch_add(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
) -> AppResult<Vec<RepoWatch>> {
    let v = state.services.github.add_watch(full_name)?;
    emit_entity_changed(&app, "github", "watchlist", "updated")?;
    Ok(v)
}
#[tauri::command]
pub fn github_watch_remove(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
) -> AppResult<Vec<RepoWatch>> {
    let v = state.services.github.remove_watch(&full_name)?;
    emit_entity_changed(&app, "github", "watchlist", "updated")?;
    Ok(v)
}
#[tauri::command]
pub fn github_watch_filters(
    state: State<'_, AppState>,
    full_name: String,
    filters: Vec<String>,
) -> AppResult<Vec<RepoWatch>> {
    state.services.github.set_filters(&full_name, filters)
}
#[tauri::command]
pub fn github_refresh_repo(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
) -> AppResult<RepoSnapshot> {
    let v = state.services.github.refresh(&full_name)?;
    emit_entity_changed(&app, "github", &full_name, "refreshed")?;
    Ok(v)
}
#[tauri::command]
pub fn github_refresh_all(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<Vec<RepoSnapshot>> {
    let v = state.services.github.refresh_all()?;
    emit_entity_changed(&app, "github", "*", "refreshed")?;
    Ok(v)
}
#[tauri::command]
pub fn github_get_snapshot(
    state: State<'_, AppState>,
    full_name: String,
) -> AppResult<Option<RepoSnapshot>> {
    state.services.github.snapshot(&full_name)
}
