use crate::{error::AppResult, models::FocusOverview, AppState};
use tauri::State;

/// Read-only Focus aggregation. Loads todo / note / github in parallel and
/// isolates per-domain failures; never writes into the domain stores.
#[tauri::command]
pub async fn focus_overview(state: State<'_, AppState>) -> AppResult<FocusOverview> {
    let focus = state.services.focus.clone();
    tauri::async_runtime::spawn_blocking(move || Ok(focus.overview()))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}
