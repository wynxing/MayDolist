use crate::{
    error::AppResult,
    events::emit_entity_changed,
    models::{TodoItem, TodoList},
    AppState,
};
use tauri::{AppHandle, State};
#[tauri::command]
pub fn todo_list(
    state: State<'_, AppState>,
    include_deleted: Option<bool>,
) -> AppResult<Vec<TodoList>> {
    state.services.todo.list(include_deleted.unwrap_or(false))
}
#[tauri::command]
pub fn todo_create_list(
    state: State<'_, AppState>,
    app: AppHandle,
    title: String,
) -> AppResult<TodoList> {
    let v = state.services.todo.create_list(title)?;
    emit_entity_changed(&app, "todoList", &v.id, "created")?;
    Ok(v)
}
#[tauri::command]
pub fn todo_update_list(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    title: Option<String>,
    deleted: Option<bool>,
) -> AppResult<TodoList> {
    let v = state.services.todo.update_list(&id, title, deleted)?;
    emit_entity_changed(&app, "todoList", &id, "updated")?;
    Ok(v)
}
#[tauri::command]
pub fn todo_reorder_lists(
    state: State<'_, AppState>,
    app: AppHandle,
    ids: Vec<String>,
) -> AppResult<Vec<TodoList>> {
    let v = state.services.todo.reorder_lists(&ids)?;
    emit_entity_changed(&app, "todo", "*", "reordered")?;
    Ok(v)
}
#[tauri::command]
pub fn todo_create_item(
    state: State<'_, AppState>,
    app: AppHandle,
    list_id: String,
    title: String,
) -> AppResult<TodoItem> {
    let v = state.services.todo.create_item(&list_id, title)?;
    emit_entity_changed(&app, "todoItem", &v.id, "created")?;
    Ok(v)
}
#[tauri::command]
pub fn todo_update_item(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    title: Option<String>,
    completed: Option<bool>,
    deleted: Option<bool>,
) -> AppResult<TodoItem> {
    let v = state
        .services
        .todo
        .update_item(&id, title, completed, deleted)?;
    emit_entity_changed(&app, "todoItem", &id, "updated")?;
    Ok(v)
}
#[tauri::command]
pub fn todo_move_item(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    target_list_id: String,
    index: usize,
) -> AppResult<TodoItem> {
    let v = state.services.todo.move_item(&id, &target_list_id, index)?;
    emit_entity_changed(&app, "todoItem", &id, "moved")?;
    Ok(v)
}
#[tauri::command]
pub fn todo_reorder_items(
    state: State<'_, AppState>,
    app: AppHandle,
    list_id: String,
    ids: Vec<String>,
) -> AppResult<TodoList> {
    let v = state.services.todo.reorder_items(&list_id, &ids)?;
    emit_entity_changed(&app, "todo", &list_id, "reordered")?;
    Ok(v)
}
#[tauri::command]
pub fn todo_soft_delete(state: State<'_, AppState>, app: AppHandle, id: String) -> AppResult<()> {
    state
        .services
        .todo
        .update_item(&id, None, None, Some(true))?;
    emit_entity_changed(&app, "todoItem", &id, "deleted")
}
