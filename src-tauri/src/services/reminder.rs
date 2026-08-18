use chrono::{DateTime, NaiveDate, Utc};

use crate::models::{parse_due_date, TodoList};

/// One Todo whose `remindAt` has arrived and still needs attention.
#[derive(Debug, Clone, PartialEq)]
pub struct DueReminder {
    pub id: String,
    pub title: String,
    pub list_title: String,
    pub remind_at: String,
    pub due_date: Option<String>,
}

/// Todos whose reminder time has arrived (`remindAt <= now`), not completed
/// and not deleted. Items without a `dueDate` are skipped because `remindAt`
/// is only meaningful together with a due date. Unparseable reminder
/// timestamps are skipped (degrade, never crash).
pub fn due_reminders(lists: &[TodoList], now: &DateTime<Utc>) -> Vec<DueReminder> {
    let mut out = Vec::new();
    for list in lists {
        if list.deleted {
            continue;
        }
        for item in &list.items {
            if item.deleted || item.completed || item.due_date.is_none() {
                continue;
            }
            let Some(remind_at) = &item.remind_at else {
                continue;
            };
            let Ok(ts) = DateTime::parse_from_rfc3339(remind_at) else {
                continue;
            };
            if ts.with_timezone(&Utc) <= *now {
                if item.last_reminded_at.as_deref() == Some(remind_at.as_str()) {
                    continue;
                }
                out.push(DueReminder {
                    id: item.id.clone(),
                    title: item.title.clone(),
                    list_title: list.title.clone(),
                    remind_at: remind_at.clone(),
                    due_date: item.due_date.clone(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.remind_at.cmp(&b.remind_at));
    out
}

/// Count of incomplete, non-deleted Todos whose due date is before `today`
/// (the tray badge number; 0 hides the badge). Unparseable dates never count.
pub fn overdue_count(lists: &[TodoList], today: NaiveDate) -> usize {
    lists
        .iter()
        .flat_map(|list| list.items.iter())
        .filter(|item| !item.deleted && !item.completed)
        .filter(|item| {
            item.due_date
                .as_deref()
                .and_then(parse_due_date)
                .is_some_and(|due| due < today)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TodoItem, TodoList};

    fn list(items: Vec<TodoItem>) -> TodoList {
        TodoList {
            schema_version: 1,
            id: "list-1".into(),
            title: "工作".into(),
            kind: None,
            sort_order: 0,
            deleted: false,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-01T00:00:00Z".into(),
            items,
        }
    }

    fn item(
        id: &str,
        title: &str,
        completed: bool,
        due: Option<&str>,
        remind: Option<&str>,
    ) -> TodoItem {
        TodoItem {
            id: id.into(),
            title: title.into(),
            completed,
            deleted: false,
            sort_order: 0,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-01T00:00:00Z".into(),
            source: None,
            github_sync: None,
            due_date: due.map(str::to_string),
            remind_at: remind.map(str::to_string),
            repeat: None,
            repeat_until: None,
            last_reminded_at: None,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-12T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn collects_only_due_incomplete_items_with_remind_at() {
        let lists = vec![list(vec![
            item(
                "due-1",
                "已到提醒",
                false,
                Some("2026-08-12"),
                Some("2026-08-12T09:00:00Z"),
            ),
            item(
                "future",
                "未来提醒",
                false,
                Some("2026-08-13"),
                Some("2026-08-13T09:00:00Z"),
            ),
            item(
                "completed",
                "已完成",
                true,
                Some("2026-08-12"),
                Some("2026-08-12T09:00:00Z"),
            ),
            item(
                "no-due",
                "无到期日",
                false,
                None,
                Some("2026-08-12T09:00:00Z"),
            ),
            item("no-remind", "未设提醒", false, Some("2026-08-12"), None),
            item(
                "bad-time",
                "坏时间",
                false,
                Some("2026-08-12"),
                Some("not-a-time"),
            ),
        ])];
        let reminders = due_reminders(&lists, &now());
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].id, "due-1");
        assert_eq!(reminders[0].list_title, "工作");
    }

    #[test]
    fn skips_reminders_already_persisted() {
        let mut reminded = item(
            "due-1",
            "已提醒",
            false,
            Some("2026-08-12"),
            Some("2026-08-12T09:00:00Z"),
        );
        reminded.last_reminded_at = Some("2026-08-12T09:00:00Z".into());
        let lists = vec![list(vec![reminded])];
        assert!(due_reminders(&lists, &now()).is_empty());
    }

    #[test]
    fn overdue_count_ignores_completed_deleted_and_bad_dates() {
        let done = item("c1", "完成", true, Some("2026-08-01"), None);
        let mut deleted = item("d1", "删除", false, Some("2026-08-01"), None);
        deleted.deleted = true;
        let lists = vec![list(vec![
            done,
            deleted,
            item("o1", "逾期", false, Some("2026-08-11"), None),
            item("today", "今天", false, Some("2026-08-12"), None),
            item("soon", "未来", false, Some("2026-08-13"), None),
            item("bad", "坏日期", false, Some("oops"), None),
            item("none", "无日期", false, None, None),
        ])];
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        assert_eq!(overdue_count(&lists, today), 1);
    }

    #[test]
    fn due_reminders_sort_by_remind_time() {
        let lists = vec![list(vec![
            item(
                "later",
                "较晚",
                false,
                Some("2026-08-12"),
                Some("2026-08-12T09:30:00Z"),
            ),
            item(
                "earlier",
                "较早",
                false,
                Some("2026-08-12"),
                Some("2026-08-12T08:00:00Z"),
            ),
        ])];
        let reminders = due_reminders(&lists, &now());
        let ids: Vec<&str> = reminders.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, vec!["earlier", "later"]);
    }
}
