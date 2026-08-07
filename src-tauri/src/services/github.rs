use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{GhAuthStatus, GhIgnoredItem, GhIssue, GhPullRequest, RepoSnapshot, RepoWatch};
use crate::storage::Storage;

const FILTERS: &[&str] = &["mine", "mentioned", "assigned", "involved", "all-prs"];
const DEFAULT_FILTERS: &[&str] = &["mine", "mentioned", "assigned", "involved"];

pub struct GithubService {
    storage: Arc<Storage>,
    refreshing: Mutex<HashSet<String>>,
    /// Cached auth status; only "authenticated" results are cached so that a
    /// later `gh auth login` is still picked up on the next check.
    auth_cache: Mutex<Option<GhAuthStatus>>,
}
impl GithubService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            refreshing: Mutex::new(HashSet::new()),
            auth_cache: Mutex::new(None),
        }
    }

    pub fn status(&self) -> GhAuthStatus {
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
        run_gh(&["api", &format!("repos/{full_name}"), "--jq", ".full_name"])?;
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

        let mut snapshot = self.snapshot(&full_name)?.unwrap_or_else(|| RepoSnapshot {
            schema_version: 1,
            repo: full_name.clone(),
            fetched_at: now_rfc3339(),
            last_success_at: None,
            last_error: None,
            issues: vec![],
            pull_requests: vec![],
        });
        merge_pinned_item(&mut snapshot, fetched);
        apply_watch_prefs(&mut snapshot, &ignored, &pinned);
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
        self.storage.read_json(&cache_path(&self.storage, repo))
    }

    pub fn refresh(&self, repo: &str) -> AppResult<RepoSnapshot> {
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
        std::thread::scope(|scope| {
            let handles: Vec<_> = watches
                .iter()
                .map(|watch| {
                    let repo = watch.full_name.clone();
                    scope.spawn(move || self.refresh(&repo))
                })
                .collect();
            for handle in handles {
                if let Ok(Ok(snapshot)) = handle.join() {
                    out.push(snapshot);
                }
            }
        });
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
            });
        let filters = watch.filters.clone();
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
        let mut snapshot = RepoSnapshot {
            schema_version: 1,
            repo: repo.into(),
            fetched_at: now.clone(),
            last_success_at: Some(now),
            last_error: None,
            issues: issues.into_values().collect(),
            pull_requests: prs.into_values().collect(),
        };
        apply_watch_prefs(
            &mut snapshot,
            watch.ignored.as_slice(),
            watch.pinned.as_slice(),
        );
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
    let pinned_first = |matches: &[String], number: u64| -> (bool, u64) {
        (!matches.iter().any(|m| m == "pinned"), u64::MAX - number)
    };
    snapshot
        .pull_requests
        .sort_by_key(|pr| pinned_first(&pr.matches, pr.number));
    snapshot
        .issues
        .sort_by_key(|issue| pinned_first(&issue.matches, issue.number));
}

#[derive(Deserialize, Clone)]
struct SearchResult {
    items: Vec<ApiItem>,
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
        }
    }
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
