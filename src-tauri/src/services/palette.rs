use std::sync::Arc;

use crate::models::{
    Note, PaletteCommand, PaletteGithub, PaletteNote, PaletteSearchResult, PaletteTodo,
    RepoSnapshot, TodoList,
};
use crate::services::github::GithubService;
use crate::services::note::NoteService;
use crate::services::todo::{TodoService, INBOX_KIND};

/// Per-domain display caps keep the palette bounded even with large datasets.
pub const MAX_TODO_RESULTS: usize = 8;
pub const MAX_NOTE_RESULTS: usize = 8;
pub const MAX_GITHUB_RESULTS: usize = 8;
/// Truncation length for the note preview.
pub const NOTE_PREVIEW_CHARS: usize = 80;

/// Read-only aggregated search across todo / note / github. Loading is
/// parallel and per-domain isolated: a failing domain degrades to an empty
/// section and never blocks the other sections or the command list.
pub struct PaletteService {
    todo: Arc<TodoService>,
    note: Arc<NoteService>,
    github: Arc<GithubService>,
}

impl PaletteService {
    pub fn new(todo: Arc<TodoService>, note: Arc<NoteService>, github: Arc<GithubService>) -> Self {
        Self { todo, note, github }
    }

    pub fn search(&self, query: &str) -> PaletteSearchResult {
        let query = query.trim();
        let commands = match_commands(query);
        if query.is_empty() {
            // An empty input only shows the command list; no domain search.
            return PaletteSearchResult {
                query: query.into(),
                commands,
                todos: Vec::new(),
                notes: Vec::new(),
                github: Vec::new(),
                github_offline: false,
            };
        }
        std::thread::scope(|scope| {
            let todos = scope.spawn(|| self.load_todos(query));
            let notes = scope.spawn(|| self.load_notes(query));
            let github = scope.spawn(|| self.load_github(query));
            let github = github.join().unwrap_or_default();
            PaletteSearchResult {
                query: query.into(),
                commands,
                todos: todos.join().unwrap_or_default(),
                notes: notes.join().unwrap_or_default(),
                github: github.0,
                github_offline: github.1,
            }
        })
    }

    fn load_todos(&self, query: &str) -> Vec<PaletteTodo> {
        match self.todo.list(false) {
            Ok(lists) => filter_todos(query, &lists, MAX_TODO_RESULTS),
            Err(_) => Vec::new(),
        }
    }

    fn load_notes(&self, query: &str) -> Vec<PaletteNote> {
        match self.note.list(false) {
            Ok(notes) => filter_notes(query, &notes, MAX_NOTE_RESULTS),
            Err(_) => Vec::new(),
        }
    }

    /// Search only local snapshot caches (watchlist repos), never the
    /// network. Returns the matched items and whether they come from a stale /
    /// offline cache (not logged in or a previous refresh failed).
    fn load_github(&self, query: &str) -> (Vec<PaletteGithub>, bool) {
        let status = self.github.status();
        let watchlist = match self.github.watchlist() {
            Ok(watchlist) => watchlist,
            Err(_) => return (Vec::new(), !status.logged_in),
        };
        let mut snapshots = Vec::new();
        for watch in &watchlist {
            if let Ok(Some(snapshot)) = self.github.snapshot(&watch.full_name) {
                snapshots.push(snapshot);
            }
        }
        let offline = !status.logged_in
            || snapshots
                .iter()
                .any(|snapshot| snapshot.last_error.is_some());
        (
            filter_github(query, &snapshots, MAX_GITHUB_RESULTS),
            offline,
        )
    }
}

/// Case-insensitive match rank: `0` exact, `1` prefix, `2` substring; `None`
/// means no match. Empty query or text never matches (callers special-case an
/// empty query to the command list only).
pub fn match_rank(query: &str, text: &str) -> Option<u8> {
    let query = query.trim().to_lowercase();
    let text = text.trim().to_lowercase();
    if query.is_empty() || text.is_empty() {
        return None;
    }
    if text == query {
        Some(0)
    } else if text.starts_with(&query) {
        Some(1)
    } else if text.contains(&query) {
        Some(2)
    } else {
        None
    }
}

/// The full static command list in display order.
pub fn palette_commands() -> Vec<PaletteCommand> {
    vec![
        command(
            "go-focus",
            "切换到今日焦点",
            "打开 Focus 今日视图",
            &["focus", "今日", "焦点"],
        ),
        command(
            "go-todo",
            "切换到待办",
            "打开 Todo 列表",
            &["todo", "待办", "任务"],
        ),
        command(
            "go-note",
            "切换到便签",
            "打开便签模块",
            &["note", "便签", "笔记"],
        ),
        command(
            "go-github",
            "切换到 GitHub",
            "打开 GitHub 追踪",
            &["github", "pr", "issue", "仓库"],
        ),
        command(
            "go-settings",
            "打开设置",
            "打开设置页",
            &["settings", "设置", "配置"],
        ),
        command(
            "new-todo",
            "新建 Todo",
            "输入标题后保存到收件箱",
            &["todo", "新建", "添加", "待办"],
        ),
        command(
            "new-note",
            "新建便签",
            "输入标题后创建便签",
            &["note", "新建", "便签", "笔记"],
        ),
        command(
            "backup-now",
            "立即备份",
            "在数据目录创建一份本地备份",
            &["backup", "备份"],
        ),
        command(
            "open-data-dir",
            "打开数据目录",
            "在资源管理器中打开本地数据目录",
            &["data", "数据目录", "打开", "目录"],
        ),
        command(
            "refresh-github",
            "刷新 GitHub",
            "重新拉取全部追踪仓库的 PR / Issue",
            &["github", "刷新", "refresh", "pr", "issue"],
        ),
        command(
            "open-quick-capture",
            "打开快速收集",
            "呼出快速收集窗口记录想法",
            &["quick", "快速收集", "捕获", "记录"],
        ),
    ]
}

fn command(id: &str, label: &str, hint: &str, keywords: &[&str]) -> PaletteCommand {
    PaletteCommand {
        id: id.into(),
        label: label.into(),
        hint: hint.into(),
        keywords: keywords.iter().map(|v| v.to_string()).collect(),
    }
}

/// Match commands against the query. An empty query returns the full list in
/// display order; otherwise commands rank by the best match across label /
/// keywords / id (exact < prefix < substring), preserving display order for
/// ties.
pub fn match_commands(query: &str) -> Vec<PaletteCommand> {
    let query = query.trim();
    if query.is_empty() {
        return palette_commands();
    }
    let mut matched: Vec<(u8, PaletteCommand)> = Vec::new();
    for command in palette_commands() {
        let mut best: Option<u8> = None;
        for text in std::iter::once(&command.label)
            .chain(command.keywords.iter())
            .chain(std::iter::once(&command.id))
        {
            if let Some(rank) = match_rank(query, text) {
                best = Some(best.map_or(rank, |current| current.min(rank)));
            }
        }
        if let Some(rank) = best {
            matched.push((rank, command));
        }
    }
    matched.sort_by_key(|(rank, _)| *rank);
    matched.into_iter().map(|(_, command)| command).collect()
}

/// Incomplete, non-deleted Todo items whose title matches the query. Inbox
/// items come first, then by `updated_at` descending, capped at `limit`.
pub fn filter_todos(query: &str, lists: &[TodoList], limit: usize) -> Vec<PaletteTodo> {
    let mut out = Vec::new();
    for list in lists {
        if list.deleted {
            continue;
        }
        let inbox = list.kind.as_deref() == Some(INBOX_KIND);
        for item in &list.items {
            if item.deleted || item.completed || match_rank(query, &item.title).is_none() {
                continue;
            }
            out.push(PaletteTodo {
                id: item.id.clone(),
                title: item.title.clone(),
                list_id: list.id.clone(),
                list_title: list.title.clone(),
                inbox,
                updated_at: item.updated_at.clone(),
                source: item.source.clone(),
                due_date: item.due_date.clone(),
            });
        }
    }
    out.sort_by(|a, b| {
        b.inbox
            .cmp(&a.inbox)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    out.truncate(limit);
    out
}

/// Non-deleted Notes whose title or full-text content matches the query.
/// Pinned notes first, then by `updated_at` descending, capped at `limit`.
pub fn filter_notes(query: &str, notes: &[Note], limit: usize) -> Vec<PaletteNote> {
    let mut out = Vec::new();
    for note in notes {
        if note.deleted {
            continue;
        }
        if match_rank(query, &note.title).is_none() && match_rank(query, &note.content).is_none() {
            continue;
        }
        out.push(PaletteNote {
            id: note.id.clone(),
            title: note.title.clone(),
            preview: preview_of(&note.content),
            pinned: note.pinned,
            floating: note.floating,
            updated_at: note.updated_at.clone(),
        });
    }
    out.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    out.truncate(limit);
    out
}

/// Open GitHub items from local snapshot caches matching title, repo or
/// `#number`, by `updated_at` descending, capped at `limit`. Closed items and
/// duplicate keys (same repo + number + kind) are excluded.
pub fn filter_github(query: &str, snapshots: &[RepoSnapshot], limit: usize) -> Vec<PaletteGithub> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for snapshot in snapshots {
        for pr in &snapshot.pull_requests {
            if pr.state != "open"
                || !github_matches(query, &snapshot.repo, pr.number, &pr.title)
                || !seen.insert((snapshot.repo.clone(), pr.number, "pr".to_string()))
            {
                continue;
            }
            out.push(PaletteGithub {
                kind: "pr".into(),
                repo: snapshot.repo.clone(),
                number: pr.number,
                title: pr.title.clone(),
                url: pr.url.clone(),
                updated_at: pr.updated_at.clone(),
            });
        }
        for issue in &snapshot.issues {
            if issue.state != "open"
                || !github_matches(query, &snapshot.repo, issue.number, &issue.title)
                || !seen.insert((snapshot.repo.clone(), issue.number, "issue".to_string()))
            {
                continue;
            }
            out.push(PaletteGithub {
                kind: "issue".into(),
                repo: snapshot.repo.clone(),
                number: issue.number,
                title: issue.title.clone(),
                url: issue.url.clone(),
                updated_at: issue.updated_at.clone(),
            });
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out.truncate(limit);
    out
}

fn github_matches(query: &str, repo: &str, number: u64, title: &str) -> bool {
    match_rank(query, title).is_some()
        || match_rank(query, repo).is_some()
        || match_rank(query, &format!("#{number}")).is_some()
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
    use crate::models::{
        GhIssue, GhPullRequest, Note, RepoWatch, TodoItem, TodoList, TodoSource, WindowBounds,
    };
    use crate::services::Services;
    use crate::storage::Storage;

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

    fn todo_item(
        id: &str,
        title: &str,
        completed: bool,
        updated_at: &str,
        source: Option<TodoSource>,
    ) -> TodoItem {
        TodoItem {
            id: id.into(),
            title: title.into(),
            completed,
            deleted: false,
            sort_order: 0,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: updated_at.into(),
            source,
            github_sync: None,
            due_date: None,
            remind_at: None,
            repeat: None,
            repeat_until: None,
            last_reminded_at: None,
        }
    }

    fn note(id: &str, title: &str, content: &str, pinned: bool, updated_at: &str) -> Note {
        Note {
            schema_version: 1,
            id: id.into(),
            title: title.into(),
            content: content.into(),
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

    fn pr(number: u64, title: &str, updated_at: &str) -> GhPullRequest {
        GhPullRequest {
            number,
            title: title.into(),
            state: "open".into(),
            draft: false,
            url: format!("https://example.test/pull/{number}"),
            updated_at: updated_at.into(),
            matches: vec!["mine".into()],
            assignees: vec![],
            reviewers: vec![],
            head_sha: None,
            checks_state: None,
            signals: vec![],
        }
    }

    fn issue(number: u64, title: &str, updated_at: &str) -> GhIssue {
        GhIssue {
            number,
            title: title.into(),
            state: "open".into(),
            url: format!("https://example.test/issues/{number}"),
            updated_at: updated_at.into(),
            kind: "issue".into(),
            matches: vec!["mine".into()],
            assignees: vec![],
            signals: vec![],
        }
    }

    #[test]
    fn match_rank_orders_exact_prefix_substring() {
        assert_eq!(match_rank("todo", "todo"), Some(0));
        assert_eq!(match_rank("todo", "TODO"), Some(0));
        assert_eq!(match_rank("todo", "todolist"), Some(1));
        assert_eq!(match_rank("todo", "我的 todo 列表"), Some(2));
        assert_eq!(match_rank("修复", "修复登录"), Some(1));
        assert_eq!(match_rank("登录", "修复登录"), Some(2));
        assert_eq!(match_rank("zzz", "todo"), None);
        assert_eq!(match_rank("", "todo"), None);
        assert_eq!(match_rank("todo", ""), None);
    }

    #[test]
    fn empty_query_returns_full_command_list_in_order() {
        let commands = match_commands("");
        assert_eq!(commands.len(), palette_commands().len());
        assert_eq!(commands[0].id, "go-focus");
        assert_eq!(commands[8].id, "open-data-dir");
        assert_eq!(commands[9].id, "refresh-github");
        assert_eq!(commands[10].id, "open-quick-capture");
    }

    #[test]
    fn github_refresh_and_quick_capture_commands_match() {
        let refresh = match_commands("刷新 github");
        assert_eq!(refresh[0].id, "refresh-github");
        let capture = match_commands("快速收集");
        assert_eq!(capture[0].id, "open-quick-capture");
    }

    #[test]
    fn commands_match_by_label_keyword_and_id() {
        let by_label = match_commands("打开设置");
        assert_eq!(by_label[0].id, "go-settings");
        let by_keyword = match_commands("github");
        assert_eq!(by_keyword[0].id, "go-github");
        let by_id = match_commands("new-todo");
        assert_eq!(by_id[0].id, "new-todo");
        // Chinese keyword alias works case-insensitively too.
        let by_chinese = match_commands("备份");
        assert_eq!(by_chinese[0].id, "backup-now");
    }

    #[test]
    fn prefix_matches_rank_above_substring() {
        let commands = match_commands("to");
        // "新建 Todo" / "切换到待办" hit by keywords; ids/labels that start
        // with "to" rank first.
        assert!(commands.iter().position(|c| c.id == "go-todo").is_some());
        assert!(commands.iter().position(|c| c.id == "new-todo").is_some());
    }

    #[test]
    fn filter_todos_matches_title_skips_completed_and_caps() {
        let mut inbox = todo_list("inbox", "收件箱", Some(INBOX_KIND), 0);
        inbox.items = vec![
            todo_item("t1", "修复登录", false, "2026-08-02T00:00:00Z", None),
            todo_item("t2", "登录页改版", true, "2026-08-03T00:00:00Z", None),
            todo_item("t3", "修复注册", false, "2026-08-04T00:00:00Z", None),
        ];
        let mut work = todo_list("work", "工作", None, 1);
        work.items = vec![todo_item(
            "t4",
            "登录联调",
            false,
            "2026-08-05T00:00:00Z",
            None,
        )];
        let mut deleted = todo_list("del", "已删除", None, 2);
        deleted.deleted = true;
        deleted.items = vec![todo_item(
            "t5",
            "修复登录",
            false,
            "2026-08-06T00:00:00Z",
            None,
        )];

        let items = filter_todos("登录", &[work, inbox.clone(), deleted], 10);
        let ids: Vec<&str> = items.iter().map(|v| v.id.as_str()).collect();
        // Inbox t1 ("修复登录") first, then work t4 ("登录联调"); t3
        // ("修复注册") does not contain the query.
        assert_eq!(ids, vec!["t1", "t4"]);
        assert!(items.iter().all(|v| !v.inbox || v.list_title == "收件箱"));
        assert!(items[0].inbox);

        let capped = filter_todos("修复", &[inbox], 1);
        assert_eq!(capped.len(), 1);
    }

    #[test]
    fn filter_todos_carries_source_and_due_date() {
        let mut inbox = todo_list("inbox", "收件箱", Some(INBOX_KIND), 0);
        let mut item = todo_item(
            "s1",
            "跟进 issue 修复",
            false,
            "2026-08-05T00:00:00Z",
            Some(TodoSource {
                kind: "github-issue".into(),
                repo: "owner/repo".into(),
                number: 7,
                url: "https://github.com/owner/repo/issues/7".into(),
            }),
        );
        item.due_date = Some("2026-08-20".into());
        inbox.items = vec![item];
        let items = filter_todos("issue", &[inbox], 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source.as_ref().unwrap().number, 7);
        assert_eq!(items[0].due_date.as_deref(), Some("2026-08-20"));
    }

    #[test]
    fn filter_notes_searches_full_text_pinned_first_and_truncates() {
        let notes = vec![
            note(
                "n1",
                "会议记录",
                "讨论了登录模块的设计",
                false,
                "2026-08-02T00:00:00Z",
            ),
            note("n2", "发布计划", "下周五发布", true, "2026-08-01T00:00:00Z"),
            note(
                "n3",
                "其它",
                "登录问题已修复",
                false,
                "2026-08-03T00:00:00Z",
            ),
        ];
        let items = filter_notes("登录", &notes, 10);
        // n1 matches content; n3 matches content. n2 does not match.
        let ids: Vec<&str> = items.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&"n1") && ids.contains(&"n3"));

        let pinned = filter_notes("发布", &notes, 10);
        assert_eq!(pinned[0].id, "n2");
        assert!(pinned[0].pinned);

        let long: String = "字".repeat(120);
        let preview = preview_of(&long);
        assert_eq!(preview.chars().count(), NOTE_PREVIEW_CHARS + 1);
        assert!(preview.ends_with('…'));

        let capped = filter_notes("登录", &notes, 1);
        assert_eq!(capped.len(), 1);
    }

    #[test]
    fn filter_github_matches_title_repo_number_and_caps() {
        let mut snap = snapshot("owner/repo");
        snap.pull_requests = vec![
            pr(101, "修复登录流程", "2026-08-02T00:00:00Z"),
            pr(102, "登录超时", "2026-08-01T00:00:00Z"),
            pr(103, "closed", "2026-08-03T00:00:00Z"),
        ];
        snap.pull_requests[2].state = "closed".into();
        snap.issues = vec![issue(201, "首页性能", "2026-08-03T00:00:00Z")];

        let items = filter_github("登录", &[snap.clone()], 10);
        let numbers: Vec<u64> = items.iter().map(|v| v.number).collect();
        // Open only, updated desc: #101 (08-02) then #102 (08-01).
        assert_eq!(numbers, vec![101, 102]);
        assert!(items.iter().all(|v| v.kind == "pr"));

        // Repo and #number matching.
        let by_repo = filter_github("owner/repo", &[snap.clone()], 10);
        let repo_numbers: Vec<u64> = by_repo.iter().map(|v| v.number).collect();
        // Open items only (#103 closed excluded), updated desc: #201, #101, #102.
        assert_eq!(repo_numbers, vec![201, 101, 102]);
        let by_number = filter_github("#201", &[snap.clone()], 10);
        assert_eq!(by_number.len(), 1);
        assert_eq!(by_number[0].kind, "issue");

        let capped = filter_github("登录", &[snap], 1);
        assert_eq!(capped.len(), 1);
    }

    #[test]
    fn empty_query_returns_commands_only() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::with_dir(tmp.path()).unwrap());
        let services = Services::new(storage.clone());
        let result = services.palette.search("");
        assert!(!result.commands.is_empty());
        assert!(result.todos.is_empty());
        assert!(result.notes.is_empty());
        assert!(result.github.is_empty());
    }

    #[test]
    fn search_aggregates_real_data_across_domains() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::with_dir(tmp.path()).unwrap());
        let services = Services::new(storage.clone());

        let inbox = services.todo.ensure_inbox().unwrap();
        services
            .todo
            .create_item(
                &inbox.id,
                "修复登录".into(),
                None,
                crate::services::todo::TodoSchedule::default(),
            )
            .unwrap();
        services
            .note
            .create("发布计划".into(), "登录联调安排".into())
            .unwrap();

        let mut snap = snapshot("owner/repo");
        snap.pull_requests = vec![pr(7, "登录页性能优化", "2026-08-01T00:00:00Z")];
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

        let result = services.palette.search("登录");
        assert_eq!(result.query, "登录");
        assert_eq!(result.todos.len(), 1);
        assert_eq!(result.todos[0].title, "修复登录");
        assert!(result.todos[0].inbox);
        assert_eq!(result.notes.len(), 1);
        assert_eq!(result.notes[0].title, "发布计划");
        assert_eq!(result.github.len(), 1);
        assert_eq!(result.github[0].number, 7);
        assert_eq!(result.github[0].repo, "owner/repo");

        // Todos that do not match stay out; commands still match keywords.
        let none = services.palette.search("完全不存在的词");
        assert!(none.todos.is_empty() && none.notes.is_empty() && none.github.is_empty());
        assert!(none.commands.is_empty());

        let commands = services.palette.search("备份");
        assert_eq!(commands.commands[0].id, "backup-now");
    }

    #[test]
    fn search_marks_offline_cache_from_snapshot_error() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::with_dir(tmp.path()).unwrap());
        let services = Services::new(storage.clone());

        let mut snap = snapshot("owner/repo");
        snap.last_error = Some("network down".into());
        snap.pull_requests = vec![pr(7, "登录页性能优化", "2026-08-01T00:00:00Z")];
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

        let result = services.palette.search("登录");
        assert_eq!(result.github.len(), 1, "cached items still shown offline");
        assert!(
            result.github_offline,
            "a snapshot carrying lastError must mark the section offline"
        );
    }
}
