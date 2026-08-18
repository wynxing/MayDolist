use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::events::now_rfc3339;
use crate::models::{
    parse_due_date, FocusGithub, FocusNote, FocusOverview, FocusSection, FocusSectionState,
    FocusTodo, FocusTodoGroup, FocusTodoSection, GhAuthStatus, Note, RepoSnapshot, RepoWatch,
    TodoList,
};
use crate::services::github::GithubService;
use crate::services::note::NoteService;
use crate::services::todo::{TodoService, INBOX_KIND};

/// Display caps keep the Focus view bounded even with large data sets.
pub const MAX_TODOS: usize = 50;
pub const MAX_NOTES: usize = 8;
pub const MAX_GITHUB: usize = 30;
/// Non-pinned "recently updated" notes shown after the pinned ones.
pub const RECENT_NON_PINNED_NOTES: usize = 5;
/// Truncation length for the note preview.
pub const NOTE_PREVIEW_CHARS: usize = 80;

/// Read-only projection across the todo / note / github domains. Loading is
/// parallel and per-domain isolated: a failure in one domain becomes a local
/// section error and never blocks the other sections.
pub struct FocusService {
    todo: Arc<TodoService>,
    note: Arc<NoteService>,
    github: Arc<GithubService>,
}

impl FocusService {
    pub fn new(todo: Arc<TodoService>, note: Arc<NoteService>, github: Arc<GithubService>) -> Self {
        Self { todo, note, github }
    }

    pub fn overview(&self) -> FocusOverview {
        let generated_at = now_rfc3339();
        std::thread::scope(|scope| {
            let todo = scope.spawn(|| self.load_todo_section());
            let note = scope.spawn(|| self.load_note());
            let github = scope.spawn(|| self.load_github());
            FocusOverview {
                generated_at,
                todo: todo
                    .join()
                    .unwrap_or_else(|_| FocusTodoSection::error("todo loader panicked".into())),
                note: note
                    .join()
                    .unwrap_or_else(|_| FocusSection::error("note loader panicked".into())),
                github: github
                    .join()
                    .unwrap_or_else(|_| FocusSection::error("github loader panicked".into())),
            }
        })
    }

    fn load_todo_section(&self) -> FocusTodoSection {
        match self.todo.list(false) {
            Ok(lists) => {
                let items = project_todos(&lists);
                let total = items.len();
                let groups = group_todos(items, chrono::Local::now().date_naive());
                FocusTodoSection {
                    state: FocusSectionState::Ready,
                    error: None,
                    total,
                    groups,
                }
            }
            Err(err) => FocusTodoSection {
                state: FocusSectionState::Error,
                error: Some(err.to_string()),
                total: 0,
                groups: Vec::new(),
            },
        }
    }

    fn load_note(&self) -> FocusSection<FocusNote> {
        match self.note.list(false) {
            Ok(notes) => {
                let items = project_notes(&notes);
                let total = items.len();
                FocusSection {
                    state: FocusSectionState::Ready,
                    error: None,
                    total,
                    offline_cache: false,
                    items: items.into_iter().take(MAX_NOTES).collect(),
                }
            }
            Err(err) => FocusSection::error(err.to_string()),
        }
    }

    fn load_github(&self) -> FocusSection<FocusGithub> {
        let status = self.github.status();
        let watchlist = match self.github.watchlist() {
            Ok(watchlist) => watchlist,
            Err(err) => return FocusSection::error(err.to_string()),
        };
        let mut snapshots = Vec::new();
        let mut errors = Vec::new();
        for watch in &watchlist {
            match self.github.snapshot(&watch.full_name) {
                Ok(Some(snapshot)) => snapshots.push(snapshot),
                Ok(None) => {}
                Err(err) => errors.push(format!("{}: {err}", watch.full_name)),
            }
        }
        let (items, offline_cache) = project_github(&watchlist, &snapshots, &status);
        let total = items.len();
        FocusSection {
            state: if errors.is_empty() {
                FocusSectionState::Ready
            } else {
                FocusSectionState::Error
            },
            error: (!errors.is_empty()).then(|| errors.join("; ")),
            total,
            offline_cache,
            items: items.into_iter().take(MAX_GITHUB).collect(),
        }
    }
}

impl<T> FocusSection<T> {
    fn error(message: String) -> Self {
        Self {
            state: FocusSectionState::Error,
            error: Some(message),
            total: 0,
            offline_cache: false,
            items: Vec::new(),
        }
    }
}

impl FocusTodoSection {
    fn error(message: String) -> Self {
        Self {
            state: FocusSectionState::Error,
            error: Some(message),
            total: 0,
            groups: Vec::new(),
        }
    }
}

/// Incomplete, non-deleted Todo items. Inbox lists come first; within each
/// group the existing list/item order (sort order) is preserved, so the input
/// must already be sorted like `TodoService::list`.
pub fn project_todos(lists: &[TodoList]) -> Vec<FocusTodo> {
    let mut items = Vec::new();
    for list in lists {
        if list.deleted {
            continue;
        }
        let inbox = list.kind.as_deref() == Some(INBOX_KIND);
        for item in &list.items {
            if item.deleted || item.completed {
                continue;
            }
            items.push(FocusTodo {
                id: item.id.clone(),
                title: item.title.clone(),
                list_id: list.id.clone(),
                list_title: list.title.clone(),
                inbox,
                updated_at: item.updated_at.clone(),
                source: item.source.clone(),
                github_sync: item.github_sync.clone(),
                due_date: item.due_date.clone(),
                remind_at: item.remind_at.clone(),
                repeat: item.repeat,
            });
        }
    }
    // Stable sort keeps list/item sort order inside each inbox group.
    items.sort_by_key(|item| !item.inbox);
    items
}

/// Group incomplete todos by due state and sort within each group:
/// 已逾期 (due < today, oldest first) → 今天到期 (by due time) →
/// 近期 7 天 (due <= today + 7, by due date) → 无日期 (inbox first).
/// Unparseable due dates degrade to the "无日期" group. The display cap is
/// applied across groups in priority order; only non-empty groups are
/// returned.
pub fn group_todos(items: Vec<FocusTodo>, today: NaiveDate) -> Vec<FocusTodoGroup> {
    let mut overdue = Vec::new();
    let mut today_group = Vec::new();
    let mut soon = Vec::new();
    let mut none = Vec::new();
    for item in items {
        match item.due_date.as_deref().and_then(parse_due_date) {
            Some(due) if due < today => overdue.push(item),
            Some(due) if due == today => today_group.push(item),
            Some(due) if due <= today + chrono::Duration::days(7) => soon.push(item),
            _ => none.push(item),
        }
    }
    let due_of = |item: &FocusTodo| item.due_date.as_deref().and_then(parse_due_date);
    overdue.sort_by_key(|item| due_of(item));
    today_group.sort_by_key(|item| due_of(item));
    soon.sort_by_key(|item| due_of(item));
    // Stable: keeps the original inbox-first / sort-order layout.
    none.sort_by_key(|item| !item.inbox);

    let mut groups = Vec::new();
    let mut remaining = MAX_TODOS;
    for (key, title, mut items) in [
        ("overdue", "已逾期", overdue),
        ("today", "今天到期", today_group),
        ("soon", "近期 7 天", soon),
        ("none", "无日期", none),
    ] {
        if items.is_empty() {
            continue;
        }
        let count = items.len();
        let take = items.len().min(remaining);
        items.truncate(take);
        remaining -= take;
        groups.push(FocusTodoGroup {
            key: key.into(),
            title: title.into(),
            count,
            items,
        });
        if remaining == 0 {
            break;
        }
    }
    groups
}

/// Pinned notes first (by `updated_at` descending), then the most recently
/// updated non-pinned notes, deduplicated by id.
pub fn project_notes(notes: &[Note]) -> Vec<FocusNote> {
    let mut pinned = Vec::new();
    let mut recent = Vec::new();
    for note in notes {
        if note.deleted {
            continue;
        }
        let item = FocusNote {
            id: note.id.clone(),
            title: note.title.clone(),
            pinned: note.pinned,
            floating: note.floating,
            updated_at: note.updated_at.clone(),
            preview: preview_of(&note.content),
        };
        if note.pinned {
            pinned.push(item);
        } else {
            recent.push(item);
        }
    }
    pinned.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    recent.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    recent.truncate(RECENT_NON_PINNED_NOTES);
    merge_note_sections(pinned, recent, MAX_NOTES)
}

/// Merges pinned and recent note lists, deduplicating by id and applying the
/// display cap. Public for focused unit tests.
pub fn merge_note_sections(
    mut pinned: Vec<FocusNote>,
    mut recent: Vec<FocusNote>,
    cap: usize,
) -> Vec<FocusNote> {
    let mut seen = HashSet::new();
    pinned.retain(|item| seen.insert(item.id.clone()));
    recent.retain(|item| seen.insert(item.id.clone()));
    pinned.extend(recent);
    pinned.truncate(cap);
    pinned
}

/// Open GitHub items from local snapshot caches. Focus is an action inbox, so
/// only items carrying at least one actionable signal
/// (`needsAction` / `needsReview` / `ciFailed` / `stale`) are aggregated;
/// draft-only or unrelated open items stay in the GitHub view. Pinned items
/// come first, then by `updated_at` descending. Returns the items and whether
/// the section relies on an offline / stale cache.
pub fn project_github(
    _watchlist: &[RepoWatch],
    snapshots: &[RepoSnapshot],
    status: &GhAuthStatus,
) -> (Vec<FocusGithub>, bool) {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for snapshot in snapshots {
        for pr in &snapshot.pull_requests {
            if pr.state != "open" || !pr.signals.iter().any(|s| s.is_actionable()) {
                continue;
            }
            let key = (snapshot.repo.clone(), pr.number, "pr".to_string());
            if !seen.insert(key) {
                continue;
            }
            let pinned = pr.matches.iter().any(|m| m == "pinned");
            items.push(FocusGithub {
                kind: "pr".into(),
                repo: snapshot.repo.clone(),
                number: pr.number,
                title: pr.title.clone(),
                state: pr.state.clone(),
                draft: pr.draft,
                url: pr.url.clone(),
                updated_at: pr.updated_at.clone(),
                pinned,
                matches: pr.matches.clone(),
                signals: pr.signals.clone(),
            });
        }
        for issue in &snapshot.issues {
            if issue.state != "open" || !issue.signals.iter().any(|s| s.is_actionable()) {
                continue;
            }
            let key = (snapshot.repo.clone(), issue.number, "issue".to_string());
            if !seen.insert(key) {
                continue;
            }
            let pinned = issue.matches.iter().any(|m| m == "pinned");
            items.push(FocusGithub {
                kind: "issue".into(),
                repo: snapshot.repo.clone(),
                number: issue.number,
                title: issue.title.clone(),
                state: issue.state.clone(),
                draft: false,
                url: issue.url.clone(),
                updated_at: issue.updated_at.clone(),
                pinned,
                matches: issue.matches.clone(),
                signals: issue.signals.clone(),
            });
        }
    }
    items.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    let offline_cache = !status.logged_in || snapshots.iter().any(|s| s.last_error.is_some());
    (items, offline_cache)
}

fn preview_of(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or_default().trim();
    if first_line.chars().count() <= NOTE_PREVIEW_CHARS {
        first_line.to_string()
    } else {
        let truncated: String = first_line.chars().take(NOTE_PREVIEW_CHARS).collect();
        format!("{truncated}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{GhIssue, GhPullRequest, Note, TodoItem, TodoList, WindowBounds};

    fn todo_list(id: &str, title: &str, kind: Option<&str>, sort_order: i32) -> TodoList {
        TodoList {
            schema_version: 1,
            id: id.into(),
            title: title.into(),
            kind: kind.map(str::to_string),
            sort_order,
            deleted: false,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-01T00:00:00Z".into(),
            items: vec![],
        }
    }

    fn todo_item(id: &str, title: &str, completed: bool, order: i32) -> TodoItem {
        TodoItem {
            id: id.into(),
            title: title.into(),
            completed,
            deleted: false,
            sort_order: order,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: format!("2026-08-01T00:00:0{order}Z"),
            source: None,
            github_sync: None,
            due_date: None,
            remind_at: None,
            repeat: None,
            repeat_until: None,
            last_reminded_at: None,
        }
    }

    fn note(id: &str, title: &str, pinned: bool, updated_at: &str) -> Note {
        Note {
            schema_version: 1,
            id: id.into(),
            title: title.into(),
            content: "内容内容内容".into(),
            tags: vec![],
            color: "blue".into(),
            pinned,
            floating: false,
            collapsed: false,
            always_on_top: true,
            window_bounds: None::<WindowBounds>,
            deleted: false,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: updated_at.into(),
        }
    }

    fn snapshot(repo: &str) -> RepoSnapshot {
        RepoSnapshot {
            schema_version: 1,
            repo: repo.into(),
            fetched_at: "2026-08-01T00:00:00Z".into(),
            last_success_at: Some("2026-08-01T00:00:00Z".into()),
            last_error: None,
            issues: vec![],
            pull_requests: vec![],
            signals_computed_at: None,
        }
    }

    fn test_signals(matches: &[&str]) -> Vec<crate::models::ActionSignal> {
        let mut signals = Vec::new();
        if matches
            .iter()
            .any(|m| ["pinned", "mine", "mentioned", "assigned", "involved"].contains(m))
        {
            signals.push(crate::models::ActionSignal::NeedsAction);
        }
        if matches.contains(&"review") {
            signals.push(crate::models::ActionSignal::NeedsReview);
        }
        if matches.contains(&"ci") {
            signals.push(crate::models::ActionSignal::CiFailed);
        }
        if matches.contains(&"stale") {
            signals.push(crate::models::ActionSignal::Stale);
        }
        if matches.contains(&"draft") {
            signals.push(crate::models::ActionSignal::Draft);
        }
        signals
    }

    fn pr(number: u64, state: &str, updated_at: &str, matches: &[&str]) -> GhPullRequest {
        GhPullRequest {
            number,
            title: format!("PR #{number}"),
            state: state.into(),
            draft: false,
            url: format!("https://example.test/pr/{number}"),
            updated_at: updated_at.into(),
            matches: matches.iter().map(|v| v.to_string()).collect(),
            assignees: vec![],
            reviewers: vec![],
            head_sha: None,
            checks_state: None,
            signals: test_signals(matches),
        }
    }

    fn issue(number: u64, state: &str, updated_at: &str, matches: &[&str]) -> GhIssue {
        GhIssue {
            number,
            title: format!("Issue #{number}"),
            state: state.into(),
            url: format!("https://example.test/issues/{number}"),
            updated_at: updated_at.into(),
            kind: "issue".into(),
            matches: matches.iter().map(|v| v.to_string()).collect(),
            assignees: vec![],
            signals: test_signals(matches),
        }
    }

    #[test]
    fn todos_filter_completed_and_put_inbox_first() {
        let mut inbox = todo_list("inbox", "收件箱", Some(INBOX_KIND), 3);
        inbox.items = vec![
            todo_item("i1", "收件箱任务", false, 0),
            todo_item("i2", "已完成任务", true, 1),
            todo_item("i3", "收件箱任务2", false, 2),
        ];
        let mut work = todo_list("work", "工作", None, 0);
        work.items = vec![todo_item("w1", "工作任务", false, 0)];
        let mut deleted = todo_list("del", "已删列表", None, 2);
        deleted.deleted = true;
        deleted.items = vec![todo_item("d1", "隐藏", false, 0)];

        let items = project_todos(&[work.clone(), inbox, deleted]);
        let titles: Vec<&str> = items.iter().map(|v| v.title.as_str()).collect();
        assert_eq!(titles, vec!["收件箱任务", "收件箱任务2", "工作任务"]);
        assert!(items[0].inbox && items[1].inbox && !items[2].inbox);
        assert_eq!(items[0].list_id, "inbox");
        // Completed and deleted items never enter the projection.
        assert!(!items.iter().any(|v| v.id == "i2" || v.id == "d1"));
    }

    #[test]
    fn project_todos_carries_github_source() {
        use crate::models::TodoSource;

        let mut inbox = todo_list("inbox", "收件箱", Some(INBOX_KIND), 0);
        let mut item = todo_item("s1", "带来源待办", false, 0);
        item.source = Some(TodoSource {
            kind: "github-pr".into(),
            repo: "owner/repo".into(),
            number: 12,
            url: "https://github.com/owner/repo/pull/12".into(),
        });
        inbox.items = vec![item];
        let items = project_todos(&[inbox]);
        assert_eq!(items.len(), 1);
        let source = items[0].source.clone().expect("source must be projected");
        assert_eq!(source.kind, "github-pr");
        assert_eq!(source.repo, "owner/repo");
        assert_eq!(source.number, 12);
        assert_eq!(items[0].source.as_ref().unwrap().url, source.url);
    }

    #[test]
    fn group_todos_orders_overdue_today_soon_none() {
        let mut inbox = todo_list("inbox", "收件箱", Some(INBOX_KIND), 0);
        inbox.items = vec![
            todo_item("none-1", "无日期", false, 0),
            todo_item("overdue-1", "逾期较早", false, 1),
            todo_item("today-1", "今天到期", false, 2),
            todo_item("soon-1", "三天后", false, 3),
            todo_item("soon-2", "七天后", false, 4),
            todo_item("overdue-2", "逾期较晚", false, 5),
        ];
        for item in &mut inbox.items {
            item.due_date = match item.title.as_str() {
                "逾期较早" => Some("2026-08-10".into()),
                "逾期较晚" => Some("2026-08-11".into()),
                "今天到期" => Some("2026-08-12".into()),
                "三天后" => Some("2026-08-15".into()),
                "七天后" => Some("2026-08-19".into()),
                _ => None,
            };
        }
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let groups = group_todos(project_todos(&[inbox]), today);
        let keys: Vec<&str> = groups.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(keys, vec!["overdue", "today", "soon", "none"]);
        assert_eq!(groups[0].title, "已逾期");
        assert_eq!(groups[0].count, 2);
        // Overdue sorted oldest first; today and soon by due date ascending.
        let overdue_ids: Vec<&str> = groups[0].items.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(overdue_ids, vec!["overdue-1", "overdue-2"]);
        assert_eq!(groups[1].items[0].id, "today-1");
        let soon_ids: Vec<&str> = groups[2].items.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(soon_ids, vec!["soon-1", "soon-2"]);
        assert_eq!(groups[3].items[0].id, "none-1");
    }

    #[test]
    fn group_todos_degrades_unparseable_dates_to_none_and_caps() {
        let mut inbox = todo_list("inbox", "收件箱", Some(INBOX_KIND), 0);
        inbox.items = vec![
            todo_item("bad-1", "坏日期", false, 0),
            todo_item("over-1", "逾期", false, 1),
        ];
        inbox.items[0].due_date = Some("not-a-date".into());
        inbox.items[1].due_date = Some("2026-08-01".into());
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let groups = group_todos(project_todos(&[inbox]), today);
        assert_eq!(groups[0].key, "overdue");
        assert_eq!(groups[1].key, "none");
        assert_eq!(groups[1].items[0].id, "bad-1");

        // Cap applies across groups in priority order.
        let mut capped = todo_list("capped", "收件箱", Some(INBOX_KIND), 0);
        for i in 0..=MAX_TODOS {
            let mut item = todo_item(&format!("o{i}"), &format!("逾期{i}"), false, i as i32);
            item.due_date = Some("2026-08-01".into());
            capped.items.push(item);
        }
        let groups = group_todos(project_todos(&[capped]), today);
        assert_eq!(groups.len(), 1, "cap is consumed by the first group");
        assert_eq!(groups[0].count, MAX_TODOS + 1);
        assert_eq!(groups[0].items.len(), MAX_TODOS);
    }

    #[test]
    fn group_todos_carries_due_and_repeat_fields() {
        let mut inbox = todo_list("inbox", "收件箱", Some(INBOX_KIND), 0);
        let mut item = todo_item("r1", "每周任务", false, 0);
        item.due_date = Some("2026-08-12".into());
        item.remind_at = Some("2026-08-12T09:00:00+08:00".into());
        item.repeat = Some(crate::models::RepeatRule::Weekly);
        inbox.items = vec![item];
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let groups = group_todos(project_todos(&[inbox]), today);
        let shown = &groups[0].items[0];
        assert_eq!(shown.due_date.as_deref(), Some("2026-08-12"));
        assert_eq!(shown.repeat, Some(crate::models::RepeatRule::Weekly));
        assert!(shown.remind_at.is_some());
    }

    #[test]
    fn notes_pinned_first_then_recent_with_dedup_and_cap() {
        let notes = vec![
            note("n1", "旧置顶", true, "2026-08-01T00:00:00Z"),
            note("n2", "新便签", false, "2026-08-02T00:00:00Z"),
            note("n3", "更旧", false, "2026-07-01T00:00:00Z"),
        ];
        let items = project_notes(&notes);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id, "n1");
        assert_eq!(items[1].id, "n2");
        assert_eq!(items[2].id, "n3");

        // Dedup + cap on the merge step itself.
        let pinned = vec![
            FocusNote {
                id: "x".into(),
                title: "重复".into(),
                pinned: true,
                floating: false,
                updated_at: "2026-08-01T00:00:00Z".into(),
                preview: String::new(),
            },
            FocusNote {
                id: "keep".into(),
                title: "保留".into(),
                pinned: true,
                floating: false,
                updated_at: "2026-08-01T00:00:00Z".into(),
                preview: String::new(),
            },
        ];
        let recent = vec![FocusNote {
            id: "x".into(),
            title: "重复".into(),
            pinned: false,
            floating: false,
            updated_at: "2026-08-01T00:00:00Z".into(),
            preview: String::new(),
        }];
        let merged = merge_note_sections(pinned, recent, 1);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "x");
    }

    #[test]
    fn github_open_only_dedup_pinned_first() {
        let mut snap = snapshot("owner/repo");
        snap.pull_requests = vec![
            pr(103, "open", "2026-08-02T00:00:00Z", &["pinned", "mine"]),
            // Duplicate key (repo + number + kind) must be deduplicated.
            pr(103, "open", "2026-08-02T00:00:00Z", &["pinned", "mine"]),
            pr(101, "open", "2026-08-01T00:00:00Z", &["mine"]),
            pr(102, "closed", "2026-08-03T00:00:00Z", &["pinned"]),
        ];
        snap.issues = vec![issue(201, "open", "2026-08-01T00:00:00Z", &["mentioned"])];
        let status = GhAuthStatus {
            state: "authenticated".into(),
            logged_in: true,
            user: Some("wynxing".into()),
            version: Some("gh 2.97.0".into()),
            message: String::new(),
        };
        let (items, offline) = project_github(&[], &[snap], &status);
        assert!(!offline);
        // Closed PR #102 excluded; open PR #103 appears once (pinned first).
        let numbers: Vec<u64> = items.iter().map(|v| v.number).collect();
        assert_eq!(numbers, vec![103, 101, 201]);
        assert!(items.iter().all(|v| v.state == "open"));
        assert_eq!(items[0].kind, "pr");
        assert!(items[0].pinned);
        assert_eq!(items[0].repo, "owner/repo");
        assert_eq!(items[0].url, "https://example.test/pr/103");
    }

    #[test]
    fn github_marks_offline_cache() {
        let mut snap = snapshot("owner/repo");
        snap.last_error = Some("network down".into());
        snap.pull_requests = vec![pr(1, "open", "2026-08-01T00:00:00Z", &["mine"])];
        let offline_status = GhAuthStatus {
            state: "offline".into(),
            logged_in: false,
            user: None,
            version: None,
            message: String::new(),
        };
        let (items, offline) = project_github(&[], &[snap], &offline_status);
        assert!(offline);
        assert_eq!(items.len(), 1, "cached items still shown while offline");
    }

    #[test]
    fn github_aggregates_only_actionable_signals() {
        let mut snap = snapshot("owner/repo");
        snap.pull_requests = vec![
            // Review request → actionable.
            pr(101, "open", "2026-08-02T00:00:00Z", &["all-prs", "review"]),
            // Failed CI → actionable.
            pr(102, "open", "2026-08-02T00:00:00Z", &["all-prs", "ci"]),
            // Stale only → actionable.
            pr(103, "open", "2026-06-01T00:00:00Z", &["all-prs", "stale"]),
            // Draft only → informational, stays in GitHub view only.
            pr(104, "open", "2026-08-02T00:00:00Z", &["all-prs", "draft"]),
            // No signal at all → excluded.
            pr(105, "open", "2026-08-02T00:00:00Z", &["all-prs"]),
            // Closed items never enter Focus.
            pr(106, "closed", "2026-08-02T00:00:00Z", &["pinned"]),
        ];
        snap.issues = vec![issue(201, "open", "2026-08-01T00:00:00Z", &["mentioned"])];
        let status = GhAuthStatus {
            state: "authenticated".into(),
            logged_in: true,
            user: Some("wynxing".into()),
            version: Some("gh 2.97.0".into()),
            message: String::new(),
        };
        let (items, offline) = project_github(&[], &[snap], &status);
        assert!(!offline);
        let numbers: Vec<u64> = items.iter().map(|v| v.number).collect();
        // Pinned first, then by `updated_at` descending: #101/#102 (08-02),
        // issue #201 (08-01), stale #103 (oldest) last.
        assert_eq!(numbers, vec![101, 102, 201, 103]);
        assert!(items
            .iter()
            .all(|v| v.signals.iter().any(|s| s.is_actionable())));
        assert!(items.iter().any(|v| v
            .signals
            .contains(&crate::models::ActionSignal::NeedsReview)));
        assert!(items
            .iter()
            .any(|v| v.signals.contains(&crate::models::ActionSignal::CiFailed)));
        assert!(items
            .iter()
            .any(|v| v.signals.contains(&crate::models::ActionSignal::Stale)));
        assert!(!items.iter().any(|v| v.number == 104));
    }

    #[test]
    fn note_preview_truncates_long_content() {
        let long: String = "字".repeat(120);
        assert_eq!(preview_of(&long).chars().count(), NOTE_PREVIEW_CHARS + 1);
        assert!(preview_of(&long).ends_with('…'));
        assert_eq!(preview_of("第一行\n第二行"), "第一行");
        assert_eq!(preview_of(""), "");
    }

    #[test]
    fn overview_isolates_domain_failures() {
        use crate::services::Services;
        use crate::storage::Storage;

        let dir =
            std::env::temp_dir().join(format!("maydolist-focus-fail-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        let services = Services::new(storage.clone());

        // Sabotage the todos directory (a file blocks read_dir) so only the
        // todo section fails; note and github must stay ready.
        std::fs::remove_dir_all(dir.join("todos")).unwrap();
        std::fs::write(dir.join("todos"), "not a directory").unwrap();
        let overview = services.focus.overview();

        assert_eq!(overview.todo.state, FocusSectionState::Error);
        assert!(overview.todo.error.is_some());
        assert_eq!(overview.note.state, FocusSectionState::Ready);
        assert_eq!(overview.github.state, FocusSectionState::Ready);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn overview_aggregates_real_data() {
        use crate::services::Services;
        use crate::storage::Storage;

        let dir =
            std::env::temp_dir().join(format!("maydolist-focus-data-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        let services = Services::new(storage.clone());

        let inbox = services.todo.ensure_inbox().unwrap();
        services
            .todo
            .create_item(
                &inbox.id,
                "收件箱任务".into(),
                None,
                crate::services::todo::TodoSchedule::default(),
            )
            .unwrap();
        let work = services.todo.create_list("工作".into()).unwrap();
        services
            .todo
            .create_item(
                &work.id,
                "工作任务".into(),
                None,
                crate::services::todo::TodoSchedule::default(),
            )
            .unwrap();
        let done = services
            .todo
            .create_item(
                &work.id,
                "已完成".into(),
                None,
                crate::services::todo::TodoSchedule::default(),
            )
            .unwrap();
        services
            .todo
            .update_item(&done.id, None, Some(true), None, None)
            .unwrap();
        let pinned_note = services
            .note
            .create("置顶便签".into(), "内容".into())
            .unwrap();
        services
            .note
            .update(
                &pinned_note.id,
                crate::services::note::NotePatch {
                    pinned: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();

        // Seed a GitHub snapshot cache directly (focus reads cache only).
        let mut snap = snapshot("owner/repo");
        snap.pull_requests = vec![pr(7, "open", "2026-08-01T00:00:00Z", &["mine"])];
        storage
            .write_json(
                &storage.data_dir().join("github/cache/owner_repo.json"),
                &snap,
            )
            .unwrap();
        let watchlist = vec![RepoWatch {
            full_name: "owner/repo".into(),
            filters: vec!["mine".into()],
            collapsed: false,
            ignored: vec![],
            pinned: vec![],
            signal_filters: vec![],
        }];
        storage
            .write_json(
                &storage.data_dir().join("github/watchlist.json"),
                &watchlist,
            )
            .unwrap();

        let overview = services.focus.overview();
        assert_eq!(overview.todo.state, FocusSectionState::Ready);
        assert_eq!(overview.todo.total, 2);
        let flat: Vec<&FocusTodo> = overview
            .todo
            .groups
            .iter()
            .flat_map(|g| g.items.iter())
            .collect();
        assert_eq!(flat.len(), 2);
        assert!(flat[0].inbox);
        assert_eq!(overview.note.items.len(), 1);
        assert!(overview.note.items[0].pinned);
        assert_eq!(overview.github.items.len(), 1);
        assert_eq!(overview.github.items[0].number, 7);
        std::fs::remove_dir_all(&dir).ok();
    }
}
