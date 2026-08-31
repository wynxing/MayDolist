use std::sync::Arc;

use super::refresh::FetchedItem;
use super::signals::{
    apply_signals, compute_signals, merge_pinned_item, sort_snapshot, strip_pin_marker,
    SignalInputs,
};
use super::sync::{parse_linked_repo_states, source_alias};
use super::{GhRunner, GithubService};
use crate::error::AppError;
use crate::models::{
    ActionSignal, GhIssue, GhPullRequest, GithubSyncState, RepoSnapshot, RepoWatch, TodoSource,
};
use crate::services::todo::TodoService;
use crate::storage::Storage;

fn matches(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn repo_snapshot(repo: &str) -> RepoSnapshot {
    RepoSnapshot {
        schema_version: 2,
        repo: repo.into(),
        fetched_at: "2026-08-15T00:00:00Z".into(),
        last_success_at: Some("2026-08-15T00:00:00Z".into()),
        last_error: None,
        issues: vec![],
        pull_requests: vec![],
        signals_computed_at: Some("2026-08-15T00:00:00Z".into()),
    }
}

fn pr(number: u64, updated_at: &str, matches: &[&str], signals: &[ActionSignal]) -> GhPullRequest {
    GhPullRequest {
        number,
        title: format!("PR #{number}"),
        state: "open".into(),
        draft: false,
        url: format!("https://example.test/pull/{number}"),
        updated_at: updated_at.into(),
        matches: matches.iter().map(|v| v.to_string()).collect(),
        assignees: vec![],
        reviewers: vec![],
        head_sha: None,
        checks_state: None,
        signals: signals.to_vec(),
    }
}

#[test]
fn compute_signals_maps_response_fixtures() {
    let now = "2026-08-15T00:00:00Z";
    let input = |is_pr: bool,
                 draft: bool,
                 assignees: Vec<String>,
                 reviewers: Vec<String>,
                 checks_state: Option<String>,
                 matches: Vec<String>,
                 updated_at: &str|
     -> SignalInputs {
        SignalInputs {
            is_pr,
            draft,
            assignees,
            reviewers,
            checks_state,
            matches,
            user: "wynxing".into(),
            stale_days: 14,
            updated_at: updated_at.into(),
            now: now.into(),
        }
    };

    // 需要我处理：被分配过滤器命中，assignees 字段兜底。
    let signals = compute_signals(input(
        false,
        false,
        vec!["wynxing".into()],
        vec![],
        None,
        matches(&["assigned"]),
        "2026-08-10T00:00:00Z",
    ));
    assert_eq!(signals, vec![ActionSignal::NeedsAction]);

    // 需要 Review：requested_reviewers 含当前用户；check 成功不误报。
    let signals = compute_signals(input(
        true,
        false,
        vec![],
        vec!["alice".into(), "wynxing".into()],
        Some("success".into()),
        matches(&["mine"]),
        "2026-08-10T00:00:00Z",
    ));
    assert_eq!(signals, vec![ActionSignal::NeedsReview]);

    // CI 失败：failure / error 都算失败，pending 不算。
    let signals = compute_signals(input(
        true,
        false,
        vec![],
        vec![],
        Some("failure".into()),
        matches(&["mine"]),
        "2026-08-10T00:00:00Z",
    ));
    assert_eq!(signals, vec![ActionSignal::CiFailed]);
    let signals = compute_signals(input(
        true,
        false,
        vec![],
        vec![],
        Some("error".into()),
        matches(&["mine"]),
        "2026-08-10T00:00:00Z",
    ));
    assert_eq!(signals, vec![ActionSignal::CiFailed]);
    let signals = compute_signals(input(
        true,
        false,
        vec![],
        vec![],
        Some("pending".into()),
        matches(&["mine"]),
        "2026-08-10T00:00:00Z",
    ));
    assert!(signals.is_empty());

    // Draft 仅作用于 PR；issue 的 draft 字段不产生信号。
    let signals = compute_signals(input(
        true,
        true,
        vec![],
        vec![],
        Some("success".into()),
        matches(&["mine"]),
        "2026-08-10T00:00:00Z",
    ));
    assert_eq!(signals, vec![ActionSignal::Draft]);
    let signals = compute_signals(input(
        false,
        true,
        vec![],
        vec![],
        None,
        matches(&["mine"]),
        "2026-08-10T00:00:00Z",
    ));
    assert!(signals.is_empty());

    // 长期未更新：超过配置天数。
    let signals = compute_signals(input(
        true,
        false,
        vec![],
        vec![],
        None,
        matches(&["mine"]),
        "2026-07-01T00:00:00Z",
    ));
    assert_eq!(signals, vec![ActionSignal::Stale]);

    // 手动关注（pinned）也算需要我处理。
    let signals = compute_signals(input(
        false,
        false,
        vec![],
        vec![],
        None,
        matches(&["pinned"]),
        "2026-08-10T00:00:00Z",
    ));
    assert_eq!(signals, vec![ActionSignal::NeedsAction]);

    // 完全无关的条目（all-prs 且无 review/check/draft）不产生信号。
    let signals = compute_signals(input(
        true,
        false,
        vec![],
        vec![],
        None,
        matches(&["all-prs"]),
        "2026-08-10T00:00:00Z",
    ));
    assert!(signals.is_empty());
}

#[test]
fn sort_snapshot_orders_pinned_then_actionable_then_recent() {
    let mut snapshot = repo_snapshot("owner/repo");
    snapshot.pull_requests = vec![
        pr(
            1,
            "2026-08-01T00:00:00Z",
            &["mine"],
            &[ActionSignal::NeedsAction],
        ),
        pr(
            2,
            "2026-08-03T00:00:00Z",
            &["mine"],
            &[ActionSignal::NeedsAction],
        ),
        pr(
            3,
            "2026-08-04T00:00:00Z",
            &["mine"],
            &[ActionSignal::NeedsAction, ActionSignal::CiFailed],
        ),
        pr(
            4,
            "2026-08-02T00:00:00Z",
            &["pinned"],
            &[ActionSignal::NeedsAction, ActionSignal::NeedsReview],
        ),
        pr(
            5,
            "2026-08-05T00:00:00Z",
            &["all-prs"],
            &[ActionSignal::Draft],
        ),
    ];
    sort_snapshot(&mut snapshot);
    let numbers: Vec<u64> = snapshot.pull_requests.iter().map(|p| p.number).collect();
    assert_eq!(numbers, vec![4, 3, 2, 1, 5]);
}

#[test]
fn set_signal_filters_accepts_only_stable_names() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::with_dir(tmp.path()).unwrap());
    let service = GithubService::new(storage.clone());
    let watch = RepoWatch {
        full_name: "owner/repo".into(),
        filters: vec!["mine".into()],
        collapsed: false,
        ignored: vec![],
        pinned: vec![],
        signal_filters: vec![],
    };
    storage
        .write_json(
            &storage.data_dir().join("github/watchlist.json"),
            &vec![watch],
        )
        .unwrap();

    let list = service
        .set_signal_filters(
            "owner/repo",
            vec![
                "needsAction".into(),
                "review_requested".into(),
                "ciFailed".into(),
                "draft".into(),
                "stale".into(),
            ],
        )
        .unwrap();
    assert_eq!(
        list[0].signal_filters,
        vec!["needsAction", "ciFailed", "stale"]
    );

    let list = service.set_signal_filters("owner/repo", vec![]).unwrap();
    assert!(list[0].signal_filters.is_empty());
    assert!(service.set_signal_filters("missing/repo", vec![]).is_err());
}

#[test]
fn snapshot_serves_fresh_stale_signal() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::with_dir(tmp.path()).unwrap());
    let service = GithubService::new(storage.clone());
    let mut snapshot = repo_snapshot("owner/repo");
    snapshot.pull_requests = vec![pr(
        1,
        "2026-06-01T00:00:00Z",
        &["mine"],
        &[ActionSignal::NeedsAction],
    )];
    storage
        .write_json(
            &storage.data_dir().join("github/cache/owner_repo.json"),
            &snapshot,
        )
        .unwrap();
    let served = service.snapshot("owner/repo").unwrap().unwrap();
    assert!(served.pull_requests[0]
        .signals
        .contains(&ActionSignal::Stale));
}

fn linked_source(kind: &str, number: u64) -> TodoSource {
    TodoSource {
        kind: kind.into(),
        repo: "owner/repo".into(),
        number,
        url: format!("https://example.test/{number}"),
    }
}

#[test]
fn source_alias_uses_kind_prefix() {
    assert_eq!(source_alias(&linked_source("github-pr", 42)), "pr42");
    assert_eq!(source_alias(&linked_source("github-issue", 7)), "issue7");
}

#[test]
fn parse_linked_repo_states_maps_graphql_states() {
    let response = serde_json::json!({
        "data": {
            "repository": {
                "pr1": { "state": "OPEN" },
                "pr2": { "state": "MERGED" },
                "issue3": { "state": "CLOSED" },
                "pr4": null
            }
        }
    });
    let states = parse_linked_repo_states(&response).unwrap();
    assert_eq!(states.get("pr1"), Some(&GithubSyncState::Open));
    assert_eq!(states.get("pr2"), Some(&GithubSyncState::Merged));
    assert_eq!(states.get("issue3"), Some(&GithubSyncState::Closed));
    // null 节点（条目删除/不可见）应被跳过，交由调用方按源记失败。
    assert!(!states.contains_key("pr4"));
}

#[test]
fn parse_linked_repo_states_rejects_missing_repository() {
    let response = serde_json::json!({ "data": {} });
    assert!(parse_linked_repo_states(&response).is_err());
    let response = serde_json::json!({ "data": { "repository": "oops" } });
    assert!(parse_linked_repo_states(&response).is_err());
}

fn issue_fixture(number: u64, updated_at: &str, matches: &[&str]) -> GhIssue {
    GhIssue {
        number,
        title: format!("Issue #{number}"),
        state: "open".into(),
        url: format!("https://example.test/issues/{number}"),
        updated_at: updated_at.into(),
        kind: "issue".into(),
        matches: matches.iter().map(|v| v.to_string()).collect(),
        assignees: vec![],
        signals: vec![],
    }
}

#[test]
fn apply_signals_recomputes_open_items_and_clears_closed_ones() {
    let mut snapshot = repo_snapshot("owner/repo");
    let mut open_pr = pr(1, "2026-08-14T00:00:00Z", &["mine"], &[ActionSignal::Stale]);
    open_pr.reviewers = vec!["wynxing".into()];
    let mut closed_issue = issue_fixture(2, "2026-08-14T00:00:00Z", &["assigned"]);
    closed_issue.state = "closed".into();
    closed_issue.signals = vec![ActionSignal::NeedsAction];
    snapshot.pull_requests = vec![open_pr];
    snapshot.issues = vec![closed_issue];

    apply_signals(&mut snapshot, "wynxing", 14, "2026-08-15T00:00:00Z");
    assert_eq!(
        snapshot.pull_requests[0].signals,
        vec![ActionSignal::NeedsReview]
    );
    // 关闭条目不保留任何信号（展示规则：置灰、无行动徽标）。
    assert!(snapshot.issues[0].signals.is_empty());
}

#[test]
fn merge_pinned_item_dedups_and_strip_pin_marker_removes_orphans() {
    let mut snapshot = repo_snapshot("owner/repo");
    snapshot.pull_requests = vec![pr(1, "2026-08-14T00:00:00Z", &["mine"], &[])];

    // 已存在的 PR 只补 pinned 标记，不重复插入。
    merge_pinned_item(
        &mut snapshot,
        FetchedItem::Pr(pr(1, "2026-08-14T00:00:00Z", &["pinned"], &[])),
    );
    assert_eq!(snapshot.pull_requests.len(), 1);
    assert!(snapshot.pull_requests[0]
        .matches
        .iter()
        .any(|m| m == "pinned"));

    // 取消关注后：还有其他匹配来源的条目只去掉标记，继续保留。
    strip_pin_marker(&mut snapshot, 1);
    assert_eq!(snapshot.pull_requests.len(), 1);
    assert_eq!(snapshot.pull_requests[0].matches, vec!["mine"]);

    // 仅靠 pinned 存在的条目，取消关注后被整体移除。
    snapshot.pull_requests = vec![pr(3, "2026-08-14T00:00:00Z", &["pinned"], &[])];
    strip_pin_marker(&mut snapshot, 3);
    assert!(snapshot.pull_requests.is_empty());

    merge_pinned_item(
        &mut snapshot,
        FetchedItem::Issue(issue_fixture(
            2,
            "2026-08-14T00:00:00Z",
            &["mine", "pinned"],
        )),
    );
    strip_pin_marker(&mut snapshot, 2);
    assert_eq!(snapshot.issues.len(), 1);
    assert_eq!(snapshot.issues[0].matches, vec!["mine"]);
}

fn github_test_storage(tag: &str) -> (tempfile::TempDir, Arc<Storage>) {
    let tmp = tempfile::Builder::new()
        .prefix(&format!("maydolist-gh-{tag}-"))
        .tempdir()
        .unwrap();
    let storage = Arc::new(Storage::with_dir(tmp.path()).unwrap());
    (tmp, storage)
}

#[test]
fn sync_linked_todos_uses_one_graphql_call_per_repo() {
    let (_tmp, storage) = github_test_storage("sync-batch");
    let todo = TodoService::new(storage.clone());
    todo.ensure_inbox().unwrap();
    todo.create_item_from_github(
        "github-pr",
        "owner/repo",
        1,
        "PR 一",
        "https://github.com/owner/repo/pull/1",
    )
    .unwrap();
    todo.create_item_from_github(
        "github-issue",
        "owner/repo",
        2,
        "Issue 二",
        "https://github.com/owner/repo/issues/2",
    )
    .unwrap();
    todo.create_item_from_github(
        "github-pr",
        "other/repo",
        3,
        "PR 三",
        "https://github.com/other/repo/pull/3",
    )
    .unwrap();

    let graphql_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = graphql_calls.clone();
    let runner: GhRunner = Arc::new(move |args: &[&str]| {
        if args.contains(&"--version") {
            return Ok("gh version 2.60.0".into());
        }
        if args.first() == Some(&"auth") {
            return Ok(String::new());
        }
        if args.contains(&".login") {
            return Ok("wynxing".into());
        }
        let joined = args.join(" ");
        if joined.contains("graphql") {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if joined.contains("owner: \"other\"") {
                return Ok(r#"{"data":{"repository":{"pr3":{"state":"MERGED"}}}}"#.into());
            }
            if joined.contains("owner: \"owner\"") {
                return Ok(
                    r#"{"data":{"repository":{"pr1":{"state":"OPEN"},"issue2":{"state":"CLOSED"}}}}"#
                        .into(),
                );
            }
            return Err(AppError::Github("unexpected graphql call".into()));
        }
        Err(AppError::Github(format!("unexpected args: {joined}")))
    });
    let service = GithubService::new_with_runner(storage, false, runner);

    let summary = service.sync_linked_todos(&todo, true);
    assert_eq!(
        graphql_calls.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "一个仓库只发一条 GraphQL"
    );
    assert_eq!(summary.checked, 3);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.auto_completed, 2);

    let items = &todo.list(false).unwrap()[0].items;
    assert_eq!(items.len(), 3);
    let by_number = |n: u64| {
        items
            .iter()
            .find(|v| v.source.as_ref().is_some_and(|s| s.number == n))
            .unwrap()
    };
    assert!(!by_number(1).completed);
    assert!(by_number(2).completed);
    assert!(by_number(3).completed);
    assert_eq!(
        by_number(3).github_sync.as_ref().unwrap().state,
        GithubSyncState::Merged
    );
}

#[test]
fn refresh_inner_merges_filter_matches_and_enriches_open_prs() {
    let (_tmp, storage) = github_test_storage("refresh-merge");
    storage
        .write_json(
            &storage.data_dir().join("github/watchlist.json"),
            &vec![RepoWatch {
                full_name: "owner/repo".into(),
                filters: vec!["mine".into(), "assigned".into()],
                collapsed: false,
                ignored: vec![],
                pinned: vec![],
                signal_filters: vec![],
            }],
        )
        .unwrap();
    let updated = "2026-08-14T00:00:00Z";
    let mine_page = format!(
        r#"{{"items":[
            {{"number":10,"title":"PR 十","state":"open","html_url":"https://example.test/pull/10","updated_at":"{updated}","assignees":[],"pull_request":{{"url":"x"}}}},
            {{"number":11,"title":"Issue 十一","state":"open","html_url":"https://example.test/issues/11","updated_at":"{updated}","assignees":[]}}
        ]}}"#
    );
    let assigned_page = r#"{"items":[
        {"number":10,"title":"PR 十","state":"open","html_url":"https://example.test/pull/10","updated_at":"2026-08-14T00:00:00Z","assignees":[{"login":"wynxing"}],"pull_request":{"url":"x"}}
    ]}"#;
    let runner: GhRunner = Arc::new(move |args: &[&str]| {
        let joined = args.join(" ");
        if args.contains(&"--version") {
            return Ok("gh version 2.60.0".into());
        }
        if args.first() == Some(&"auth") {
            return Ok(String::new());
        }
        if args.contains(&".login") {
            return Ok("wynxing".into());
        }
        if joined.contains("search/issues") {
            if joined.contains("author:wynxing") {
                return Ok(mine_page.clone());
            }
            if joined.contains("assignee:wynxing") {
                return Ok(assigned_page.into());
            }
            return Ok(r#"{"items":[]}"#.into());
        }
        if joined.contains("repos/owner/repo/pulls/10") {
            return Ok(format!(
                r#"{{"draft":false,"updated_at":"{updated}","assignees":[{{"login":"wynxing"}}],"requested_reviewers":[{{"login":"alice"}}],"head":{{"sha":"abc123"}}}}"#
            ));
        }
        if joined.contains("commits/abc123/status") {
            return Ok(r#"{"state":"success","statuses":[{"state":"success"}]}"#.into());
        }
        Err(AppError::Github(format!("unexpected args: {joined}")))
    });
    let service = GithubService::new_with_runner(storage.clone(), false, runner);

    let snapshot = service.refresh_inner("owner/repo").unwrap();
    assert_eq!(snapshot.pull_requests.len(), 1);
    assert_eq!(snapshot.issues.len(), 1);
    let pr = &snapshot.pull_requests[0];
    // 同一 PR 命中两个过滤器时 matches 合并，而不是重复插入。
    assert!(pr.matches.contains(&"mine".to_string()));
    assert!(pr.matches.contains(&"assigned".to_string()));
    // enrich 补齐 reviewers / head SHA / checks 状态，并据此计算信号。
    assert_eq!(pr.reviewers, vec!["alice"]);
    assert_eq!(pr.head_sha.as_deref(), Some("abc123"));
    assert_eq!(pr.checks_state.as_deref(), Some("success"));
    assert!(pr.signals.contains(&ActionSignal::NeedsAction));
    // 快照已落盘，离线时可直接服务。
    let served = service.snapshot("owner/repo").unwrap();
    assert!(served.is_some());
}
