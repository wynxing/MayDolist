use std::sync::Mutex;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{GhAuthStatus, GhIssue, GhPullRequest, RepoSnapshot, RepoWatch};

pub trait GithubService: Send + Sync {
    fn auth_status(&self) -> GhAuthStatus;
    fn watchlist(&self) -> Vec<RepoWatch>;
    fn add_watch(&self, full_name: String) -> AppResult<Vec<RepoWatch>>;
    fn remove_watch(&self, full_name: String) -> AppResult<Vec<RepoWatch>>;
    fn refresh(&self) -> Vec<RepoSnapshot>;
}

/// In-memory mock: no real `gh` subprocess calls in the skeleton phase.
pub struct MockGithubService {
    watchlist: Mutex<Vec<RepoWatch>>,
}

impl MockGithubService {
    pub fn seeded() -> Self {
        Self {
            watchlist: Mutex::new(vec![
                RepoWatch { full_name: "tauri-apps/tauri".into() },
                RepoWatch { full_name: "vuejs/core".into() },
            ]),
        }
    }

    fn snapshot_for(&self, repo: &str) -> RepoSnapshot {
        let fetched_at = now_rfc3339();
        match repo {
            "tauri-apps/tauri" => RepoSnapshot {
                repo: repo.into(),
                fetched_at,
                issues: vec![
                    GhIssue {
                        number: 13001,
                        title: "Improve window effects on Windows 10".into(),
                        state: "open".into(),
                        url: format!("https://github.com/{repo}/issues/13001"),
                        updated_at: "2026-08-03T08:00:00Z".into(),
                    },
                    GhIssue {
                        number: 12988,
                        title: "Document CSP defaults".into(),
                        state: "closed".into(),
                        url: format!("https://github.com/{repo}/issues/12988"),
                        updated_at: "2026-08-02T15:30:00Z".into(),
                    },
                ],
                pull_requests: vec![GhPullRequest {
                    number: 13010,
                    title: "feat: add draft PR badge".into(),
                    state: "open".into(),
                    draft: true,
                    url: format!("https://github.com/{repo}/pull/13010"),
                    updated_at: "2026-08-04T02:00:00Z".into(),
                }],
            },
            "vuejs/core" => RepoSnapshot {
                repo: repo.into(),
                fetched_at,
                issues: vec![GhIssue {
                    number: 12500,
                    title: "RFC: compiler inline mode".into(),
                    state: "open".into(),
                    url: format!("https://github.com/{repo}/issues/12500"),
                    updated_at: "2026-08-01T12:00:00Z".into(),
                }],
                pull_requests: vec![],
            },
            _ => RepoSnapshot {
                repo: repo.into(),
                fetched_at,
                issues: vec![],
                pull_requests: vec![],
            },
        }
    }
}

impl GithubService for MockGithubService {
    fn auth_status(&self) -> GhAuthStatus {
        GhAuthStatus {
            logged_in: true,
            user: Some("wynn".into()),
            message: "mock: gh auth status (真实调用在 v0.2 接入)".into(),
        }
    }

    fn watchlist(&self) -> Vec<RepoWatch> {
        self.watchlist.lock().unwrap().clone()
    }

    fn add_watch(&self, full_name: String) -> AppResult<Vec<RepoWatch>> {
        let full_name = full_name.trim().to_string();
        if !full_name.contains('/') {
            return Err(AppError::InvalidInput(
                "仓库格式应为 owner/repo".into(),
            ));
        }
        let mut watchlist = self.watchlist.lock().unwrap();
        if !watchlist.iter().any(|watch| watch.full_name == full_name) {
            watchlist.push(RepoWatch { full_name });
        }
        Ok(watchlist.clone())
    }

    fn remove_watch(&self, full_name: String) -> AppResult<Vec<RepoWatch>> {
        let mut watchlist = self.watchlist.lock().unwrap();
        watchlist.retain(|watch| watch.full_name != full_name);
        Ok(watchlist.clone())
    }

    fn refresh(&self) -> Vec<RepoSnapshot> {
        let watchlist = self.watchlist.lock().unwrap().clone();
        watchlist.iter().map(|watch| self.snapshot_for(&watch.full_name)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_mock_flow() {
        let service = MockGithubService::seeded();
        assert!(service.auth_status().logged_in);
        assert_eq!(service.watchlist().len(), 2);

        assert!(matches!(
            service.add_watch("no-slash".into()),
            Err(AppError::InvalidInput(_))
        ));
        let watchlist = service.add_watch("microsoft/vscode".into()).unwrap();
        assert_eq!(watchlist.len(), 3);

        let snapshots = service.refresh();
        assert_eq!(snapshots.len(), 3);
        assert!(snapshots.iter().all(|s| !s.fetched_at.is_empty()));
        assert!(!snapshots[0].issues.is_empty());

        let watchlist = service.remove_watch("microsoft/vscode".into()).unwrap();
        assert_eq!(watchlist.len(), 2);
    }
}
