use crate::{
    error::AppResult, events::emit_entity_changed, models::Note, services::note::NotePatch,
    AppState,
};
use tauri::{AppHandle, Manager, State};
#[tauri::command]
pub fn note_list(
    state: State<'_, AppState>,
    include_deleted: Option<bool>,
) -> AppResult<Vec<Note>> {
    state.services.note.list(include_deleted.unwrap_or(false))
}
#[tauri::command]
pub fn note_get(state: State<'_, AppState>, id: String) -> AppResult<Note> {
    state.services.note.get(&id)
}
#[tauri::command]
pub fn note_create(
    state: State<'_, AppState>,
    app: AppHandle,
    title: String,
    content: String,
) -> AppResult<Note> {
    let v = state.services.note.create(title, content)?;
    emit_entity_changed(&app, "note", &v.id, "created")?;
    Ok(v)
}
#[tauri::command]
pub fn note_update(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    patch: NotePatch,
) -> AppResult<Note> {
    let v = state.services.note.update(&id, patch)?;
    emit_entity_changed(&app, "note", &id, "updated")?;
    Ok(v)
}
#[tauri::command]
pub fn note_soft_delete(state: State<'_, AppState>, app: AppHandle, id: String) -> AppResult<()> {
    state.services.note.update(
        &id,
        NotePatch {
            deleted: Some(true),
            ..Default::default()
        },
    )?;
    emit_entity_changed(&app, "note", &id, "deleted")
}
#[tauri::command]
pub async fn note_show_floating(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
) -> AppResult<Note> {
    let v = state.services.note.update(
        &id,
        NotePatch {
            floating: Some(true),
            ..Default::default()
        },
    )?;
    // On Windows, `WebviewWindowBuilder::build()` deadlocks when called from a
    // synchronous command; Tauri runs async commands on a separate thread
    // where window creation is safe (see wry#583).
    crate::app::show_note(&app, &v)?;
    emit_entity_changed(&app, "note", &id, "floating")?;
    Ok(v)
}
#[tauri::command]
pub async fn note_dock(state: State<'_, AppState>, app: AppHandle, id: String) -> AppResult<Note> {
    if let Some(w) = app.get_webview_window(&format!("note-{id}")) {
        w.close().ok();
    }
    let v = state.services.note.update(
        &id,
        NotePatch {
            floating: Some(false),
            ..Default::default()
        },
    )?;
    emit_entity_changed(&app, "note", &id, "docked")?;
    Ok(v)
}
#[tauri::command]
pub fn note_update_window_state(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    patch: NotePatch,
) -> AppResult<Note> {
    let v = state.services.note.update(&id, patch)?;
    emit_entity_changed(&app, "note", &id, "window")?;
    Ok(v)
}
