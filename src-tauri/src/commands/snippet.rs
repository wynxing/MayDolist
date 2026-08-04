use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::events::emit_data_changed;
use crate::models::Snippet;
use crate::AppState;

#[tauri::command]
pub fn snippet_list(state: State<'_, AppState>) -> Vec<Snippet> {
    state.services.snippet.list()
}

#[tauri::command]
pub fn snippet_create(
    state: State<'_, AppState>,
    app: AppHandle,
    title: String,
    content: String,
    tags: Vec<String>,
) -> AppResult<Snippet> {
    let title = title.trim().to_string();
    let tags = normalize_tags(tags);
    let snippet = state.services.snippet.create(title, content, tags)?;
    emit_data_changed(&app, "snippet")?;
    Ok(snippet)
}

#[tauri::command]
pub fn snippet_update(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    title: Option<String>,
    content: Option<String>,
    tags: Option<Vec<String>>,
) -> AppResult<Snippet> {
    if title.as_deref().map(str::trim) == Some("") {
        return Err(AppError::InvalidInput("title must not be empty".into()));
    }
    let tags = tags.map(normalize_tags);
    let snippet = state.services.snippet.update(&id, title, content, tags)?;
    emit_data_changed(&app, "snippet")?;
    Ok(snippet)
}

#[tauri::command]
pub fn snippet_delete(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
) -> AppResult<()> {
    state.services.snippet.delete(&id)?;
    emit_data_changed(&app, "snippet")?;
    Ok(())
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect()
}
