use crate::{
    error::AppResult,
    events::emit_entity_changed,
    models::{RepeatRule, TodoItem, TodoList, TodoSource},
    AppState,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

/// Optional due / reminder / repeat fields sent from the frontend. The UI
/// always sends the complete schedule when editing, so `null` means "cleared"
/// and an omitted `schedule` key means "leave unchanged".
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TodoScheduleInput {
    pub due_date: Option<String>,
    pub remind_at: Option<String>,
    pub repeat: Option<RepeatRule>,
    pub repeat_until: Option<String>,
}

/// Update payload for `todo_update_item`. `schedule` is optional so callers
/// that only rename / complete / delete never touch the due fields.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TodoPatchInput {
    pub title: Option<String>,
    pub completed: Option<bool>,
    pub deleted: Option<bool>,
    pub schedule: Option<TodoScheduleInput>,
}
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
    source: Option<TodoSource>,
    schedule: Option<TodoScheduleInput>,
) -> AppResult<TodoItem> {
    let schedule = schedule.unwrap_or_default();
    let v = state.services.todo.create_item(
        &list_id,
        title,
        source,
        crate::services::todo::TodoSchedule {
            due_date: schedule.due_date,
            remind_at: schedule.remind_at,
            repeat: schedule.repeat,
            repeat_until: schedule.repeat_until,
        },
    )?;
    emit_entity_changed(&app, "todoItem", &v.id, "created")?;
    Ok(v)
}

/// Result of converting a GitHub issue / PR into a Todo. `source_type` is the
/// source kind (`"github-pr"` / `"github-issue"`) and `target_list_id` is the
/// inbox list the item landed in.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoFromGithubResult {
    pub source_type: String,
    pub id: String,
    pub title: String,
    pub repo: String,
    pub number: u64,
    pub target_list_id: String,
    pub already_existed: bool,
}

/// Create a Todo from a GitHub issue / PR. The item always lands in the
/// capture inbox (idempotent `ensure_inbox`); only the Todo domain is
/// touched, so GitHub cache, auth and network state are never modified.
#[tauri::command]
pub fn todo_create_from_github(
    state: State<'_, AppState>,
    app: AppHandle,
    kind: String,
    repo: String,
    number: u64,
    title: String,
    url: String,
) -> AppResult<TodoFromGithubResult> {
    let (list, item, created) = state
        .services
        .todo
        .create_item_from_github(&kind, &repo, number, &title, &url)?;
    if created {
        emit_entity_changed(&app, "todoItem", &item.id, "created")?;
    }
    Ok(TodoFromGithubResult {
        source_type: kind,
        id: item.id,
        title: item.title,
        repo,
        number,
        target_list_id: list.id,
        already_existed: !created,
    })
}
#[tauri::command]
pub fn todo_update_item(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    patch: TodoPatchInput,
) -> AppResult<TodoItem> {
    let v = state.services.todo.update_item(
        &id,
        patch.title,
        patch.completed,
        patch.deleted,
        patch
            .schedule
            .map(|schedule| crate::services::todo::TodoSchedule {
                due_date: schedule.due_date,
                remind_at: schedule.remind_at,
                repeat: schedule.repeat,
                repeat_until: schedule.repeat_until,
            }),
    )?;
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
        .update_item(&id, None, None, Some(true), None)?;
    emit_entity_changed(&app, "todoItem", &id, "deleted")
}
