use crate::error::{AppError, AppResult};
use crate::models::palette::PaletteSearchResult;
use crate::AppState;
use tauri::{AppHandle, State};

/// Read-only aggregated search across todo / note / github local data plus
/// the static command list. Never touches the network and never writes.
#[tauri::command]
pub async fn palette_search(
    state: State<'_, AppState>,
    query: String,
) -> AppResult<PaletteSearchResult> {
    let palette = state.services.palette.clone();
    tauri::async_runtime::spawn_blocking(move || Ok(palette.search(&query)))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub fn palette_hide(app: AppHandle) -> AppResult<()> {
    crate::app::hide_command_palette(&app)
}
