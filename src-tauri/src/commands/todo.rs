use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::events::emit_data_changed;
use crate::models::{TodoItem, TodoList};
use crate::AppState;

#[tauri::command]
pub fn todo_list(state: State<'_, AppState>) -> Vec<TodoList> {
    state.services.todo.list()
}

#[tauri::command]
pub fn todo_create_list(
    state: State<'_, AppState>,
    app: AppHandle,
    title: String,
) -> AppResult<TodoList> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::InvalidInput("title must not be empty".into()));
    }
    let list = state.services.todo.create_list(title)?;
    emit_data_changed(&app, "todo")?;
    Ok(list)
}

#[tauri::command]
pub fn todo_create_item(
    state: State<'_, AppState>,
    app: AppHandle,
    list_id: String,
    title: String,
) -> AppResult<TodoItem> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::InvalidInput("title must not be empty".into()));
    }
    let item = state.services.todo.create_item(&list_id, title)?;
    emit_data_changed(&app, "todo")?;
    Ok(item)
}

#[tauri::command]
pub fn todo_update_item(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    title: Option<String>,
    completed: Option<bool>,
) -> AppResult<TodoItem> {
    let item = state.services.todo.update_item(&id, title, completed)?;
    emit_data_changed(&app, "todo")?;
    Ok(item)
}

#[tauri::command]
pub fn todo_soft_delete(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
) -> AppResult<()> {
    state.services.todo.soft_delete_item(&id)?;
    emit_data_changed(&app, "todo")?;
    Ok(())
}
