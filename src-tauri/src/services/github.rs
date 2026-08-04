use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{GhAuthStatus, GhIssue, GhPullRequest, RepoSnapshot, RepoWatch};
use crate::storage::Storage;

const FILTERS: &[&str] = &["mine", "mentioned", "assigned", "involved"];

pub struct GithubService {
    storage: Arc<Storage>,
    refreshing: Mutex<HashSet<String>>,
}
impl GithubService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            refreshing: Mutex::new(HashSet::new()),
        }
    }

    pub fn status(&self) -> GhAuthStatus {
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
                filters: FILTERS.iter().map(|v| v.to_string()).collect(),
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
        let mut out = vec![];
        for watch in self.watchlist()? {
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
        let filters = watches
            .iter()
            .find(|v| v.full_name == repo)
            .map(|v| v.filters.clone())
            .unwrap_or_else(|| FILTERS.iter().map(|v| v.to_string()).collect());
        let mut issues: HashMap<(u64, String), GhIssue> = HashMap::new();
        let mut prs: HashMap<u64, GhPullRequest> = HashMap::new();
        let all_prs: Vec<ApiItem> = gh_json(&[
            "api",
            "--paginate",
            &format!("repos/{repo}/pulls?state=open&per_page=100"),
        ])?;
        for row in all_prs {
            prs.insert(row.number, row.into_pr(vec!["open".into()]));
        }
        for filter in &filters {
            let qualifier = match filter.as_str() {
                "mine" => format!("author:{user}"),
                "mentioned" => format!("mentions:{user}"),
                "assigned" => format!("assignee:{user}"),
                _ => format!("involves:{user}"),
            };
            let query = format!("repo:{repo} state:open {qualifier}");
            let result: SearchResult = gh_json(&[
                "api",
                "--method",
                "GET",
                "search/issues",
                "-f",
                &format!("q={query}"),
                "-f",
                "per_page=100",
            ])?;
            for row in result.items {
                if row.pull_request.is_some() {
                    prs.entry(row.number)
                        .and_modify(|v| {
                            if !v.matches.contains(filter) {
                                v.matches.push(filter.clone())
                            }
                        })
                        .or_insert_with(|| row.into_pr(vec![filter.clone()]));
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
        }
        let now = now_rfc3339();
        let snapshot = RepoSnapshot {
            schema_version: 1,
            repo: repo.into(),
            fetched_at: now.clone(),
            last_success_at: Some(now),
            last_error: None,
            issues: issues.into_values().collect(),
            pull_requests: prs.into_values().collect(),
        };
        self.storage
            .write_json(&cache_path(&self.storage, repo), &snapshot)?;
        Ok(snapshot)
    }
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
    let output = Command::new("gh")
        .args(args)
        .output()
        .map_err(|e| AppError::Github(e.to_string()))?;
    if !output.status.success() {
        return Err(AppError::Github(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
