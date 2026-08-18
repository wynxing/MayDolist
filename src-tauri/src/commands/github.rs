use crate::{
    error::AppResult,
    events::emit_entity_changed,
    models::{GhAuthStatus, RepoSnapshot, RepoWatch},
    services::github::GithubSyncSummary,
    AppState,
};
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRefreshResult {
    pub snapshot: RepoSnapshot,
    pub sync: GithubSyncSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRefreshAllResult {
    pub snapshots: Vec<RepoSnapshot>,
    pub sync: GithubSyncSummary,
}

fn emit_sync_events(app: &AppHandle, summary: &GithubSyncSummary) {
    let auto_completed = summary
        .auto_completed_item_ids
        .iter()
        .collect::<std::collections::HashSet<_>>();
    for id in &summary.changed_item_ids {
        let operation = if auto_completed.contains(id) {
            "auto-completed"
        } else {
            "source-state-changed"
        };
        emit_entity_changed(app, "todoItem", id, operation).ok();
    }
    emit_entity_changed(
        app,
        "github",
        "*",
        if summary.failed > 0 {
            "sync-failed"
        } else {
            "status-synced"
        },
    )
    .ok();
}

#[tauri::command]
pub async fn github_status(state: State<'_, AppState>) -> crate::error::AppResult<GhAuthStatus> {
    let github = state.services.github.clone();
    tauri::async_runtime::spawn_blocking(move || github.status())
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))
}

#[tauri::command]
pub fn github_watchlist(state: State<'_, AppState>) -> AppResult<Vec<RepoWatch>> {
    state.services.github.watchlist()
}

#[tauri::command]
pub async fn github_watch_add(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
) -> AppResult<Vec<RepoWatch>> {
    let github = state.services.github.clone();
    let v = tauri::async_runtime::spawn_blocking(move || github.add_watch(full_name))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))??;
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
pub fn github_watch_signal_filters(
    state: State<'_, AppState>,
    full_name: String,
    filters: Vec<String>,
) -> AppResult<Vec<RepoWatch>> {
    state
        .services
        .github
        .set_signal_filters(&full_name, filters)
}

#[tauri::command]
pub fn github_watch_collapsed(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
    collapsed: bool,
) -> AppResult<Vec<RepoWatch>> {
    let v = state.services.github.set_collapsed(&full_name, collapsed)?;
    emit_entity_changed(&app, "github", "watchlist", "updated")?;
    Ok(v)
}

#[tauri::command]
pub fn github_ignore_item(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
    number: u64,
    kind: String,
) -> AppResult<Vec<RepoWatch>> {
    let v = state
        .services
        .github
        .ignore_item(&full_name, number, kind)?;
    emit_entity_changed(&app, "github", &full_name, "updated")?;
    Ok(v)
}

#[tauri::command]
pub async fn github_pin_item(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
    number: u64,
) -> AppResult<RepoSnapshot> {
    let github = state.services.github.clone();
    let repo = full_name.clone();
    let v = tauri::async_runtime::spawn_blocking(move || github.pin_item(&repo, number))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))??;
    emit_entity_changed(&app, "github", &full_name, "updated")?;
    Ok(v)
}

#[tauri::command]
pub fn github_unpin_item(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
    number: u64,
) -> AppResult<Vec<RepoWatch>> {
    let v = state.services.github.unpin_item(&full_name, number)?;
    emit_entity_changed(&app, "github", &full_name, "updated")?;
    Ok(v)
}

#[tauri::command]
pub async fn github_refresh_repo(
    state: State<'_, AppState>,
    app: AppHandle,
    full_name: String,
) -> AppResult<GithubRefreshResult> {
    let github = state.services.github.clone();
    let todo = state.services.todo.clone();
    let config = state.storage.load_config()?;
    let sync_enabled = config.github_sync_enabled;
    let auto_complete = config.github_auto_complete_todos;
    let repo = full_name.clone();
    let (v, sync) = tauri::async_runtime::spawn_blocking(move || {
        let snapshot = github.refresh(&repo)?;
        let sync = if sync_enabled {
            github.sync_linked_todos(&todo, auto_complete)
        } else {
            GithubSyncSummary::default()
        };
        Ok::<_, crate::error::AppError>((snapshot, sync))
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(e.to_string()))??;
    emit_entity_changed(&app, "github", &full_name, "refreshed")?;
    emit_sync_events(&app, &sync);
    Ok(GithubRefreshResult { snapshot: v, sync })
}

#[tauri::command]
pub async fn github_refresh_all(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<GithubRefreshAllResult> {
    let github = state.services.github.clone();
    let todo = state.services.todo.clone();
    let config = state.storage.load_config()?;
    let sync_enabled = config.github_sync_enabled;
    let auto_complete = config.github_auto_complete_todos;
    let (v, sync) = tauri::async_runtime::spawn_blocking(move || {
        let snapshots = github.refresh_all()?;
        let sync = if sync_enabled {
            github.sync_linked_todos(&todo, auto_complete)
        } else {
            GithubSyncSummary::default()
        };
        Ok::<_, crate::error::AppError>((snapshots, sync))
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(e.to_string()))??;
    emit_entity_changed(&app, "github", "*", "refreshed")?;
    emit_sync_events(&app, &sync);
    Ok(GithubRefreshAllResult { snapshots: v, sync })
}

#[tauri::command]
pub async fn github_sync_linked_todos(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<GithubSyncSummary> {
    let config = state.storage.load_config()?;
    if !config.github_sync_enabled {
        return Ok(GithubSyncSummary::default());
    }
    let github = state.services.github.clone();
    let todo = state.services.todo.clone();
    let auto_complete = config.github_auto_complete_todos;
    let summary = tauri::async_runtime::spawn_blocking(move || {
        github.sync_linked_todos(&todo, auto_complete)
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    emit_sync_events(&app, &summary);
    Ok(summary)
}

#[tauri::command]
pub fn github_get_snapshot(
    state: State<'_, AppState>,
    full_name: String,
) -> AppResult<Option<RepoSnapshot>> {
    state.services.github.snapshot(&full_name)
}
