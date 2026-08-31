//! Snapshot serving and the remote refresh pipeline (search filters, pinned
//! backfill, PR enrichment and check state).

use std::collections::HashMap;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{refresh_stale, GhIssue, GhPullRequest, RepoSnapshot, RepoWatch};

use super::gh_cli::{ApiItem, CheckRuns, CombinedStatus, PullDetail, SearchResult};
use super::signals::{apply_signals, apply_watch_prefs, sort_snapshot};
use super::{cache_path, GithubService, DEFAULT_FILTERS};

pub(super) enum FetchedItem {
    Pr(GhPullRequest),
    Issue(GhIssue),
}

impl GithubService {
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
        super::normalize_repo(repo)?;
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

    pub(super) fn refresh_inner(&self, repo: &str) -> AppResult<RepoSnapshot> {
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
            let all_prs: Vec<ApiItem> = self.gh(&[
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
                let result: SearchResult = self.gh(&[
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

    pub(super) fn fetch_issue_or_pr(&self, repo: &str, number: u64) -> AppResult<FetchedItem> {
        let path = format!("repos/{repo}/issues/{number}");
        let row: ApiItem = self.gh(&["api", &path]).map_err(|err| {
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
        let detail: PullDetail = self.gh(&["api", &format!("repos/{repo}/pulls/{}", pr.number)])?;
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
        let (_, state) = self.fetch_checks_state(repo, &sha);
        pr.checks_state = state;
        Ok(())
    }

    /// Best-effort check status for a commit. Classic commit statuses are checked
    /// first; when a repo only uses check runs (e.g. GitHub Actions), the
    /// `check-runs` endpoint is queried instead. Any API failure / missing field
    /// degrades to `(false, None)` so one PR never fails the whole repo refresh.
    fn fetch_checks_state(&self, repo: &str, sha: &str) -> (bool, Option<String>) {
        if let Ok(status) =
            self.gh::<CombinedStatus>(&["api", &format!("repos/{repo}/commits/{sha}/status")])
        {
            let state = status.state.to_ascii_lowercase();
            if !status.statuses.is_empty() {
                let failed = state == "failure" || state == "error";
                return (failed, Some(state));
            }
        }
        match self.gh::<CheckRuns>(&["api", &format!("repos/{repo}/commits/{sha}/check-runs")]) {
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
}
