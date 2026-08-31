use crate::{
    error::{AppError, AppResult},
    events::emit_entity_changed,
    AppState,
};
use chrono::{Datelike, Duration, Local, NaiveDate};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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
/// `/note` opens a new floating note; a trailing title (`/note 想法`)
/// creates the note with that title. `todo:` remains an optional Todo
/// prefix; all other text, including the former `note:` syntax, creates a Todo.
pub fn parse_quick_capture(input: &str) -> Result<(&'static str, &str), AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("input must not be empty".into()));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower == "/note" {
        return Ok(("note", ""));
    }
    if let Some(rest) = lower.strip_prefix("/note") {
        if rest.starts_with(char::is_whitespace) {
            // `/note <title>`: the remainder becomes the note title.
            return Ok(("note", trimmed["/note".len()..].trim()));
        }
    }
    let (kind, rest) = if lower.starts_with("todo:") {
        ("todo", &trimmed["todo:".len()..])
    } else {
        ("todo", trimmed)
    };
    let content = rest.trim();
    if content.is_empty() {
        return Err(AppError::InvalidInput("content must not be empty".into()));
    }
    Ok((kind, content))
}

/// Parse a leading natural-language due-date token from a quick capture
/// input. Supported minimal grammar: `明天` / `今天` / `后天` / `周X` /
/// `星期X` / `下周X` (X = 一..六日天) / `N天后` (1..=365) / `月底` / `月末`.
/// A token must be followed by whitespace or the end of input so words like
/// "明天性计划" never match. Returns the matched token and the due date, or
/// `None` — callers degrade to a plain Todo.
pub fn parse_quick_capture_due(input: &str, today: NaiveDate) -> Option<(String, NaiveDate)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    for (token, delta) in [
        ("明天", 1i64),
        ("明日", 1),
        ("今天", 0),
        ("今日", 0),
        ("后天", 2),
    ] {
        if let Some(rest) = trimmed.strip_prefix(token) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                return Some((token.to_string(), today + Duration::days(delta)));
            }
        }
    }
    if let Some(found) = parse_weekday_prefix(trimmed, today) {
        return Some(found);
    }
    if let Some(found) = parse_n_days_later(trimmed, today) {
        return Some(found);
    }
    for token in ["月底", "月末"] {
        if let Some(rest) = trimmed.strip_prefix(token) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                let last = crate::models::todo::last_day_of_month(today.year(), today.month());
                let due = NaiveDate::from_ymd_opt(today.year(), today.month(), last)?;
                return Some((token.to_string(), due));
            }
        }
    }
    None
}

fn parse_weekday_prefix(input: &str, today: NaiveDate) -> Option<(String, NaiveDate)> {
    const DAYS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];
    // `下周X` resolves the same weekday shifted one full week later.
    for (prefix, week_offset) in [("下周", 7i64), ("星期", 0), ("周", 0)] {
        let Some(rest) = input.strip_prefix(prefix) else {
            continue;
        };
        let day = rest.chars().next()?.to_string();
        // "周天" / "星期天" are common aliases for Sunday.
        let index = if day == "天" {
            6
        } else {
            DAYS.iter().position(|v| **v == day)?
        };
        let token: String = input.chars().take(prefix.chars().count() + 1).collect();
        let after = &input[token.len()..];
        if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
            return None;
        }
        let today_n = i64::from(today.weekday().num_days_from_monday());
        let mut ahead = (7 + index as i64 - today_n) % 7;
        if ahead == 0 {
            ahead = 7;
        }
        return Some((token, today + Duration::days(ahead + week_offset)));
    }
    None
}

fn parse_n_days_later(input: &str, today: NaiveDate) -> Option<(String, NaiveDate)> {
    let bytes = input.as_bytes();
    let mut len = 0;
    while len < bytes.len() && bytes[len].is_ascii_digit() {
        len += 1;
    }
    if len == 0 {
        return None;
    }
    let after = input[len..].strip_prefix("天后")?;
    if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
        return None;
    }
    let days: i64 = input[..len].parse().ok()?;
    if !(1..=365).contains(&days) {
        return None;
    }
    Some((
        input[..len + "天后".len()].to_string(),
        today + Duration::days(days),
    ))
}

#[tauri::command]
pub async fn quick_capture_submit(
    state: State<'_, AppState>,
    app: AppHandle,
    text: String,
) -> AppResult<QuickCaptureResult> {
    let (kind, content) = parse_quick_capture(&text)?;
    match kind {
        "todo" => {
            let list = state.services.todo.ensure_inbox()?;
            let today = Local::now().date_naive();
            let (title, due_date) = match parse_quick_capture_due(content, today) {
                Some((token, date)) => {
                    let rest = content[token.len()..].trim();
                    if rest.is_empty() {
                        // A bare date token is not a useful title: keep the
                        // whole input as a plain Todo without a due date.
                        (content.to_string(), None)
                    } else {
                        (rest.to_string(), Some(date.format("%Y-%m-%d").to_string()))
                    }
                }
                None => (content.to_string(), None),
            };
            let item = state.services.todo.create_item(
                &list.id,
                title,
                None,
                crate::services::todo::TodoSchedule {
                    due_date,
                    remind_at: None,
                    repeat: None,
                    repeat_until: None,
                },
            )?;
            emit_entity_changed(&app, "todoItem", &item.id, "created")?;
            Ok(QuickCaptureResult {
                kind: "todo".into(),
                id: item.id,
                title: item.title.clone(),
                target_list_id: Some(list.id),
            })
        }
        _ => {
            let title = if content.is_empty() {
                "新便签".to_string()
            } else {
                content.to_string()
            };
            let note = state.services.note.create(title, String::new())?;
            let note = match state.services.note.update(
                &note.id,
                crate::services::note::NotePatch {
                    floating: Some(true),
                    ..Default::default()
                },
            ) {
                Ok(note) => note,
                Err(err) => {
                    state.services.note.permanent_delete(&note.id).ok();
                    return Err(err);
                }
            };
            if let Err(err) = crate::app::show_note(&app, &note, true) {
                state.services.note.permanent_delete(&note.id).ok();
                return Err(err);
            }
            if let Err(err) = emit_entity_changed(&app, "note", &note.id, "created") {
                if let Some(window) = app.get_webview_window(&format!("note-{}", note.id)) {
                    window.close().ok();
                }
                state.services.note.permanent_delete(&note.id).ok();
                return Err(err);
            }
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

#[tauri::command]
pub fn quick_capture_show(app: AppHandle) -> AppResult<()> {
    crate::app::show_quick_capture(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn parses_todo_prefix() {
        assert_eq!(
            parse_quick_capture("todo: 修复登录").unwrap(),
            ("todo", "修复登录")
        );
    }

    #[test]
    fn parses_note_command() {
        assert_eq!(parse_quick_capture("  /note  ").unwrap(), ("note", ""));
    }

    #[test]
    fn defaults_to_todo_without_prefix() {
        assert_eq!(
            parse_quick_capture("直接记录").unwrap(),
            ("todo", "直接记录")
        );
    }

    #[test]
    fn commands_and_prefixes_are_case_insensitive_and_trimmed() {
        assert_eq!(parse_quick_capture("  /NOTE  ").unwrap(), ("note", ""));
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
    }

    #[test]
    fn note_command_with_title_creates_note() {
        assert_eq!(
            parse_quick_capture("/note 写点什么").unwrap(),
            ("note", "写点什么")
        );
        assert_eq!(
            parse_quick_capture("  /note   前后空格  ").unwrap(),
            ("note", "前后空格")
        );
    }

    #[test]
    fn text_that_only_starts_like_note_command_is_a_todo() {
        assert_eq!(
            parse_quick_capture("/notebook").unwrap(),
            ("todo", "/notebook")
        );
    }

    #[test]
    fn former_note_prefix_is_plain_todo_text() {
        assert_eq!(
            parse_quick_capture("note: 记录一个想法").unwrap(),
            ("todo", "note: 记录一个想法")
        );
    }

    #[test]
    fn parses_relative_day_tokens() {
        let today = date("2026-08-12");
        assert_eq!(
            parse_quick_capture_due("明天 提交周报", today).unwrap(),
            ("明天".to_string(), date("2026-08-13"))
        );
        assert_eq!(
            parse_quick_capture_due("后天 取快递", today).unwrap(),
            ("后天".to_string(), date("2026-08-14"))
        );
        assert_eq!(
            parse_quick_capture_due("今天 整理", today).unwrap(),
            ("今天".to_string(), date("2026-08-12"))
        );
        assert_eq!(
            parse_quick_capture_due("明日 开会", today).unwrap(),
            ("明日".to_string(), date("2026-08-13"))
        );
    }

    #[test]
    fn parses_weekday_tokens() {
        // 2026-08-12 is a Wednesday (周三).
        let today = date("2026-08-12");
        assert_eq!(
            parse_quick_capture_due("周五 清理 stale PR", today).unwrap(),
            ("周五".to_string(), date("2026-08-14"))
        );
        assert_eq!(
            parse_quick_capture_due("周一 计划", today).unwrap(),
            ("周一".to_string(), date("2026-08-17"))
        );
        assert_eq!(
            parse_quick_capture_due("星期天 休息", today).unwrap(),
            ("星期天".to_string(), date("2026-08-16"))
        );
        // Same weekday as today -> next week (strictly after today).
        assert_eq!(
            parse_quick_capture_due("周三 周会", today).unwrap(),
            ("周三".to_string(), date("2026-08-19"))
        );
    }

    #[test]
    fn parses_days_later_and_month_end() {
        let today = date("2026-08-12");
        assert_eq!(
            parse_quick_capture_due("3天后 提交", today).unwrap(),
            ("3天后".to_string(), date("2026-08-15"))
        );
        assert_eq!(
            parse_quick_capture_due("30天后 复查", today).unwrap(),
            ("30天后".to_string(), date("2026-09-11"))
        );
        assert_eq!(
            parse_quick_capture_due("月底 发布", today).unwrap(),
            ("月底".to_string(), date("2026-08-31"))
        );
        assert_eq!(
            parse_quick_capture_due("月末 对账", today).unwrap(),
            ("月末".to_string(), date("2026-08-31"))
        );
    }

    #[test]
    fn date_tokens_require_following_whitespace_or_end() {
        let today = date("2026-08-12");
        // Embedded prefixes are plain text, not date tokens.
        assert_eq!(parse_quick_capture_due("明天性计划", today), None);
        assert_eq!(parse_quick_capture_due("周五策划案", today), None);
        assert_eq!(parse_quick_capture_due("3天后端联调", today), None);
        assert_eq!(parse_quick_capture_due("月底总结会", today), None);
        // Bare token matches with no remainder.
        assert_eq!(
            parse_quick_capture_due("明天", today).unwrap().1,
            date("2026-08-13")
        );
        // Unsupported patterns degrade to None.
        assert_eq!(parse_quick_capture_due("0天后 无效", today), None);
        assert_eq!(parse_quick_capture_due("400天后 超限", today), None);
        assert_eq!(parse_quick_capture_due("", today), None);
    }

    #[test]
    fn parses_next_weekday_tokens() {
        // 2026-08-12 is a Wednesday (周三).
        let today = date("2026-08-12");
        assert_eq!(
            parse_quick_capture_due("下周五 开会", today).unwrap(),
            ("下周五".to_string(), date("2026-08-21"))
        );
        assert_eq!(
            parse_quick_capture_due("下周三 周报", today).unwrap(),
            ("下周三".to_string(), date("2026-08-26"))
        );
        assert_eq!(
            parse_quick_capture_due("下周日 休息", today).unwrap(),
            ("下周日".to_string(), date("2026-08-23"))
        );
        // `下周` without a valid weekday stays plain text.
        assert_eq!(parse_quick_capture_due("下周计划", today), None);
        assert_eq!(parse_quick_capture_due("下周周会", today), None);
    }
}
