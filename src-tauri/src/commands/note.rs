use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::events::emit_data_changed;
use crate::models::Note;
use crate::AppState;

#[tauri::command]
pub fn note_list(state: State<'_, AppState>) -> Vec<Note> {
    state.services.note.list()
}

#[tauri::command]
pub fn note_create(
    state: State<'_, AppState>,
    app: AppHandle,
    title: String,
    content: String,
) -> AppResult<Note> {
    let title = title.trim().to_string();
    let note = state.services.note.create(title, content)?;
    emit_data_changed(&app, "note")?;
    Ok(note)
}

#[tauri::command]
pub fn note_update(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    title: Option<String>,
    content: Option<String>,
) -> AppResult<Note> {
    if title.as_deref().map(str::trim) == Some("") {
        return Err(AppError::InvalidInput("title must not be empty".into()));
    }
    let note = state.services.note.update(&id, title, content)?;
    emit_data_changed(&app, "note")?;
    Ok(note)
}
