use crate::{
    error::{AppError, AppResult},
    events::emit_entity_changed,
    AppState,
};
use serde::Serialize;
use tauri::{AppHandle, State};

/// Result of a quick capture submission. `kind` is "todo" or "note"; for todos
/// `target_list_id` carries the inbox list the item landed in.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuickCaptureResult {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub target_list_id: Option<String>,
}

/// Split a quick capture input into its kind and trimmed content.
/// `todo:` / `note:` prefixes are matched case-insensitively; anything else
/// (including no prefix) creates a Todo.
pub fn parse_quick_capture(input: &str) -> Result<(&'static str, &str), AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("input must not be empty".into()));
    }
    let lower = trimmed.to_ascii_lowercase();
    let (kind, rest) = if lower.starts_with("todo:") {
        ("todo", &trimmed["todo:".len()..])
    } else if lower.starts_with("note:") {
        ("note", &trimmed["note:".len()..])
    } else {
        ("todo", trimmed)
    };
    let content = rest.trim();
    if content.is_empty() {
        return Err(AppError::InvalidInput("content must not be empty".into()));
    }
    Ok((kind, content))
}

#[tauri::command]
pub fn quick_capture_submit(
    state: State<'_, AppState>,
    app: AppHandle,
    text: String,
) -> AppResult<QuickCaptureResult> {
    let (kind, content) = parse_quick_capture(&text)?;
    match kind {
        "todo" => {
            let list = state.services.todo.ensure_inbox()?;
            let item = state
                .services
                .todo
                .create_item(&list.id, content.to_string())?;
            emit_entity_changed(&app, "todoItem", &item.id, "created")?;
            Ok(QuickCaptureResult {
                kind: "todo".into(),
                id: item.id,
                title: item.title,
                target_list_id: Some(list.id),
            })
        }
        _ => {
            let note = state
                .services
                .note
                .create(content.to_string(), String::new())?;
            emit_entity_changed(&app, "note", &note.id, "created")?;
            Ok(QuickCaptureResult {
                kind: "note".into(),
                id: note.id,
                title: note.title,
                target_list_id: None,
            })
        }
    }
}

#[tauri::command]
pub fn quick_capture_hide(app: AppHandle) -> AppResult<()> {
    crate::app::hide_quick_capture(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_todo_prefix() {
        assert_eq!(
            parse_quick_capture("todo: 修复登录").unwrap(),
            ("todo", "修复登录")
        );
    }

    #[test]
    fn parses_note_prefix() {
        assert_eq!(
            parse_quick_capture("note: 记录一个想法").unwrap(),
            ("note", "记录一个想法")
        );
    }

    #[test]
    fn defaults_to_todo_without_prefix() {
        assert_eq!(
            parse_quick_capture("直接记录").unwrap(),
            ("todo", "直接记录")
        );
    }

    #[test]
    fn prefixes_are_case_insensitive_and_trimmed() {
        assert_eq!(
            parse_quick_capture("  NOTE:   大写前缀  ").unwrap(),
            ("note", "大写前缀")
        );
        assert_eq!(
            parse_quick_capture("TODO: 大写前缀").unwrap(),
            ("todo", "大写前缀")
        );
    }

    #[test]
    fn rejects_empty_and_whitespace_only_input() {
        assert!(parse_quick_capture("").is_err());
        assert!(parse_quick_capture("   ").is_err());
    }

    #[test]
    fn rejects_prefix_without_content() {
        assert!(parse_quick_capture("todo:").is_err());
        assert!(parse_quick_capture("note:   ").is_err());
    }
}
