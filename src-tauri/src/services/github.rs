use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{
    refresh_stale, ActionSignal, GhAuthStatus, GhIgnoredItem, GhIssue, GhPullRequest, RepoSnapshot,
    RepoWatch,
};
use crate::storage::Storage;

const FILTERS: &[&str] = &["mine", "mentioned", "assigned", "involved", "all-prs"];
const DEFAULT_FILTERS: &[&str] = &["mine", "mentioned", "assigned", "involved"];
const SIGNAL_FILTERS: &[&str] = &["needsAction", "needsReview", "ciFailed", "stale"];
/// Fallback used when `config.json` cannot be read for the stale threshold.
const DEFAULT_STALE_DAYS: u32 = 14;

pub struct GithubService {
    storage: Arc<Storage>,
    demo_mode: bool,
    refreshing: Mutex<HashSet<String>>,
    /// Cached auth status; only "authenticated" results are cached so that a
    /// later `gh auth login` is still picked up on the next check.
    auth_cache: Mutex<Option<GhAuthStatus>>,
}
impl GithubService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self::new_with_mode(storage, false)
    }

    pub fn new_with_mode(storage: Arc<Storage>, demo_mode: bool) -> Self {
        Self {
            storage,
            demo_mode,
            refreshing: Mutex::new(HashSet::new()),
            auth_cache: Mutex::new(None),
        }
    }

    pub fn status(&self) -> GhAuthStatus {
        if self.demo_mode {
            return GhAuthStatus {
                state: "demo".into(),
                logged_in: true,
                user: Some("demo-user".into()),
                version: Some("Demo data · no network".into()),
                message: "Demo 模式：使用模拟 GitHub 数据".into(),
            };
        }
        if let Some(cached) = self.auth_cache.lock().ok().and_then(|v| v.clone()) {
            return cached;
        }
        let status = self.status_inner();
        if status.state == "authenticated" {
            if let Ok(mut cache) = self.auth_cache.lock() {
                *cache = Some(status.clone());
            }
        }
        status
    }

    fn status_inner(&self) -> GhAuthStatus {
        let version = run_gh(&["--version"])
            .ok()
            .and_then(|v| v.lines().next().map(str::to_string));
        if version.is_none() {
            return GhAuthStatus {
                state: "missing".into(),
                logged_in: false,
                user: None,
                version: None,
                message: "未安装 GitHub CLI".into(),
            };
        }
        if run_gh(&["auth", "status"]).is_err() {
            return GhAuthStatus {
                state: "unauthenticated".into(),
                logged_in: false,
                user: None,
                version,
                message: "请运行 gh auth login".into(),
            };
        }
        match run_gh(&["api", "user", "--jq", ".login"]) {
            Ok(user) => GhAuthStatus {
                state: "authenticated".into(),
                logged_in: true,
                user: Some(user.trim().into()),
                version,
                message: "GitHub CLI 已就绪".into(),
            },
            Err(err) => GhAuthStatus {
                state: "offline".into(),
                logged_in: true,
                user: None,
                version,
                message: err.to_string(),
            },
        }
    }

    pub fn watchlist(&self) -> AppResult<Vec<RepoWatch>> {
        let path = self.storage.data_dir().join("github/watchlist.json");
        Ok(self.storage.read_json(&path)?.unwrap_or_default())
    }
    fn save_watchlist(&self, list: &[RepoWatch]) -> AppResult<()> {
        self.storage
            .write_json(&self.storage.data_dir().join("github/watchlist.json"), list)
    }
    pub fn add_watch(&self, name: String) -> AppResult<Vec<RepoWatch>> {
        let full_name = normalize_repo(&name)?;
        if !self.demo_mode {
            run_gh(&["api", &format!("repos/{full_name}"), "--jq", ".full_name"])?;
        }
        let mut list = self.watchlist()?;
        if !list
            .iter()
            .any(|v| v.full_name.eq_ignore_ascii_case(&full_name))
        {
            list.push(RepoWatch {
                full_name,
                filters: DEFAULT_FILTERS.iter().map(|v| v.to_string()).collect(),
                collapsed: false,
                ignored: vec![],
                pinned: vec![],
                signal_filters: vec![],
            });
            self.save_watchlist(&list)?;
        }
        Ok(list)
    }
    pub fn remove_watch(&self, name: &str) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        list.retain(|v| !v.full_name.eq_ignore_ascii_case(name));
        self.save_watchlist(&list)?;
        Ok(list)
    }
    pub fn set_filters(&self, name: &str, filters: Vec<String>) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.filters = filters
            .into_iter()
            .filter(|v| FILTERS.contains(&v.as_str()))
            .collect();
        self.save_watchlist(&list)?;
        Ok(list)
    }

    /// Action-signal filters. An empty list means "no signal filtering",
    /// which preserves the legacy list behavior for existing users.
    pub fn set_signal_filters(
        &self,
        name: &str,
        filters: Vec<String>,
    ) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.signal_filters = filters
            .into_iter()
            .filter(|v| SIGNAL_FILTERS.contains(&v.as_str()))
            .collect();
        self.save_watchlist(&list)?;
        Ok(list)
    }

    pub fn set_collapsed(&self, name: &str, collapsed: bool) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.collapsed = collapsed;
        self.save_watchlist(&list)?;
        Ok(list)
    }

    pub fn ignore_item(&self, name: &str, number: u64, kind: String) -> AppResult<Vec<RepoWatch>> {
        if kind != "pr" && kind != "issue" {
            return Err(AppError::InvalidInput(
                "kind must be \"pr\" or \"issue\"".into(),
            ));
        }
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.pinned.retain(|&n| n != number);
        if !item
            .ignored
            .iter()
            .any(|v| v.number == number && v.kind == kind)
        {
            item.ignored.push(GhIgnoredItem {
                number,
                kind: kind.clone(),
            });
        }
        let ignored = item.ignored.clone();
        let pinned = item.pinned.clone();
        self.save_watchlist(&list)?;
        if let Some(mut snap) = self.snapshot(name)? {
            apply_watch_prefs(&mut snap, &ignored, &pinned);
            self.storage
                .write_json(&cache_path(&self.storage, name), &snap)?;
        }
        Ok(list)
    }

    pub fn pin_item(&self, name: &str, number: u64) -> AppResult<RepoSnapshot> {
        if self.demo_mode {
            return self
                .snapshot(name)?
                .ok_or_else(|| AppError::Github("Demo 仓库没有模拟快照".into()));
        }
        let full_name = normalize_repo(name)?;
        let fetched = self.fetch_issue_or_pr(&full_name, number)?;
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == full_name)
            .ok_or_else(|| AppError::NotFound(full_name.clone()))?;
        item.ignored.retain(|v| v.number != number);
        if !item.pinned.contains(&number) {
            item.pinned.push(number);
        }
        let ignored = item.ignored.clone();
        let pinned = item.pinned.clone();
        self.save_watchlist(&list)?;

        let user = self
            .status()
            .user
            .ok_or_else(|| AppError::Github("GitHub CLI 未登录或离线".into()))?;
        let now = now_rfc3339();
        let mut snapshot = self.snapshot(&full_name)?.unwrap_or_else(|| RepoSnapshot {
            schema_version: 2,
            repo: full_name.clone(),
            fetched_at: now.clone(),
            last_success_at: None,
            last_error: None,
            issues: vec![],
            pull_requests: vec![],
            signals_computed_at: Some(now.clone()),
        });
        merge_pinned_item(&mut snapshot, fetched);
        apply_watch_prefs(&mut snapshot, &ignored, &pinned);
        apply_signals(&mut snapshot, &user, self.stale_days(), &now);
        sort_snapshot(&mut snapshot);
        self.storage
            .write_json(&cache_path(&self.storage, &full_name), &snapshot)?;
        Ok(snapshot)
    }

    pub fn unpin_item(&self, name: &str, number: u64) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.pinned.retain(|&n| n != number);
        let ignored = item.ignored.clone();
        let pinned = item.pinned.clone();
        self.save_watchlist(&list)?;
        if let Some(mut snap) = self.snapshot(name)? {
            strip_pin_marker(&mut snap, number);
            apply_watch_prefs(&mut snap, &ignored, &pinned);
            sort_snapshot(&mut snap);
            self.storage
                .write_json(&cache_path(&self.storage, name), &snap)?;
        }
        Ok(list)
    }

    pub fn snapshot(&self, repo: &str) -> AppResult<Option<RepoSnapshot>> {
        let mut snapshot: Option<RepoSnapshot> =
            self.storage.read_json(&cache_path(&self.storage, repo))?;
        if let Some(snap) = snapshot.as_mut() {
            let stale_days = self.stale_days();
            let now = now_rfc3339();
            for pr in &mut snap.pull_requests {
                refresh_stale(&mut pr.signals, &pr.updated_at, stale_days, &now);
            }
            for issue in &mut snap.issues {
                refresh_stale(&mut issue.signals, &issue.updated_at, stale_days, &now);
            }
        }
        Ok(snapshot)
    }

    pub fn refresh(&self, repo: &str) -> AppResult<RepoSnapshot> {
        if self.demo_mode {
            return self
                .snapshot(repo)?
                .ok_or_else(|| AppError::Github("Demo 仓库没有模拟快照".into()));
        }
        normalize_repo(repo)?;
        {
            let mut lock = self
                .refreshing
                .lock()
                .map_err(|_| AppError::Internal("refresh lock poisoned".into()))?;
            if !lock.insert(repo.into()) {
                return self
                    .snapshot(repo)?
                    .ok_or_else(|| AppError::Github("refresh already running".into()));
            }
        }
        let result = self.refresh_inner(repo);
        self.refreshing.lock().ok().map(|mut v| v.remove(repo));
        match result {
            Ok(snapshot) => Ok(snapshot),
            Err(err) => {
                if let Some(mut cached) = self.snapshot(repo)? {
                    cached.last_error = Some(err.to_string());
                    self.storage
                        .write_json(&cache_path(&self.storage, repo), &cached)?;
                    Ok(cached)
                } else {
                    Err(err)
                }
            }
        }
    }
    pub fn refresh_all(&self) -> AppResult<Vec<RepoSnapshot>> {
        let watches = self.watchlist()?;
        let mut out = vec![];
        // Serial per-repo refresh bounds the number of gh subprocesses and
        // smooths API rate-limit pressure. The `refreshing` guard below
        // already prevents two refreshes of the same repo from running
        // concurrently, so a repeated click / background timer never starts a
        // duplicate refresh. A failing repo keeps its old cache (its snapshot
        // carries `lastError`) and never clears other repos.
        for watch in watches {
            if let Ok(snapshot) = self.refresh(&watch.full_name) {
                out.push(snapshot);
            }
        }
        Ok(out)
    }

    fn refresh_inner(&self, repo: &str) -> AppResult<RepoSnapshot> {
        let user = self
            .status()
            .user
            .ok_or_else(|| AppError::Github("GitHub CLI 未登录或离线".into()))?;
        let watches = self.watchlist()?;
        let watch = watches
            .iter()
            .find(|v| v.full_name == repo)
            .cloned()
            .unwrap_or_else(|| RepoWatch {
                full_name: repo.into(),
                filters: DEFAULT_FILTERS.iter().map(|v| v.to_string()).collect(),
                collapsed: false,
                ignored: vec![],
                pinned: vec![],
                signal_filters: vec![],
            });
        let filters = watch.filters.clone();
        let old = self.snapshot(repo)?;
        let mut issues: HashMap<(u64, String), GhIssue> = HashMap::new();
        let mut prs: HashMap<u64, GhPullRequest> = HashMap::new();

        if filters.iter().any(|v| v == "all-prs") {
            let all_prs: Vec<ApiItem> = gh_json(&[
                "api",
                "--paginate",
                &format!("repos/{repo}/pulls?state=open&per_page=100"),
            ])?;
            for row in all_prs {
                prs.insert(row.number, row.into_pr(vec!["all-prs".into()]));
            }
        }

        for filter in filters.iter().filter(|v| v.as_str() != "all-prs") {
            let qualifier = match filter.as_str() {
                "mine" => format!("author:{user}"),
                "mentioned" => format!("mentions:{user}"),
                "assigned" => format!("assignee:{user}"),
                _ => format!("involves:{user}"),
            };
            let query = format!("repo:{repo} state:open {qualifier}");
            // GitHub search caps at 100 results/page and 1000 total; page
            // through manually because `gh api --paginate` cannot merge the
            // `{items: [...]}` envelope that search/issues returns.
            let mut page = 1u32;
            loop {
                let result: SearchResult = gh_json(&[
                    "api",
                    "--method",
                    "GET",
                    "search/issues",
                    "-f",
                    &format!("q={query}"),
                    "-f",
                    "per_page=100",
                    "-f",
                    &format!("page={page}"),
                ])?;
                let items = result.items;
                let is_last_page = items.len() < 100 || page >= 10;
                for row in items {
                    if row.pull_request.is_some() {
                        prs.entry(row.number)
                            .and_modify(|v| {
                                if !v.matches.contains(filter) {
                                    v.matches.push(filter.clone())
                                }
                            })
                            .or_insert_with(|| row.clone().into_pr(vec![filter.clone()]));
                    } else {
                        issues
                            .entry((row.number, row.html_url.clone()))
                            .and_modify(|v| {
                                if !v.matches.contains(filter) {
                                    v.matches.push(filter.clone())
                                }
                            })
                            .or_insert_with(|| row.into_issue(vec![filter.clone()]));
                    }
                }
                if is_last_page {
                    break;
                }
                page += 1;
            }
        }

        for &number in &watch.pinned {
            let already_pr = prs.contains_key(&number);
            let already_issue = issues.keys().any(|(n, _)| *n == number);
            if already_pr || already_issue {
                if let Some(pr) = prs.get_mut(&number) {
                    if !pr.matches.iter().any(|m| m == "pinned") {
                        pr.matches.push("pinned".into());
                    }
                }
                for ((n, _), issue) in issues.iter_mut() {
                    if *n == number && !issue.matches.iter().any(|m| m == "pinned") {
                        issue.matches.push("pinned".into());
                    }
                }
                continue;
            }
            match self.fetch_issue_or_pr(repo, number) {
                Ok(FetchedItem::Pr(mut pr)) => {
                    if !pr.matches.iter().any(|m| m == "pinned") {
                        pr.matches.push("pinned".into());
                    }
                    prs.insert(number, pr);
                }
                Ok(FetchedItem::Issue(mut issue)) => {
                    if !issue.matches.iter().any(|m| m == "pinned") {
                        issue.matches.push("pinned".into());
                    }
                    issues.insert((number, issue.url.clone()), issue);
                }
                Err(_) => {
                    // Keep refresh succeeding if a pinned item vanished.
                }
            }
        }

        let now = now_rfc3339();
        // Enrich open PRs with review requests and check state. When a PR's
        // `updated_at` is unchanged since the last snapshot, the cached raw
        // fields are reused and no extra GitHub API call is made (rate-limit
        // friendly); per-PR failures degrade to the fields we already have.
        for pr in prs.values_mut() {
            if pr.state != "open" {
                continue;
            }
            if let Some(cached) = old
                .as_ref()
                .and_then(|snap| snap.pull_requests.iter().find(|p| p.number == pr.number))
            {
                if cached.updated_at == pr.updated_at {
                    pr.assignees = cached.assignees.clone();
                    pr.reviewers = cached.reviewers.clone();
                    pr.head_sha = cached.head_sha.clone();
                    pr.checks_state = cached.checks_state.clone();
                    continue;
                }
            }
            let _ = self.enrich_pull(repo, pr);
        }

        let mut snapshot = RepoSnapshot {
            schema_version: 2,
            repo: repo.into(),
            fetched_at: now.clone(),
            last_success_at: Some(now.clone()),
            last_error: None,
            issues: issues.into_values().collect(),
            pull_requests: prs.into_values().collect(),
            signals_computed_at: Some(now.clone()),
        };
        apply_watch_prefs(
            &mut snapshot,
            watch.ignored.as_slice(),
            watch.pinned.as_slice(),
        );
        apply_signals(&mut snapshot, &user, self.stale_days(), &now);
        sort_snapshot(&mut snapshot);
        self.storage
            .write_json(&cache_path(&self.storage, repo), &snapshot)?;
        Ok(snapshot)
    }

    fn fetch_issue_or_pr(&self, repo: &str, number: u64) -> AppResult<FetchedItem> {
        let path = format!("repos/{repo}/issues/{number}");
        let row: ApiItem = gh_json(&["api", &path]).map_err(|err| {
            let msg = err.to_string();
            if msg.contains("404") || msg.contains("Not Found") {
                AppError::Github(format!("#{number} 不存在"))
            } else {
                err
            }
        })?;
        if row.pull_request.is_some() {
            Ok(FetchedItem::Pr(row.into_pr(vec!["pinned".into()])))
        } else {
            Ok(FetchedItem::Issue(row.into_issue(vec!["pinned".into()])))
        }
    }

    /// Fetch the PR detail (requested reviewers, head SHA, assignees) and the
    /// check state for an open PR. Missing fields degrade to `None` / empty,
    /// and an API failure for one PR never fails the whole repo refresh.
    fn enrich_pull(&self, repo: &str, pr: &mut GhPullRequest) -> AppResult<()> {
        let detail: PullDetail = gh_json(&["api", &format!("repos/{repo}/pulls/{}", pr.number)])?;
        pr.draft = detail.draft;
        pr.updated_at = detail.updated_at;
        pr.assignees = detail.assignees.iter().map(|u| u.login.clone()).collect();
        pr.reviewers = detail
            .requested_reviewers
            .iter()
            .map(|u| u.login.clone())
            .collect();
        let sha = detail.head.sha;
        pr.head_sha = Some(sha.clone());
        let (_, state) = fetch_checks_state(repo, &sha);
        pr.checks_state = state;
        Ok(())
    }

    fn stale_days(&self) -> u32 {
        self.storage
            .load_config()
            .map(|config| config.github_stale_days)
            .unwrap_or(DEFAULT_STALE_DAYS)
    }
}

enum FetchedItem {
    Pr(GhPullRequest),
    Issue(GhIssue),
}

fn merge_pinned_item(snapshot: &mut RepoSnapshot, item: FetchedItem) {
    match item {
        FetchedItem::Pr(pr) => {
            snapshot.issues.retain(|v| v.number != pr.number);
            if let Some(existing) = snapshot
                .pull_requests
                .iter_mut()
                .find(|v| v.number == pr.number)
            {
                if !existing.matches.iter().any(|m| m == "pinned") {
                    existing.matches.push("pinned".into());
                }
            } else {
                snapshot.pull_requests.push(pr);
            }
        }
        FetchedItem::Issue(issue) => {
            snapshot.pull_requests.retain(|v| v.number != issue.number);
            if let Some(existing) = snapshot
                .issues
                .iter_mut()
                .find(|v| v.number == issue.number)
            {
                if !existing.matches.iter().any(|m| m == "pinned") {
                    existing.matches.push("pinned".into());
                }
            } else {
                snapshot.issues.push(issue);
            }
        }
    }
}

fn strip_pin_marker(snapshot: &mut RepoSnapshot, number: u64) {
    if let Some(pr) = snapshot
        .pull_requests
        .iter_mut()
        .find(|v| v.number == number)
    {
        pr.matches.retain(|m| m != "pinned");
        if pr.matches.is_empty() {
            snapshot.pull_requests.retain(|v| v.number != number);
        }
    }
    if let Some(issue) = snapshot.issues.iter_mut().find(|v| v.number == number) {
        issue.matches.retain(|m| m != "pinned");
        if issue.matches.is_empty() {
            snapshot.issues.retain(|v| v.number != number);
        }
    }
}

/// Recompute the stable action signals for every open item from the parsed
/// response fields. Closed / merged items keep an empty signal list so the
/// existing display rules (dimmed, no action badges) stay intact.
fn apply_signals(snapshot: &mut RepoSnapshot, user: &str, stale_days: u32, now: &str) {
    for pr in &mut snapshot.pull_requests {
        pr.signals = if pr.state == "open" {
            compute_signals(SignalInputs {
                is_pr: true,
                draft: pr.draft,
                assignees: pr.assignees.clone(),
                reviewers: pr.reviewers.clone(),
                checks_state: pr.checks_state.clone(),
                matches: pr.matches.clone(),
                user: user.into(),
                stale_days,
                updated_at: pr.updated_at.clone(),
                now: now.into(),
            })
        } else {
            Vec::new()
        };
    }
    for issue in &mut snapshot.issues {
        issue.signals = if issue.state == "open" {
            compute_signals(SignalInputs {
                is_pr: false,
                draft: false,
                assignees: issue.assignees.clone(),
                reviewers: vec![],
                checks_state: None,
                matches: issue.matches.clone(),
                user: user.into(),
                stale_days,
                updated_at: issue.updated_at.clone(),
                now: now.into(),
            })
        } else {
            Vec::new()
        };
    }
}

/// Map parsed GitHub response fields onto the stable signal set. The UI only
/// ever consumes `ActionSignal` values, never raw GitHub strings.
struct SignalInputs {
    is_pr: bool,
    draft: bool,
    assignees: Vec<String>,
    reviewers: Vec<String>,
    checks_state: Option<String>,
    matches: Vec<String>,
    user: String,
    stale_days: u32,
    updated_at: String,
    now: String,
}

fn compute_signals(input: SignalInputs) -> Vec<ActionSignal> {
    let mut signals = Vec::new();
    let needs_action = input
        .matches
        .iter()
        .any(|m| matches!(m.as_str(), "assigned" | "mentioned" | "involved" | "pinned"))
        || input.assignees.contains(&input.user);
    if needs_action {
        signals.push(ActionSignal::NeedsAction);
    }
    if input.is_pr && input.reviewers.contains(&input.user) {
        signals.push(ActionSignal::NeedsReview);
    }
    if input.is_pr
        && input
            .checks_state
            .as_deref()
            .map(|state| state == "failure" || state == "error")
            .unwrap_or(false)
    {
        signals.push(ActionSignal::CiFailed);
    }
    if crate::models::github::is_stale(&input.updated_at, input.stale_days, &input.now) {
        signals.push(ActionSignal::Stale);
    }
    if input.is_pr && input.draft {
        signals.push(ActionSignal::Draft);
    }
    signals
}

/// Best-effort check status for a commit. Classic commit statuses are checked
/// first; when a repo only uses check runs (e.g. GitHub Actions), the
/// `check-runs` endpoint is queried instead. Any API failure / missing field
/// degrades to `(false, None)` so one PR never fails the whole repo refresh.
fn fetch_checks_state(repo: &str, sha: &str) -> (bool, Option<String>) {
    if let Ok(status) =
        gh_json::<CombinedStatus>(&["api", &format!("repos/{repo}/commits/{sha}/status")])
    {
        let state = status.state.to_ascii_lowercase();
        if !status.statuses.is_empty() {
            let failed = state == "failure" || state == "error";
            return (failed, Some(state));
        }
    }
    match gh_json::<CheckRuns>(&["api", &format!("repos/{repo}/commits/{sha}/check-runs")]) {
        Ok(runs) => {
            if runs.check_runs.is_empty() {
                return (false, None);
            }
            let any_failed = runs.check_runs.iter().any(|run| {
                matches!(
                    run.conclusion.as_deref(),
                    Some("failure" | "timed_out" | "action_required")
                )
            });
            let any_pending = runs.check_runs.iter().any(|run| {
                run.conclusion.is_none() || run.conclusion.as_deref() == Some("pending")
            });
            let state = if any_failed {
                "failure"
            } else if any_pending {
                "pending"
            } else {
                "success"
            };
            (any_failed, Some(state.into()))
        }
        Err(_) => (false, None),
    }
}

fn apply_watch_prefs(snapshot: &mut RepoSnapshot, ignored: &[GhIgnoredItem], pinned: &[u64]) {
    snapshot.pull_requests.retain(|pr| {
        !ignored
            .iter()
            .any(|v| v.number == pr.number && v.kind == "pr")
    });
    snapshot.issues.retain(|issue| {
        !ignored
            .iter()
            .any(|v| v.number == issue.number && v.kind == "issue")
    });
    for pr in &mut snapshot.pull_requests {
        if pinned.contains(&pr.number) && !pr.matches.iter().any(|m| m == "pinned") {
            pr.matches.push("pinned".into());
        }
    }
    for issue in &mut snapshot.issues {
        if pinned.contains(&issue.number) && !issue.matches.iter().any(|m| m == "pinned") {
            issue.matches.push("pinned".into());
        }
    }
}

fn sort_snapshot(snapshot: &mut RepoSnapshot) {
    let pinned = |matches: &[String]| matches.iter().any(|m| m == "pinned");
    let actionable = |signals: &[ActionSignal]| {
        signals
            .iter()
            .filter(|signal| signal.is_actionable())
            .count()
    };
    // Pinned first, then items with more actionable signals, then by update
    // recency, then by number descending (stable and explainable).
    snapshot.pull_requests.sort_by(|a, b| {
        pinned(&b.matches)
            .cmp(&pinned(&a.matches))
            .then_with(|| actionable(&b.signals).cmp(&actionable(&a.signals)))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| b.number.cmp(&a.number))
    });
    snapshot.issues.sort_by(|a, b| {
        pinned(&b.matches)
            .cmp(&pinned(&a.matches))
            .then_with(|| actionable(&b.signals).cmp(&actionable(&a.signals)))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| b.number.cmp(&a.number))
    });
}

#[derive(Deserialize, Clone)]
struct SearchResult {
    items: Vec<ApiItem>,
}

#[derive(Deserialize, Clone)]
struct GhUser {
    login: String,
}

#[derive(Deserialize, Clone)]
struct ApiItem {
    number: u64,
    title: String,
    state: String,
    html_url: String,
    updated_at: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assignees: Vec<GhUser>,
    pull_request: Option<serde_json::Value>,
}
impl ApiItem {
    fn into_pr(self, matches: Vec<String>) -> GhPullRequest {
        GhPullRequest {
            number: self.number,
            title: self.title,
            state: self.state,
            draft: self.draft,
            url: self.html_url,
            updated_at: self.updated_at,
            matches,
            assignees: self.assignees.iter().map(|u| u.login.clone()).collect(),
            reviewers: vec![],
            head_sha: None,
            checks_state: None,
            signals: vec![],
        }
    }
    fn into_issue(self, matches: Vec<String>) -> GhIssue {
        GhIssue {
            number: self.number,
            title: self.title,
            state: self.state,
            url: self.html_url,
            updated_at: self.updated_at,
            kind: "issue".into(),
            matches,
            assignees: self.assignees.iter().map(|u| u.login.clone()).collect(),
            signals: vec![],
        }
    }
}

/// Full PR response (`repos/{owner}/{repo}/pulls/{number}`), used for the
/// fields the search / list endpoints do not return: requested reviewers and
/// the head commit SHA for check lookups.
#[derive(Deserialize, Clone)]
struct PullDetail {
    #[serde(default)]
    draft: bool,
    updated_at: String,
    #[serde(default)]
    assignees: Vec<GhUser>,
    #[serde(default)]
    requested_reviewers: Vec<GhUser>,
    head: PullHead,
}

#[derive(Deserialize, Clone)]
struct PullHead {
    sha: String,
}

/// `GET /repos/{owner}/{repo}/commits/{sha}/status` (classic commit statuses).
#[derive(Deserialize)]
struct CombinedStatus {
    state: String,
    #[serde(default)]
    statuses: Vec<serde_json::Value>,
}

/// `GET /repos/{owner}/{repo}/commits/{sha}/check-runs`.
#[derive(Deserialize)]
struct CheckRuns {
    #[serde(default)]
    check_runs: Vec<CheckRun>,
}

#[derive(Deserialize)]
struct CheckRun {
    #[serde(default)]
    conclusion: Option<String>,
}
fn gh_json<T: serde::de::DeserializeOwned>(args: &[&str]) -> AppResult<T> {
    serde_json::from_str(&run_gh(args)?)
        .map_err(|e| AppError::Github(format!("invalid gh response: {e}")))
}
fn run_gh(args: &[&str]) -> AppResult<String> {
    let mut command = Command::new("gh");
    command
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: keep the console hidden in release builds.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::Github(e.to_string()))?;
    let mut stdout = child.stdout.take().expect("stdout must be piped");
    let mut stderr = child.stderr.take().expect("stderr must be piped");
    let out_thread = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        buf
    });
    let err_thread = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError::Github("gh timed out after 30s".into()));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(AppError::Github(err.to_string())),
        }
    };
    let stdout = out_thread
        .join()
        .map_err(|_| AppError::Github("gh output thread panicked".into()))?;
    let stderr = err_thread
        .join()
        .map_err(|_| AppError::Github("gh stderr thread panicked".into()))?;
    if !status.success() {
        return Err(AppError::Github(stderr.trim().to_string()));
    }
    Ok(stdout)
}
fn normalize_repo(value: &str) -> AppResult<String> {
    let value = value.trim().trim_matches('/');
    let mut p = value.split('/');
    let (Some(owner), Some(repo), None) = (p.next(), p.next(), p.next()) else {
        return Err(AppError::InvalidInput(
            "repository must be owner/repo".into(),
        ));
    };
    if owner.is_empty()
        || repo.is_empty()
        || !owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        || !repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(AppError::InvalidInput("invalid repository name".into()));
    }
    Ok(format!("{owner}/{repo}"))
}
fn cache_path(storage: &Storage, repo: &str) -> std::path::PathBuf {
    storage
        .data_dir()
        .join("github/cache")
        .join(format!("{}.json", repo.replace('/', "_")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ActionSignal;

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

    fn pr(
        number: u64,
        updated_at: &str,
        matches: &[&str],
        signals: &[ActionSignal],
    ) -> GhPullRequest {
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
        let dir =
            std::env::temp_dir().join(format!("maydolist-gh-signal-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
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
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn snapshot_serves_fresh_stale_signal() {
        let dir = std::env::temp_dir().join(format!("maydolist-gh-stale-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
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
        std::fs::remove_dir_all(&dir).ok();
    }
}
