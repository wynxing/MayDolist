//! GitHub integration service: watch-list, snapshot cache, refresh pipeline
//! and linked-Todo sync. Split into focused submodules:
//!
//! - `gh_cli`: `gh` subprocess runner + raw API response shapes
//! - `watchlist`: repository subscriptions and per-item prefs (ignore / pin)
//! - `refresh`: snapshot serving and the remote refresh pipeline
//! - `sync`: batched GraphQL state checks for linked Todos
//! - `signals`: action signals, watch prefs and snapshot ordering

mod gh_cli;
mod refresh;
mod signals;
mod sync;
mod watchlist;

#[cfg(test)]
mod tests;

pub use sync::GithubSyncSummary;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::error::{AppError, AppResult};
use crate::models::GhAuthStatus;
use crate::storage::Storage;

use gh_cli::run_gh;

const FILTERS: &[&str] = &["mine", "mentioned", "assigned", "involved", "all-prs"];
const DEFAULT_FILTERS: &[&str] = &["mine", "mentioned", "assigned", "involved"];
const SIGNAL_FILTERS: &[&str] = &["needsAction", "needsReview", "ciFailed", "stale"];
/// Fallback used when `config.json` cannot be read for the stale threshold.
const DEFAULT_STALE_DAYS: u32 = 14;

/// Injection point for `gh` subprocess invocations. Production spawns the
/// real `gh` CLI; tests substitute a fixture-driven fake so refresh / sync
/// logic can be exercised without network access.
pub type GhRunner = Arc<dyn Fn(&[&str]) -> AppResult<String> + Send + Sync>;

pub struct GithubService {
    storage: Arc<Storage>,
    demo_mode: bool,
    refreshing: Mutex<HashSet<String>>,
    /// Cached auth status; only "authenticated" results are cached so that a
    /// later `gh auth login` is still picked up on the next check.
    auth_cache: Mutex<Option<GhAuthStatus>>,
    runner: GhRunner,
}

impl GithubService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self::new_with_mode(storage, false)
    }

    pub fn new_with_mode(storage: Arc<Storage>, demo_mode: bool) -> Self {
        Self::new_with_runner(storage, demo_mode, Arc::new(run_gh))
    }

    pub fn new_with_runner(storage: Arc<Storage>, demo_mode: bool, runner: GhRunner) -> Self {
        Self {
            storage,
            demo_mode,
            refreshing: Mutex::new(HashSet::new()),
            auth_cache: Mutex::new(None),
            runner,
        }
    }

    fn run(&self, args: &[&str]) -> AppResult<String> {
        (self.runner)(args)
    }

    fn gh<T: serde::de::DeserializeOwned>(&self, args: &[&str]) -> AppResult<T> {
        serde_json::from_str(&self.run(args)?)
            .map_err(|e| AppError::Github(format!("invalid gh response: {e}")))
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
        let version = self
            .run(&["--version"])
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
        if self.run(&["auth", "status"]).is_err() {
            return GhAuthStatus {
                state: "unauthenticated".into(),
                logged_in: false,
                user: None,
                version,
                message: "请运行 gh auth login".into(),
            };
        }
        match self.run(&["api", "user", "--jq", ".login"]) {
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

    fn stale_days(&self) -> u32 {
        self.storage
            .load_config()
            .map(|config| config.github_stale_days)
            .unwrap_or(DEFAULT_STALE_DAYS)
    }
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
