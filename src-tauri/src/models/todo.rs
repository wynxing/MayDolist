use chrono::{DateTime, Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Repeat rule of a recurring Todo item. Serialized lowercase (`daily` /
/// `weekly` / `biweekly` / `monthly`) so the frontend can consume it directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatRule {
    Daily,
    Weekly,
    Biweekly,
    Monthly,
}

/// Parse an ISO date (`YYYY-MM-DD`) or RFC3339 datetime into a calendar date
/// in the local timezone. Datetimes are converted to local so the "today"
/// boundary matches what the user sees. Anything unparseable returns `None`
/// (readers degrade to "no date" instead of crashing).
pub fn parse_due_date(value: &str) -> Option<NaiveDate> {
    let value = value.trim();
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Some(date);
    }
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Local).date_naive())
}

/// Validate a due-date-ish field (`dueDate` / `remindAt` / `repeatUntil`)
/// before it is persisted. Invalid values are rejected at the service layer
/// so junk never reaches disk; reads still degrade gracefully.
pub fn validate_due_date(value: &str) -> AppResult<()> {
    if parse_due_date(value).is_none() {
        return Err(AppError::InvalidInput(format!(
            "invalid date value: {value}"
        )));
    }
    Ok(())
}

/// Number of days in `month` of `year` (handles leap years).
pub fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("first day of next month always exists")
        .pred_opt()
        .expect("month has a last day")
        .day()
}

/// Compute the due date of the next repeat instance, based on the rule, the
/// completed item's anchor date and today. The result is always strictly
/// after `today` (late completions jump to the next upcoming occurrence).
/// Monthly rules keep the anchor day-of-month and clamp to the month end
/// (Jan 31 → Feb 28 → Mar 31), so month-end boundaries stay correct.
/// Returns `None` when the next occurrence is after `repeat_until`.
pub fn next_repeat_due(
    rule: RepeatRule,
    anchor: NaiveDate,
    today: NaiveDate,
    repeat_until: Option<NaiveDate>,
) -> Option<NaiveDate> {
    let due = match rule {
        RepeatRule::Daily => next_by_step(anchor, today, 1),
        RepeatRule::Weekly => next_by_step(anchor, today, 7),
        RepeatRule::Biweekly => next_by_step(anchor, today, 14),
        RepeatRule::Monthly => next_monthly(anchor, today),
    };
    if repeat_until.is_some_and(|until| due > until) {
        None
    } else {
        Some(due)
    }
}

fn next_by_step(anchor: NaiveDate, today: NaiveDate, step: i64) -> NaiveDate {
    let mut due = anchor + chrono::Duration::days(step);
    while due <= today {
        due += chrono::Duration::days(step);
    }
    due
}

fn next_monthly(anchor: NaiveDate, today: NaiveDate) -> NaiveDate {
    let day = anchor.day();
    let (mut year, mut month) = (anchor.year(), anchor.month());
    loop {
        (year, month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let last = last_day_of_month(year, month);
        let candidate =
            NaiveDate::from_ymd_opt(year, month, day.min(last)).expect("clamped date is valid");
        if candidate > today {
            return candidate;
        }
    }
}

/// Optional source reference linking a Todo item back to an external item
/// (MVP: GitHub issues and pull requests). Old Todo data without the field
/// reads as `None` and behaves like a normal Todo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoSource {
    /// `"github-issue"` or `"github-pr"`, serialized as `type` per the #19
    /// data contract so the frontend reads `source.type`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `owner/repo` of the source repository.
    pub repo: String,
    /// GitHub issue / PR number.
    pub number: u64,
    /// Canonical URL of the source item. Only http / https are allowed.
    pub url: String,
}

impl TodoSource {
    /// Validate a source reference before it is persisted. Only the MVP
    /// GitHub kinds are accepted, and the URL must be http / https so the
    /// "open source" action can never launch a non-browser scheme.
    pub fn validate(&self) -> AppResult<()> {
        if self.kind != "github-issue" && self.kind != "github-pr" {
            return Err(AppError::InvalidInput(format!(
                "unsupported source type: {}",
                self.kind
            )));
        }
        if self.repo.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "source repo must not be empty".into(),
            ));
        }
        if self.number == 0 {
            return Err(AppError::InvalidInput(
                "source number must be positive".into(),
            ));
        }
        let parsed = url::Url::parse(&self.url)
            .map_err(|_| AppError::InvalidInput("invalid source url".into()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::InvalidInput(
                "source url must be http or https".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub completed: bool,
    /// Soft-delete flag: the row is kept on disk but filtered out of lists.
    pub deleted: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    /// Optional external source (e.g. GitHub PR / issue). Old data without
    /// the field reads as `None`; items without a source keep the old JSON
    /// shape (field is skipped when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<TodoSource>,
    /// Optional due date, stored as ISO date (`YYYY-MM-DD`) or RFC3339
    /// datetime. Used by the Focus grouping and repeat generation. Old data
    /// without the field reads as `None` and keeps the old JSON shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// Optional reminder time (RFC3339). Only meaningful when `due_date` is
    /// set; the scheduler degrades silently to a tray badge when the system
    /// blocks notifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remind_at: Option<String>,
    /// Optional repeat rule. Completing the item generates the next instance
    /// in the Rust service layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<RepeatRule>,
    /// Optional repeat end date (`YYYY-MM-DD` or RFC3339); no instance is
    /// generated after this date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_until: Option<String>,
    /// Last reminder timestamp that was delivered (or suppressed during quiet
    /// hours). Equal to `remind_at` when that reminder has already been
    /// handled, so a process restart does not toast again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reminded_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoList {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    /// Optional stable marker for system-managed lists (e.g. "inbox"). Old
    /// data without the field reads as None and behaves like a normal list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub sort_order: i32,
    pub deleted: bool,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<TodoItem>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn parse_due_date_accepts_date_and_datetime() {
        assert_eq!(parse_due_date("2026-08-12"), Some(date("2026-08-12")));
        assert!(parse_due_date("2026-08-12T14:30:00+08:00").is_some());
        assert!(parse_due_date("2026-08-12T14:30:00Z").is_some());
        assert_eq!(parse_due_date("not a date"), None);
        assert_eq!(parse_due_date("2026-13-01"), None);
        assert_eq!(parse_due_date(""), None);
    }

    #[test]
    fn validate_due_date_rejects_bad_values() {
        assert!(validate_due_date("2026-08-12").is_ok());
        assert!(validate_due_date("2026-08-12T09:00:00+08:00").is_ok());
        assert!(validate_due_date("明天").is_err());
        assert!(validate_due_date("2026-02-30").is_err());
    }

    #[test]
    fn legacy_todo_reads_new_fields_as_none_and_skips_them_on_write() {
        let json = r#"{
            "id": "item-1",
            "title": "旧待办",
            "completed": false,
            "deleted": false,
            "sortOrder": 0,
            "createdAt": "2026-08-01T00:00:00Z",
            "updatedAt": "2026-08-01T00:00:00Z"
        }"#;
        let item: TodoItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.due_date, None);
        assert_eq!(item.remind_at, None);
        assert_eq!(item.repeat, None);
        assert_eq!(item.repeat_until, None);
        assert_eq!(item.last_reminded_at, None);
        let raw = serde_json::to_string(&item).unwrap();
        assert!(!raw.contains("dueDate"));
        assert!(!raw.contains("remindAt"));
        assert!(!raw.contains("repeat"));
        assert!(!raw.contains("lastRemindedAt"));
    }

    #[test]
    fn new_fields_roundtrip_through_json() {
        let item = TodoItem {
            id: "item-1".into(),
            title: "新待办".into(),
            completed: false,
            deleted: false,
            sort_order: 0,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-01T00:00:00Z".into(),
            source: None,
            due_date: Some("2026-08-20".into()),
            remind_at: Some("2026-08-20T09:00:00+08:00".into()),
            repeat: Some(RepeatRule::Weekly),
            repeat_until: Some("2026-12-31".into()),
            last_reminded_at: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"dueDate\":\"2026-08-20\""));
        assert!(json.contains("\"repeat\":\"weekly\""));
        let restored: TodoItem = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, item);
    }

    #[test]
    fn daily_repeat_generates_tomorrow_from_today() {
        let today = date("2026-08-12");
        assert_eq!(
            next_repeat_due(RepeatRule::Daily, today, today, None),
            Some(date("2026-08-13"))
        );
    }

    #[test]
    fn late_completion_jumps_to_next_upcoming_occurrence() {
        let anchor = date("2026-08-10"); // 周一
        let today = date("2026-08-15"); // 周六, completed late
                                        // Weekly: anchor + 7 = 08-17, already after today -> 08-17.
        assert_eq!(
            next_repeat_due(RepeatRule::Weekly, anchor, today, None),
            Some(date("2026-08-17"))
        );
        // Daily: anchor + 1 = 08-11 <= today -> step to 08-16.
        assert_eq!(
            next_repeat_due(RepeatRule::Daily, anchor, today, None),
            Some(date("2026-08-16"))
        );
    }

    #[test]
    fn early_completion_skips_the_original_occurrence() {
        let anchor = date("2026-08-13"); // due tomorrow
        let today = date("2026-08-12"); // completed one day early
        assert_eq!(
            next_repeat_due(RepeatRule::Daily, anchor, today, None),
            Some(date("2026-08-14"))
        );
    }

    #[test]
    fn biweekly_repeat_keeps_week_alignment() {
        let anchor = date("2026-08-14"); // Friday
        let today = date("2026-08-15");
        assert_eq!(
            next_repeat_due(RepeatRule::Biweekly, anchor, today, None),
            Some(date("2026-08-28"))
        );
    }

    #[test]
    fn monthly_repeat_handles_month_end_boundaries() {
        let anchor = date("2026-01-31");
        let today = date("2026-01-15");
        // Jan 31 -> Feb 28 (clamped, 2026 not a leap year).
        assert_eq!(
            next_repeat_due(RepeatRule::Monthly, anchor, today, None),
            Some(date("2026-02-28"))
        );
        // After Feb 28 passes, the anchor day (31) is restored in March.
        let today = date("2026-03-01");
        assert_eq!(
            next_repeat_due(RepeatRule::Monthly, anchor, today, None),
            Some(date("2026-03-31"))
        );
    }

    #[test]
    fn repeat_until_stops_generation() {
        let today = date("2026-08-12");
        let until = date("2026-08-13");
        assert_eq!(
            next_repeat_due(RepeatRule::Daily, today, today, Some(until)),
            Some(date("2026-08-13"))
        );
        assert_eq!(
            next_repeat_due(RepeatRule::Daily, today, date("2026-08-13"), Some(until)),
            None
        );
    }

    #[test]
    fn last_day_of_month_handles_leap_years() {
        assert_eq!(last_day_of_month(2026, 2), 28);
        assert_eq!(last_day_of_month(2028, 2), 29);
        assert_eq!(last_day_of_month(2026, 12), 31);
        assert_eq!(last_day_of_month(2026, 4), 30);
    }
}
