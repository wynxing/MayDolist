use crate::{
    error::{AppError, AppResult},
    events::emit_entity_changed,
    models::{Note, TodoItem, TodoList},
    services::note::NotePatch,
    AppState,
};
use serde::Serialize;
use tauri::{AppHandle, State};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trash {
    todo_lists: Vec<TodoList>,
    todo_items: Vec<TodoItem>,
    notes: Vec<Note>,
}
#[tauri::command]
pub fn trash_list(state: State<'_, AppState>) -> AppResult<Trash> {
    // Trash is rarely opened; materialize owned copies from the shared cache.
    let lists = (*state.services.todo.list(true)?).clone();
    let notes = (*state.services.note.list(true)?).clone();
    Ok(Trash {
        todo_lists: lists.iter().filter(|v| v.deleted).cloned().collect(),
        todo_items: lists
            .into_iter()
            .flat_map(|v| v.items)
            .filter(|v| v.deleted)
            .collect(),
        notes: notes.into_iter().filter(|v| v.deleted).collect(),
    })
}
#[tauri::command]
pub fn trash_restore(
    state: State<'_, AppState>,
    app: AppHandle,
    kind: String,
    id: String,
) -> AppResult<()> {
    match kind.as_str() {
        "todoList" => {
            state.services.todo.update_list(&id, None, Some(false))?;
        }
        "todoItem" => {
            state
                .services
                .todo
                .update_item(&id, None, None, Some(false), None)?;
        }
        "note" => {
            state.services.note.update(
                &id,
                NotePatch {
                    deleted: Some(false),
                    ..Default::default()
                },
            )?;
        }
        _ => return Err(AppError::InvalidInput("invalid trash kind".into())),
    };
    emit_entity_changed(&app, &kind, &id, "restored")
}
#[tauri::command]
pub fn trash_delete_permanently(
    state: State<'_, AppState>,
    app: AppHandle,
    kind: String,
    id: String,
) -> AppResult<()> {
    match kind.as_str() {
        "todoList" | "todoItem" => state.services.todo.permanent_delete(&kind, &id)?,
        "note" => state.services.note.permanent_delete(&id)?,
        _ => return Err(AppError::InvalidInput("invalid trash kind".into())),
    };
    emit_entity_changed(&app, &kind, &id, "purged")
}

#[tauri::command]
pub fn trash_clear(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    let trash = trash_list(state.clone())?;
    // Items first so list-file deletes do not leave dangling item purges.
    for item in &trash.todo_items {
        state.services.todo.permanent_delete("todoItem", &item.id)?;
        emit_entity_changed(&app, "todoItem", &item.id, "purged")?;
    }
    for list in &trash.todo_lists {
        state.services.todo.permanent_delete("todoList", &list.id)?;
        emit_entity_changed(&app, "todoList", &list.id, "purged")?;
    }
    for note in &trash.notes {
        state.services.note.permanent_delete(&note.id)?;
        emit_entity_changed(&app, "note", &note.id, "purged")?;
    }
    Ok(())
}
