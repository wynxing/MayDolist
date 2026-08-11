use std::collections::HashSet;
use std::sync::Arc;

use crate::events::now_rfc3339;
use crate::models::{
    FocusGithub, FocusNote, FocusOverview, FocusSection, FocusSectionState, FocusTodo,
    GhAuthStatus, Note, RepoSnapshot, RepoWatch, TodoList,
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
            let todo = scope.spawn(|| self.load_todo());
            let note = scope.spawn(|| self.load_note());
            let github = scope.spawn(|| self.load_github());
            FocusOverview {
                generated_at,
                todo: todo
                    .join()
                    .unwrap_or_else(|_| FocusSection::error("todo loader panicked".into())),
                note: note
                    .join()
                    .unwrap_or_else(|_| FocusSection::error("note loader panicked".into())),
                github: github
                    .join()
                    .unwrap_or_else(|_| FocusSection::error("github loader panicked".into())),
            }
        })
    }

    fn load_todo(&self) -> FocusSection<FocusTodo> {
        match self.todo.list(false) {
            Ok(lists) => {
                let items = project_todos(&lists);
                let total = items.len();
                FocusSection {
                    state: FocusSectionState::Ready,
                    error: None,
                    total,
                    offline_cache: false,
                    items: items.into_iter().take(MAX_TODOS).collect(),
                }
            }
            Err(err) => FocusSection::error(err.to_string()),
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
            });
        }
    }
    // Stable sort keeps list/item sort order inside each inbox group.
    items.sort_by_key(|item| !item.inbox);
    items
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

/// Open GitHub items from local snapshot caches. Pinned (manually followed)
/// items come first, then by `updated_at` descending. Returns the items and
/// whether the section relies on an offline / stale cache.
pub fn project_github(
    _watchlist: &[RepoWatch],
    snapshots: &[RepoSnapshot],
    status: &GhAuthStatus,
) -> (Vec<FocusGithub>, bool) {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for snapshot in snapshots {
        for pr in &snapshot.pull_requests {
            if pr.state != "open" {
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
            });
        }
        for issue in &snapshot.issues {
            if issue.state != "open" {
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
        }
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
            .create_item(&inbox.id, "收件箱任务".into())
            .unwrap();
        let work = services.todo.create_list("工作".into()).unwrap();
        services
            .todo
            .create_item(&work.id, "工作任务".into())
            .unwrap();
        let done = services
            .todo
            .create_item(&work.id, "已完成".into())
            .unwrap();
        services
            .todo
            .update_item(&done.id, None, Some(true), None)
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
        assert!(overview.todo.items[0].inbox);
        assert_eq!(overview.note.items.len(), 1);
        assert!(overview.note.items[0].pinned);
        assert_eq!(overview.github.items.len(), 1);
        assert_eq!(overview.github.items[0].number, 7);
        std::fs::remove_dir_all(&dir).ok();
    }
}
